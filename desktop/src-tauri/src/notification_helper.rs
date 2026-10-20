//! 独立通知进程，不启动数据库恢复、托盘、应用发现或单实例转发。
//! 即使桌面主程序完全退出，通知仍可单独显示和交互。
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::PathBuf,
    sync::Mutex,
    time::Instant,
};
use tauri::{Manager, State};

#[derive(Clone, Deserialize, Serialize)]
struct Payload {
    title: String,
    body: String,
    kind: String,
    page: String,
    task_id: Option<String>,
    sound: bool,
    #[serde(skip_serializing)]
    ack_path: PathBuf,
    #[serde(skip_serializing)]
    dismiss_path: PathBuf,
}
impl Payload {
    fn validate(&self) -> Result<()> {
        if !matches!(self.kind.as_str(), "success" | "error" | "info")
            || !matches!(self.page.as_str(), "tasks" | "usage")
        {
            bail!("INVALID_NOTIFICATION");
        }
        if let Some(id) = &self.task_id {
            uuid::Uuid::parse_str(id)?;
        }
        let directory = codebeat_lib::db::data_dir()?.join("notification-acks");
        let id = self
            .ack_path
            .file_stem()
            .and_then(|s| s.to_str())
            .context("Missing notification ID")?;
        uuid::Uuid::parse_str(id)?;
        if self.ack_path != directory.join(format!("{id}.read"))
            || self.dismiss_path != directory.join(format!("{id}.dismiss"))
        {
            bail!("INVALID_NOTIFICATION_PATH");
        }
        Ok(())
    }
}
struct Lifecycle {
    seen: Option<Instant>,
    ready: bool,
}
struct Helper {
    payload: Payload,
    lifecycle: Mutex<Lifecycle>,
}
type Reply<T> = std::result::Result<T, String>;

fn ready() -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "READY")?;
    stdout.flush()?;
    Ok(())
}
#[tauri::command]
fn notification_payload(state: State<'_, Helper>) -> Payload {
    state.payload.clone()
}

#[tauri::command]
fn notification_ready(window: tauri::WebviewWindow, state: State<'_, Helper>) -> Reply<()> {
    (|| -> Result<()> {
        window.show()?;
        let mut lifecycle = state
            .lifecycle
            .lock()
            .map_err(|_| anyhow::anyhow!("Notification state unavailable"))?;
        if !lifecycle.ready {
            ready()?;
            lifecycle.ready = true;
        }
        #[cfg(windows)]
        if state.payload.sound {
            // 异步播放 Windows 提示音，不激活窗口或抢占焦点。
            unsafe {
                windows_sys::Win32::System::Diagnostics::Debug::MessageBeep(0x40);
            }
        }
        Ok(())
    })()
    .map_err(|e| e.to_string())
}

fn fully_visible(window: &tauri::WebviewWindow) -> Result<bool> {
    if !window.is_visible()? || window.is_minimized()? {
        return Ok(false);
    }
    let Some(monitor) = window.current_monitor()? else {
        return Ok(false);
    };
    let area = monitor.work_area();
    let position = window.outer_position()?;
    let size = window.outer_size()?;
    // 可见性检查仅排除外围十二个逻辑像素的透明阴影留白。
    let margin = (12. * window.scale_factor()?).round() as i32;
    Ok(position.x + margin >= area.position.x
        && position.y + margin >= area.position.y
        && position.x + size.width as i32 - margin <= area.position.x + area.size.width as i32
        && position.y + size.height as i32 - margin <= area.position.y + area.size.height as i32)
}
#[tauri::command]
fn notification_seen(window: tauri::WebviewWindow, state: State<'_, Helper>) -> Reply<bool> {
    (|| -> Result<bool> {
        let mut lifecycle = state
            .lifecycle
            .lock()
            .map_err(|_| anyhow::anyhow!("Notification state unavailable"))?;
        if lifecycle.seen.is_some() {
            return Ok(true);
        }
        if !fully_visible(&window)? {
            return Ok(false);
        }
        std::fs::write(&state.payload.ack_path, b"")?;
        lifecycle.seen = Some(Instant::now());
        Ok(true)
    })()
    .map_err(|e| e.to_string())
}
#[tauri::command]
fn notification_close(app: tauri::AppHandle, state: State<'_, Helper>, open: bool) -> Reply<()> {
    (|| -> Result<()> {
        if open {
            let mut command = std::process::Command::new(std::env::current_exe()?);
            if let Some(id) = &state.payload.task_id {
                command.args(["--open-task", id]);
            } else {
                command.args(["--open-page", &state.payload.page]);
            }
            command.spawn()?;
        }
        std::fs::write(&state.payload.ack_path, b"")?;
        app.exit(0);
        Ok(())
    })()
    .map_err(|e| e.to_string())
}

#[cfg(windows)]
struct Slot {
    handle: windows_sys::Win32::Foundation::HANDLE,
    index: u32,
}
#[cfg(windows)]
impl Slot {
    fn acquire(count: u32) -> Result<(Self, bool)> {
        use windows_sys::Win32::{Foundation::*, System::Threading::*};
        for index in 0..count {
            let candidate = Self::new(index)?;
            let status = unsafe { WaitForSingleObject(candidate.handle, 0) };
            if status == WAIT_OBJECT_0 || status == WAIT_ABANDONED {
                return Ok((candidate, false));
            }
            if status == WAIT_FAILED {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        // 位置已满时先报告 READY，再等待空位，避免阻塞调用方。
        // 排队期间仍检查“全部忽略”产生的关闭标记。
        ready()?;
        Ok((Self::new(0)?, true))
    }
    fn new(index: u32) -> Result<Self> {
        let name: Vec<_> = format!("Local\\CodeBeatNotification-{index}")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let handle = unsafe {
            windows_sys::Win32::System::Threading::CreateMutexW(std::ptr::null(), 0, name.as_ptr())
        };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(Self { handle, index })
    }
}
#[cfg(windows)]
impl Drop for Slot {
    fn drop(&mut self) {
        unsafe {
            // 未取得所有权的候选互斥量只会返回 ERROR_NOT_OWNER，可直接关闭句柄。
            windows_sys::Win32::System::Threading::ReleaseMutex(self.handle);
            windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

pub fn run() -> Result<()> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(256 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 256 * 1024 {
        bail!("Notification payload too large");
    }
    let payload: Payload = serde_json::from_slice(&bytes)?;
    payload.validate()?;
    let mut context = tauri::generate_context!();
    context.config_mut().app.windows.clear();
    let app = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            notification_payload,
            notification_ready,
            notification_seen,
            notification_close
        ])
        .build(context)?;
    let monitor = app.primary_monitor()?.context("No primary monitor")?;
    let area = monitor.work_area();
    let scale = monitor.scale_factor();
    let count = (((area.size.height as f64 / scale) - 24.) / 200.)
        .floor()
        .max(1.) as u32;
    #[cfg(windows)]
    let (slot, queued) = Slot::acquire(count)?;
    #[cfg(windows)]
    if queued {
        use windows_sys::Win32::{Foundation::*, System::Threading::*};
        loop {
            if payload.dismiss_path.exists() {
                let _ = std::fs::remove_file(&payload.dismiss_path);
                return Ok(());
            }
            let result = unsafe { WaitForSingleObject(slot.handle, 100) };
            if result == WAIT_OBJECT_0 || result == WAIT_ABANDONED {
                break;
            }
            if result == WAIT_FAILED {
                return Err(std::io::Error::last_os_error().into());
            }
        }
    }
    #[cfg(not(windows))]
    let (queued, count) = (false, {
        let _ = count;
        0
    });
    #[cfg(windows)]
    let index = slot.index;
    #[cfg(not(windows))]
    let index = count;
    if payload.dismiss_path.exists() {
        let _ = std::fs::remove_file(&payload.dismiss_path);
        return Ok(());
    }
    app.manage(Helper {
        payload: payload.clone(),
        lifecycle: Mutex::new(Lifecycle {
            seen: None,
            ready: queued,
        }),
    });
    let window = tauri::WebviewWindowBuilder::new(
        &app,
        "notification",
        tauri::WebviewUrl::App("index.html#/notification".into()),
    )
    .title("CodeBeat 消息通知")
    .inner_size(524., 200.)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .skip_taskbar(true)
    .always_on_top(true)
    .focused(false)
    .visible(false)
    .position(
        area.position.x as f64 / scale + (area.size.width as f64 / scale - 536.).max(0.),
        area.position.y as f64 / scale + 12. + index as f64 * 200.,
    )
    .build()?;
    // 由 Rust 线程处理外部关闭和已读计时，避免 WebView 节流影响关闭时机。
    let handle = app.handle().clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let state = handle.state::<Helper>();
        let dismissed = state.payload.dismiss_path.exists();
        let elapsed = state
            .lifecycle
            .lock()
            .ok()
            .and_then(|s| s.seen)
            .is_some_and(|at| at.elapsed().as_secs_f64() >= 2.);
        if dismissed || elapsed {
            if dismissed {
                let _ = std::fs::remove_file(&state.payload.dismiss_path);
            }
            handle.exit(0);
            break;
        }
    });
    let ack = payload.ack_path;
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
            let _ = std::fs::write(&ack, b"");
        }
    });
    app.run(|_, _| {});
    // 界面退出后，由取得互斥量所有权的线程释放通知位置。
    Ok(())
}
