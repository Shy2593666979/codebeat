//! 直接设置真实的 Win32 客户区尺寸，绕开 Tao 对阴影/标题栏的调整。
//! 在带阴影的无边框窗口中，这种调整在 96 DPI 下会多出 30px。
use std::ffi::c_void;

#[repr(C)]
#[derive(Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[link(name = "user32")]
extern "system" {
    fn GetClientRect(hwnd: *mut c_void, rect: *mut Rect) -> i32;
    fn GetWindowRect(hwnd: *mut c_void, rect: *mut Rect) -> i32;
    fn SetWindowPos(
        hwnd: *mut c_void,
        after: *mut c_void,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
}

pub(super) fn set_content_size(
    window: &tauri::WebviewWindow,
    width: f64,
    height: f64,
) -> anyhow::Result<()> {
    let dpi = window.scale_factor()?;
    let hwnd = window.hwnd()?.0 as isize;
    window.run_on_main_thread(move || {
        let hwnd = hwnd as *mut c_void;
        let mut client = Rect::default();
        let mut outer = Rect::default();
        // SAFETY: hwnd 属于这个仍然存活的 Tauri 窗口；矩形可写，且尺寸调整
        // 运行在其 UI 线程上。保持位置、焦点、Z 序和窗口样式不变。
        unsafe {
            if GetClientRect(hwnd, &mut client) == 0 || GetWindowRect(hwnd, &mut outer) == 0 {
                eprintln!(
                    "Unable to measure window size: {}",
                    std::io::Error::last_os_error()
                );
                return;
            }
            let frame_width = (outer.right - outer.left) - (client.right - client.left);
            let frame_height = (outer.bottom - outer.top) - (client.bottom - client.top);
            if SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                (width * dpi).round() as i32 + frame_width,
                (height * dpi).round() as i32 + frame_height,
                0x0002 | 0x0004 | 0x0010,
            ) == 0
            {
                eprintln!(
                    "Unable to resize window: {}",
                    std::io::Error::last_os_error()
                );
            }
        }
    })?;
    Ok(())
}
