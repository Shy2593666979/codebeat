use crate::{db, error, show, IpcResult};
use codebeat_lib::InboxNotice;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::{Emitter, Manager};
pub fn pending(database: &codebeat_lib::db::Database) -> anyhow::Result<Vec<InboxNotice>> {
    database.pending_notices()
}
static HOVER: AtomicBool = AtomicBool::new(false);
static ANCHOR: Mutex<Option<[f64; 2]>> = Mutex::new(None);
pub fn hover_active() -> bool {
    HOVER.load(Ordering::Relaxed)
}
pub fn stop_hover() {
    HOVER.store(false, Ordering::Relaxed);
}
fn ack_dir() -> anyhow::Result<std::path::PathBuf> {
    let p = codebeat_lib::db::data_dir()?.join("notification-acks");
    std::fs::create_dir_all(&p)?;
    Ok(p)
}
fn consume_acknowledgements(database: &mut codebeat_lib::db::Database) -> anyhow::Result<()> {
    let dir = ack_dir()?;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("dismiss")
            && entry
                .metadata()?
                .modified()?
                .elapsed()
                .is_ok_and(|age| age.as_secs() > 86400)
        {
            let _ = std::fs::remove_file(&path);
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("read") {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if uuid::Uuid::parse_str(id).is_ok() {
            database.acknowledge_notices(&[id.to_owned()])?;
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}
fn dismiss_popups(ids: &[String]) -> anyhow::Result<()> {
    let dir = ack_dir()?;
    for id in ids {
        uuid::Uuid::parse_str(id)?;
        std::fs::write(dir.join(format!("{id}.dismiss")), b"")?;
    }
    Ok(())
}
#[tauri::command]
pub fn can_present_notices(app: tauri::AppHandle) -> bool {
    app.get_webview_window("main").is_some_and(|window| {
        window.is_visible().unwrap_or(false)
            && window.is_focused().unwrap_or(false)
            && !window.is_minimized().unwrap_or(true)
    })
}
#[tauri::command]
pub fn get_pending_notices() -> IpcResult<Vec<InboxNotice>> {
    (|| -> anyhow::Result<_> {
        let mut database = db()?;
        consume_acknowledgements(&mut database)?;
        pending(&database)
    })()
    .map_err(error)
}
#[tauri::command]
pub fn publish_notice(
    id: String,
    title: String,
    message: String,
    kind: String,
    topic: Option<String>,
) -> IpcResult<()> {
    db().and_then(|database| {
        database.add_notice(&InboxNotice {
            topic,
            id,
            title,
            message,
            kind,
            task_id: None,
            page: "tasks".into(),
            at: chrono::Utc::now(),
        })
    })
    .map_err(error)
}
#[tauri::command]
pub fn acknowledge_notices(ids: Vec<String>) -> IpcResult<()> {
    (|| -> anyhow::Result<()> {
        let mut database = db()?;
        database.acknowledge_notices(&ids)?;
        dismiss_popups(&ids)?;
        Ok(())
    })()
    .map_err(error)
}
#[tauri::command]
pub fn ignore_all_notices(app: tauri::AppHandle) -> IpcResult<()> {
    (|| -> anyhow::Result<()> {
        let mut database = db()?;
        let ids = database.acknowledge_all_notices()?;
        dismiss_popups(&ids)?;
        let _ = app.emit("notices-changed", Vec::<InboxNotice>::new());
        if let Some(w) = app.get_webview_window("tray") {
            w.hide()?;
        }
        stop_hover();
        Ok(())
    })()
    .map_err(error)
}
#[tauri::command]
pub fn open_notice(app: tauri::AppHandle, id: String) -> IpcResult<()> {
    (|| -> anyhow::Result<()> {
        uuid::Uuid::parse_str(&id)?;
        let notice = pending(&db()?)?
            .into_iter()
            .find(|n| n.id == id)
            .ok_or_else(|| anyhow::anyhow!("NOTIFICATION_NOT_FOUND"))?;
        show(&app);
        let target = notice
            .task_id
            .map(|id| format!("/tasks?task={id}"))
            .unwrap_or_else(|| format!("/{}", notice.page));
        app.emit_to("main", "notification-open", target)?;
        app.emit_to("main", "inbox-present", ())?;
        if let Some(w) = app.get_webview_window("tray") {
            w.hide()?;
        }
        stop_hover();
        Ok(())
    })()
    .map_err(error)
}
pub fn hover(app: &tauri::AppHandle, position: tauri::PhysicalPosition<f64>) {
    if !hover_active()
        && app
            .get_webview_window("tray")
            .is_some_and(|w| w.is_visible().unwrap_or(false))
    {
        return;
    }
    if db()
        .and_then(|db| pending(&db))
        .is_ok_and(|items| !items.is_empty())
    {
        if let Ok(mut anchor) = ANCHOR.lock() {
            *anchor = Some([position.x, position.y]);
        }
        if !HOVER.swap(true, Ordering::Relaxed)
            && crate::tray::open_messages(app, position).is_err()
        {
            stop_hover();
        }
    }
}
pub fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut previous = Vec::<InboxNotice>::new();
        let mut phase = 0;
        let mut last_alpha = 255;
        let frames = app
            .default_window_icon()
            .map(|icon| REMINDER_ALPHAS.map(|alpha| reminder_frame(icon, alpha)));
        let mut pending_before = false;
        let mut outside_since = None;
        loop {
            let tick = std::time::Instant::now();
            if let Some(w) = app.get_webview_window("main") {
                let visible = w.is_visible().unwrap_or(false)
                    && w.is_focused().unwrap_or(false)
                    && !w.is_minimized().unwrap_or(true);
                if let Ok(dir) = ack_dir() {
                    let _ = std::fs::write(
                        dir.join("main-visible"),
                        if visible {
                            b"visible".as_slice()
                        } else {
                            b"hidden".as_slice()
                        },
                    );
                }
            }
            if let Ok(mut database) = db() {
                let _ = consume_acknowledgements(&mut database);
                if let Ok(items) = pending(&database) {
                    let pending = !items.is_empty();
                    if hover_active() && pending && items.len() != previous.len() {
                        if let Some(anchor) = ANCHOR.lock().ok().and_then(|a| *a) {
                            let _ = crate::tray::open_messages(
                                &app,
                                tauri::PhysicalPosition::new(anchor[0], anchor[1]),
                            );
                        }
                    }
                    if items != previous {
                        let _ = app.emit("notices-changed", &items);
                        previous = items;
                    }
                    if let Some(tray) = app.tray_by_id("main-tray") {
                        if pending {
                            // 保持托盘点击区域不变；每 500ms 在 RGBA 图标和
                            // 透明帧之间交替，鼠标悬停时同样交替。
                            if !pending_before {
                                phase = 0;
                            }
                            let alpha = REMINDER_ALPHAS[phase];
                            if alpha != last_alpha {
                                if let Some(frames) = &frames {
                                    let _ = tray.set_icon(Some(frames[phase].clone()));
                                }
                                last_alpha = alpha;
                            }
                            phase = (phase + 1) % REMINDER_ALPHAS.len();
                        } else if pending_before {
                            let _ = tray.set_icon(app.default_window_icon().cloned());
                            phase = 0;
                            last_alpha = 255;
                        }
                        if pending != pending_before {
                            let _ = tray.set_tooltip(if pending { None } else { Some("CodeBeat") });
                        }
                    }
                    pending_before = pending;
                    if hover_active() {
                        if !pending {
                            if let Some(w) = app.get_webview_window("tray") {
                                let _ = w.hide();
                            }
                            stop_hover();
                        } else if let (Ok(cursor), Some(w)) =
                            (app.cursor_position(), app.get_webview_window("tray"))
                        {
                            let inside = w
                                .outer_position()
                                .ok()
                                .zip(w.outer_size().ok())
                                .is_some_and(|(p, s)| {
                                    cursor.x >= p.x as f64
                                        && cursor.y >= p.y as f64
                                        && cursor.x < (p.x as f64 + s.width as f64)
                                        && cursor.y < (p.y as f64 + s.height as f64)
                                });
                            let on_icon = ANCHOR.lock().ok().and_then(|a| *a).is_some_and(|a| {
                                (cursor.x - a[0]).abs() < 30. && (cursor.y - a[1]).abs() < 30.
                            });
                            if inside || on_icon {
                                outside_since = None;
                            } else {
                                let since =
                                    outside_since.get_or_insert_with(std::time::Instant::now);
                                if since.elapsed() >= std::time::Duration::from_millis(650) {
                                    let _ = w.hide();
                                    stop_hover();
                                    outside_since = None;
                                }
                            }
                        }
                    }
                }
            }
            std::thread::sleep(REMINDER_INTERVAL.saturating_sub(tick.elapsed()));
        }
    });
}

const REMINDER_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);
const REMINDER_ALPHAS: [u8; 2] = [255, 0];
fn reminder_frame(icon: &tauri::image::Image<'_>, alpha: u8) -> tauri::image::Image<'static> {
    let mut rgba = icon.rgba().to_vec();
    if alpha == 0 {
        rgba.fill(0);
        // 全零 alpha 的 HICON 在 Windows 上会回退到旧的 AND 掩码。
        // 在角落放一个 alpha 为 1/255 的黑色像素即可启用 alpha 合成；
        // 它肉眼不可见，还能避免回退产生的黑色条纹。
        #[cfg(windows)]
        if let Some(corner) = rgba.get_mut(3) {
            *corner = 1;
        }
    } else {
        for channel in rgba.iter_mut().skip(3).step_by(4) {
            *channel = ((u16::from(*channel) * u16::from(alpha) + 127) / 255) as u8;
        }
    }
    tauri::image::Image::new_owned(rgba, icon.width(), icon.height())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reminder_alternates_full_logo_and_invisible_frame_every_half_second() {
        let mut source = vec![0; 32 * 32 * 4];
        source[..4].copy_from_slice(&[20, 40, 80, 255]);
        let icon = tauri::image::Image::new_owned(source, 32, 32);
        let hidden = reminder_frame(&icon, 0);
        assert_eq!(REMINDER_INTERVAL.as_millis(), 500);
        assert_eq!(REMINDER_ALPHAS, [255, 0]);
        assert_eq!(reminder_frame(&icon, 255).rgba(), icon.rgba());
        assert_eq!((hidden.width(), hidden.height()), (32, 32));
        assert!(hidden
            .rgba()
            .iter()
            .enumerate()
            .all(|(i, b)| i % 4 == 3 || *b == 0));
        #[cfg(windows)]
        {
            assert_eq!(hidden.rgba()[3], 1);
            assert!(hidden.rgba()[4..].iter().all(|b| *b == 0));
        }
        #[cfg(not(windows))]
        assert!(hidden.rgba().iter().all(|b| *b == 0));
        // 导出实际生产使用的帧，供离屏 Win32 DrawIconEx 质量检查使用。
        if let Some(dir) = std::env::var_os("CODEBEAT_ICON_TEST_DIR") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("hidden.rgba"), hidden.rgba()).unwrap();
        }
    }
}
