#[cfg(windows)]
mod native;
use crate::{error, IpcResult};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Mutex};
use tauri::{Emitter, Manager};

#[derive(Clone, Serialize, Deserialize)]
struct Preference {
    scale: f64,
    width: f64,
    height: f64,
}
impl Default for Preference {
    fn default() -> Self {
        Self {
            scale: 0.,
            width: 1200.,
            height: 800.,
        }
    }
}
#[derive(Default)]
pub struct LayoutState(Mutex<LayoutData>);
#[derive(Default)]
struct LayoutData {
    prefs: BTreeMap<String, Preference>,
    key: String,
    scale: f64,
    applying: bool,
    first_show: bool,
}
#[derive(Clone, Serialize)]
pub struct LayoutInfo {
    pub selected: f64,
    pub effective: f64,
}
fn key(m: &tauri::Monitor) -> String {
    format!(
        "{}:{}x{}:{}",
        m.name().map(String::as_str).unwrap_or("display"),
        m.size().width,
        m.size().height,
        m.scale_factor()
    )
}
fn path() -> anyhow::Result<std::path::PathBuf> {
    Ok(codebeat_lib::db::data_dir()?.join("window-layout.json"))
}
fn write(data: &LayoutData) -> anyhow::Result<()> {
    let path = path()?;
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, serde_json::to_vec(&data.prefs)?)?;
    std::fs::rename(temp, path)?;
    Ok(())
}
// 工作区和窗口尺寸使用逻辑像素；操作系统 DPI 另行处理。
fn dimensions(area: [f64; 2], pref: &Preference) -> (f64, f64, f64) {
    let width = pref.width.clamp(1024., 2400.);
    let height = pref.height.clamp(700., 1800.);
    let auto = (area[0] / 1920.).min(area[1] / 1040.).clamp(0.5, 1.4);
    // 任务栏或工作区的小幅差异不能让 1080p 基准尺寸缩小。
    let auto = if (auto - 1.).abs() <= 0.02 { 1. } else { auto };
    let requested = if pref.scale == 0. { auto } else { pref.scale };
    let scale = requested
        .min(area[0] * 0.94 / width)
        .min(area[1] * 0.94 / height)
        .max(0.1);
    (width * scale, height * scale, scale)
}
// 托盘和主界面使用相同的分屏适配逻辑，包括手动覆盖值。
pub(crate) fn monitor_scale(app: &tauri::AppHandle, monitor: &tauri::Monitor) -> f64 {
    let pref = app
        .try_state::<LayoutState>()
        .and_then(|state| {
            state
                .0
                .lock()
                .ok()
                .and_then(|data| data.prefs.get(&key(monitor)).cloned())
        })
        .unwrap_or_default();
    let area = monitor.work_area();
    let dpi = monitor.scale_factor();
    dimensions(
        [area.size.width as f64 / dpi, area.size.height as f64 / dpi],
        &pref,
    )
    .2
}

pub fn initialize(app: &tauri::AppHandle) -> anyhow::Result<()> {
    let prefs = std::fs::read(path()?)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    app.manage(LayoutState(Mutex::new(LayoutData {
        prefs,
        first_show: true,
        ..Default::default()
    })));
    apply(app, true)?;
    Ok(())
}
// Windows 在窗口首次显示时才会确定无边框阴影的尺寸度量。
// 在完成该转换后、记录尺寸之前，再应用一次目标客户区尺寸。
pub fn shown(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<LayoutState>() else {
        return;
    };
    let first = state
        .0
        .lock()
        .map(|mut data| {
            let first = data.first_show;
            data.first_show = false;
            first
        })
        .unwrap_or(false);
    if first {
        if let Err(error) = apply(app, false) {
            eprintln!("Window size restoration failed: {error}");
        }
    }
}
fn apply(app: &tauri::AppHandle, initial: bool) -> anyhow::Result<LayoutInfo> {
    let w = app
        .get_webview_window("main")
        .ok_or_else(|| anyhow::anyhow!("Main window unavailable"))?;
    let monitor = if initial {
        app.cursor_position()
            .ok()
            .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
    } else {
        None
    }
    .or(w.current_monitor()?)
    .or(app.primary_monitor()?)
    .ok_or_else(|| anyhow::anyhow!("Display unavailable"))?;
    let dpi = monitor.scale_factor();
    let area = monitor.work_area();
    let display = key(&monitor);
    let state = app.state::<LayoutState>();
    let pref = {
        let mut data = state.0.lock().map_err(|_| anyhow::anyhow!("Layout lock"))?;
        if data.applying {
            return Ok(LayoutInfo {
                selected: data.prefs.get(&data.key).map(|p| p.scale).unwrap_or(0.),
                effective: data.scale,
            });
        }
        data.applying = true;
        data.key = display.clone();
        data.prefs.entry(display).or_default().clone()
    };
    let (width, height, scale) = dimensions(
        [area.size.width as f64 / dpi, area.size.height as f64 / dpi],
        &pref,
    );
    let result = (|| -> anyhow::Result<LayoutInfo> {
        w.set_min_size(Some(tauri::LogicalSize::new(1024. * scale, 700. * scale)))?;
        w.set_zoom(scale)?;
        if !w.is_maximized()? && !w.is_fullscreen()? {
            #[cfg(windows)]
            native::set_content_size(&w, width, height)?;
            #[cfg(not(windows))]
            w.set_size(tauri::LogicalSize::new(width, height))?;
            let old = w.outer_position()?;
            let x = if initial {
                area.position.x as f64 + (area.size.width as f64 - width * dpi) / 2.
            } else {
                old.x as f64
            };
            let y = if initial {
                area.position.y as f64 + (area.size.height as f64 - height * dpi) / 2.
            } else {
                old.y as f64
            };
            w.set_position(tauri::PhysicalPosition::new(
                x.clamp(
                    area.position.x as f64,
                    area.position.x as f64 + area.size.width as f64 - width * dpi,
                )
                .round() as i32,
                y.clamp(
                    area.position.y as f64,
                    area.position.y as f64 + area.size.height as f64 - height * dpi,
                )
                .round() as i32,
            ))?;
        }
        Ok(LayoutInfo {
            selected: pref.scale,
            effective: scale,
        })
    })();
    {
        let mut data = state.0.lock().map_err(|_| anyhow::anyhow!("Layout lock"))?;
        data.applying = false;
        if result.is_ok() {
            data.scale = scale;
        }
    }
    if let Ok(info) = &result {
        let _ = app.emit("display-layout", info);
    }
    result
}
pub fn monitor_changed(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<LayoutState>() else {
        return;
    };
    let Some(w) = app.get_webview_window("main") else {
        return;
    };
    let Ok(Some(monitor)) = w.current_monitor() else {
        return;
    };
    let changed = state
        .0
        .lock()
        .map(|data| !data.applying && data.key != key(&monitor))
        .unwrap_or(false);
    if changed {
        let _ = apply(app, false);
    }
}
#[tauri::command]
pub fn get_display_layout(app: tauri::AppHandle) -> IpcResult<LayoutInfo> {
    let data = app.state::<LayoutState>();
    let data = data.0.lock().map_err(|_| "Layout lock")?;
    Ok(LayoutInfo {
        selected: data.prefs.get(&data.key).map(|p| p.scale).unwrap_or(0.),
        effective: data.scale,
    })
}
#[tauri::command]
pub fn set_display_scale(app: tauri::AppHandle, value: f64) -> IpcResult<LayoutInfo> {
    if ![0., 0.8, 0.9, 1., 1.1, 1.25]
        .iter()
        .any(|v| f64::abs(*v - value) < 0.0001)
    {
        return Err("INVALID_DISPLAY_SCALE".into());
    }
    (|| -> anyhow::Result<LayoutInfo> {
        let state = app.state::<LayoutState>();
        {
            let mut data = state.0.lock().map_err(|_| anyhow::anyhow!("Layout lock"))?;
            let key = data.key.clone();
            data.prefs.entry(key).or_default().scale = value;
            write(&data)?;
        }
        apply(&app, false)
    })()
    .map_err(error)
}
#[tauri::command]
pub fn save_window_layout(app: tauri::AppHandle) -> IpcResult<()> {
    (|| -> anyhow::Result<()> {
        let w = app
            .get_webview_window("main")
            .ok_or_else(|| anyhow::anyhow!("Main window unavailable"))?;
        if w.is_maximized()? || w.is_fullscreen()? || w.is_minimized()? {
            return Ok(());
        }
        let size = w.inner_size()?;
        let dpi = w.scale_factor()?;
        let state = app.state::<LayoutState>();
        let mut data = state.0.lock().map_err(|_| anyhow::anyhow!("Layout lock"))?;
        if data.applying || data.scale <= 0. {
            return Ok(());
        }
        let key = data.key.clone();
        let scale = data.scale;
        let pref = data.prefs.entry(key).or_default();
        pref.width = (size.width as f64 / dpi / scale).clamp(1024., 2400.);
        pref.height = (size.height as f64 / dpi / scale).clamp(700., 1800.);
        write(&data)?;
        Ok(())
    })()
    .map_err(error)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn baseline_and_proportions_follow_work_area() {
        let p = Preference::default();
        let baseline = dimensions([1920., 1040.], &p);
        assert_eq!(baseline, (1200., 800., 1.));
        assert_eq!(dimensions([1920., 1032.], &p), (1200., 800., 1.));
        assert_eq!(
            dimensions([1920., 1010.], &p),
            (1200. * 1010. / 1040., 800. * 1010. / 1040., 1010. / 1040.)
        );
        for area in [[2560., 1400.], [1536., 832.], [1280., 693.], [800., 550.]] {
            let (w, h, s) = dimensions(area, &p);
            assert!((w / h - 1200. / 800.).abs() < 0.0001);
            assert!(w <= area[0] * 0.94 + 0.01 && h <= area[1] * 0.94 + 0.01);
            assert!(s > 0.);
        }
        assert!(dimensions([2560., 1400.], &p).2 > 1.);
        assert!(dimensions([1536., 832.], &p).2 < 1.);
    }
    #[test]
    fn saved_sizes_and_override_are_clamped_without_distortion() {
        let p = Preference {
            scale: 1.25,
            width: 1400.,
            height: 900.,
        };
        let (w, h, s) = dimensions([1536., 832.], &p);
        assert!((w / h - 1400. / 900.).abs() < 0.0001);
        assert!(h <= 832. * 0.94 + 0.01);
        assert!(s < 1.25);
    }
}
