use crate::{db, error, run_task_now, show, IpcResult};
use serde_json::{json, Value};
use tauri::{Emitter, Manager};

static LAST_LAYOUT: std::sync::Mutex<Option<Value>> = std::sync::Mutex::new(None);

#[tauri::command]
pub fn get_tray_snapshot(app: tauri::AppHandle) -> IpcResult<Value> {
    (|| {
        let database = db()?;
        let layout = LAST_LAYOUT.lock().ok().and_then(|value| value.clone());
        let usage_refresh = crate::usage_refresh::get_usage_refresh(app);
        let usage: Vec<_> = usage_refresh.iter().map(|state| state.report.clone()).collect();
        Ok(json!({"usageRefresh":usage_refresh,"layout":layout,"tasks":database.tasks()?,"settings":database.settings()?,
            "inbox":crate::notifications::pending(&database)?,"usage":usage}))
    })()
    .map_err(error)
}

#[tauri::command]
pub fn tray_action(
    action: String,
    task_id: Option<String>,
    app: tauri::AppHandle,
) -> IpcResult<()> {
    if action == "hide" {
        crate::notifications::stop_hover();
        if let Some(window) = app.get_webview_window("tray") {
            window.hide().map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    if action == "pause" {
        return (|| {
            let database = db()?;
            let mut settings = database.settings()?;
            settings.paused = !settings.paused;
            database.save_settings(&settings)?;
            app.emit("settings-updated", settings)?;
            Ok(())
        })()
        .map_err(error);
    }
    if action == "continue" {
        run_task_now(task_id.ok_or("请选择可继续的已排程任务")?)?;
    } else {
        let target = match action.as_str() {
            "open" | "status" => "/".to_string(),
            "tasks" => match task_id {
                Some(id) => format!(
                    "/tasks?task={}",
                    uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?
                ),
                None => "/tasks".into(),
            },
            "usage" => "/usage".into(),
            "settings" => "/settings".into(),
            "exit" => String::new(),
            _ => return Err("未知托盘操作".into()),
        };
        show(&app);
        if action == "exit" {
            app.emit_to("main", "exit-requested", ())
                .map_err(|e| e.to_string())?;
        } else {
            app.emit_to("main", "notification-open", target)
                .map_err(|e| e.to_string())?;
        }
    }
    if let Some(window) = app.get_webview_window("tray") {
        window.hide().map_err(|e| e.to_string())?;
    }
    crate::notifications::stop_hover();
    Ok(())
}

#[derive(Debug, PartialEq)]
pub(crate) enum ClickAction {
    Main,
    Menu,
}
pub(crate) fn click_action(
    button: tauri::tray::MouseButton,
    state: tauri::tray::MouseButtonState,
) -> Option<ClickAction> {
    if state != tauri::tray::MouseButtonState::Up {
        return None;
    }
    match button {
        tauri::tray::MouseButton::Left => Some(ClickAction::Main),
        tauri::tray::MouseButton::Right => Some(ClickAction::Menu),
        _ => None,
    }
}
#[derive(Debug)]
struct Placement {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    opens_left: bool,
    tail: f64,
    above: bool,
    scale: f64,
}
// 位置使用物理像素；窗口尺寸和 CSS 缩放使用逻辑像素。
fn placement(area: [f64; 4], anchor: [f64; 2], dpi: f64, adaptation: f64) -> Placement {
    let [left, top, right, bottom] = area;
    let scale = (0.85 * adaptation)
        .min((right - left) / dpi / 606.)
        .min((bottom - top) / dpi / 620.);
    let density = dpi * scale;
    let width = 606. * density;
    let height = 620. * density;
    let menu_left = anchor[0] - (326. - 16.) * density;
    let opens_left = menu_left + 592. * density > right;
    let menu_offset = if opens_left { 266. } else { 14. };
    let x = (menu_left - menu_offset * density).clamp(left, right - width);
    let above = anchor[1] > top + height;
    let y = (if above {
        anchor[1] - height - 6. * density
    } else {
        anchor[1] + 24. * density
    })
    .clamp(top, bottom - height);
    let tail = ((anchor[0] - x) / density - menu_offset).clamp(20., 306.);
    Placement {
        x,
        y,
        width: width / dpi,
        height: height / dpi,
        opens_left,
        tail,
        above,
        scale,
    }
}
fn message_placement(
    area: [f64; 4],
    icon: [f64; 4],
    dpi: f64,
    adaptation: f64,
    count: usize,
) -> Placement {
    let [left, top, right, bottom] = area;
    // 与不滚动的 CSS 保持一致：头部/尾部/内边距 + 每行 104px、行间距 10px。
    let logical_height = 110. + count.max(1) as f64 * 114.;
    let scale = (0.85 * adaptation)
        .min((right - left) / dpi / 354.)
        .min((bottom - top) / dpi / logical_height);
    let density = dpi * scale;
    let width = 354. * density;
    let height = logical_height * density;
    let center = [icon[0] + icon[2] / 2., icon[1] + icon[3] / 2.];
    let above = center[1] >= (top + bottom) / 2.;
    let mut x = center[0] - width / 2.;
    let mut y = if above {
        icon[1] - height
    } else {
        icon[1] + icon[3]
    };
    // 竖排任务栏：面板紧贴图标，并沿图标方向居中。
    if center[0] < left {
        x = icon[0] + icon[2];
        y = center[1] - height / 2.;
    } else if center[0] > right {
        x = icon[0] - width;
        y = center[1] - height / 2.;
    }
    Placement {
        x: x.clamp(left, (right - width).max(left)),
        y: y.clamp(top, (bottom - height).max(top)),
        width: width / dpi,
        height: height / dpi,
        opens_left: false,
        tail: 0.,
        above,
        scale,
    }
}

pub fn open(app: &tauri::AppHandle, position: tauri::PhysicalPosition<f64>) -> tauri::Result<()> {
    crate::notifications::stop_hover();
    open_mode(app, position, false)
}
pub fn open_messages(
    app: &tauri::AppHandle,
    position: tauri::PhysicalPosition<f64>,
) -> tauri::Result<()> {
    open_mode(app, position, true)
}
fn open_mode(
    app: &tauri::AppHandle,
    position: tauri::PhysicalPosition<f64>,
    messages: bool,
) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window("tray") else {
        return Ok(());
    };
    let Some(monitor) = app.monitor_from_point(position.x, position.y)? else {
        return Ok(());
    };
    let area = monitor.work_area();
    let scale = monitor.scale_factor();
    let area = [
        area.position.x as f64,
        area.position.y as f64,
        area.position.x as f64 + area.size.width as f64,
        area.position.y as f64 + area.size.height as f64,
    ];
    let anchor = [position.x, position.y];
    let adaptation = crate::window::monitor_scale(app, &monitor);
    let layout = if messages {
        let count = db()
            .and_then(|d| crate::notifications::pending(&d))
            .map_or(0, |items| items.len());
        let icon = app
            .tray_by_id("main-tray")
            .and_then(|tray| tray.rect().ok().flatten())
            .map(|rect| {
                let p = rect.position.to_physical::<f64>(scale);
                let size = rect.size.to_physical::<f64>(scale);
                [p.x, p.y, size.width, size.height]
            })
            .unwrap_or([position.x, position.y, 0., 0.]);
        message_placement(area, icon, scale, adaptation, count)
    } else {
        placement(area, anchor, scale, adaptation)
    };
    window.set_size(tauri::LogicalSize::new(layout.width, layout.height))?;
    window.set_position(tauri::PhysicalPosition::new(
        layout.x.round() as i32,
        layout.y.round() as i32,
    ))?;
    let layout = json!({"opensLeft":layout.opens_left,"tail":layout.tail,"above":layout.above,"messages":messages,"scale":layout.scale});
    if let Ok(mut last) = LAST_LAYOUT.lock() {
        *last = Some(layout.clone());
    }
    window.emit("tray-open", layout)?;
    window.show()?;
    if !messages {
        window.set_focus()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn positions_fit_work_area_with_uniform_monitor_and_dpi_scaling() {
        for (area, anchor, dpi, adaptation) in [
            ([0., 0., 1920., 1040.], [1400., 1060.], 1., 1.),
            ([0., 0., 1920., 1040.], [1830., 1060.], 1.25, 0.8),
            ([-1920., -200., 0., 840.], [-90., 850.], 1.5, 0.67),
            ([0., 40., 1280., 720.], [30., 15.], 1., 0.65),
            ([0., 0., 800., 600.], [790., 590.], 1.5, 0.5),
        ] {
            let p = placement(area, anchor, dpi, adaptation);
            assert!(p.x >= area[0] && p.y >= area[1]);
            assert!(
                p.x + p.width * dpi <= area[2] + 0.01 && p.y + p.height * dpi <= area[3] + 0.01
            );
            assert!((20.0..=306.0).contains(&p.tail));
            assert!(
                (p.width / 606. - p.scale).abs() < 0.0001
                    && (p.height / 620. - p.scale).abs() < 0.0001
            );
        }
        let baseline = placement([0., 0., 1920., 1040.], [1400., 1060.], 1., 1.);
        assert!(!baseline.opens_left);
        assert!(placement([0., 0., 1920., 1040.], [1830., 1060.], 1., 1.).opens_left);
        let small = placement([0., 0., 1536., 832.], [1400., 850.], 1., 0.8);
        assert!((small.width / baseline.width - 0.8).abs() < 0.0001);
    }
    #[test]
    fn left_release_opens_main_and_right_release_opens_menu() {
        use tauri::tray::{MouseButton::*, MouseButtonState::*};
        assert_eq!(click_action(Left, Up), Some(ClickAction::Main));
        assert_eq!(click_action(Right, Up), Some(ClickAction::Menu));
        assert_eq!(click_action(Left, Down), None);
        assert_eq!(click_action(Right, Down), None);
        assert_eq!(click_action(Middle, Up), None);
    }
    #[test]
    fn message_popup_uses_compact_canvas_and_stays_inside_each_monitor() {
        for (area, anchor, dpi, adaptation) in [
            ([0., 0., 1920., 1040.], [1840., 1060.], 1., 1.),
            ([-1920., -200., 0., 840.], [-30., 850.], 1.5, 0.67),
            ([0., 40., 800., 600.], [15., 20.], 1.25, 0.5),
        ] {
            for count in [0, 1, 2, 200] {
                let p = message_placement(
                    area,
                    [anchor[0], anchor[1], 24., 24.],
                    dpi,
                    adaptation,
                    count,
                );
                assert!(p.x >= area[0] && p.y >= area[1]);
                assert!(
                    p.x + p.width * dpi <= area[2] + 0.01 && p.y + p.height * dpi <= area[3] + 0.01
                );
                assert!(!p.opens_left);
                assert_eq!(p.tail, 0.);
                assert!((p.width / p.scale - 354.).abs() < 0.001);
            }
        }
    }
    #[test]
    fn inbox_is_centered_next_to_the_icon_and_grows_for_every_notice() {
        let area = [0., 0., 1920., 1040.];
        let two = message_placement(area, [1388., 1040., 24., 24.], 1., 1., 2);
        assert!((two.x + two.width / 2. - 1400.).abs() < 0.01);
        assert!((two.y + two.height - 1040.).abs() < 0.01);
        assert!((two.height / two.scale - 338.).abs() < 0.01);
        let ten = message_placement(area, [1388., 1040., 24., 24.], 1., 1., 10);
        assert!((ten.height / ten.scale - 1250.).abs() < 0.01);
        assert!(ten.scale < two.scale);
        let top = message_placement([0., 40., 1920., 1080.], [1388., 16., 24., 24.], 1., 1., 2);
        assert_eq!(top.y, 40.);
        assert!(!top.above);
    }
}
