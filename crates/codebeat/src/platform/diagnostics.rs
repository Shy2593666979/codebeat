//! 任务和电源诊断；直接执行系统 powercfg，并限制执行时间和采集长度。
use super::scheduler;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};

pub(super) fn diagnostics() -> Result<Value> {
    fn powercfg(args: &[&str]) -> Result<String> {
        use std::os::windows::process::CommandExt;
        let executable =
            PathBuf::from(std::env::var_os("SystemRoot").context("SystemRoot missing")?)
                .join("System32/powercfg.exe");
        let mut child = Command::new(executable)
            .args(args)
            .creation_flags(0x08000000)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        fn capture(
            mut stream: impl std::io::Read + Send + 'static,
        ) -> std::thread::JoinHandle<std::io::Result<Vec<u8>>> {
            std::thread::spawn(move || {
                use std::io::Read;
                let mut bytes = Vec::new();
                std::io::Read::by_ref(&mut stream)
                    .take(512 * 1024)
                    .read_to_end(&mut bytes)?;
                std::io::copy(&mut stream, &mut std::io::sink())?;
                Ok(bytes)
            })
        }
        let stdout = capture(child.stdout.take().context("powercfg stdout unavailable")?);
        let stderr = capture(child.stderr.take().context("powercfg stderr unavailable")?);
        let started = std::time::Instant::now();
        while child.try_wait()?.is_none() {
            if started.elapsed() > Duration::from_secs(15) {
                let _ = child.kill();
                let _ = child.wait();
                bail!("WINDOWS_API_TIMEOUT: powercfg");
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        // powercfg 使用 Windows 控制台代码页，输出不一定是 UTF-8。
        let bytes = [
            stdout
                .join()
                .map_err(|_| anyhow::anyhow!("powercfg stdout reader failed"))??,
            stderr
                .join()
                .map_err(|_| anyhow::anyhow!("powercfg stderr reader failed"))??,
        ]
        .concat();
        Ok(console_text(&bytes))
    }
    Ok(
        json!({"tasks":scheduler::task_diagnostics()?,"sleepStates":powercfg(&["/a"])?,"wakeTimers":powercfg(&["/waketimers"])?,"wakePolicy":powercfg(&["/query","SCHEME_CURRENT","SUB_SLEEP","RTCWAKE"])?,"message":"若唤醒计时器禁用，请打开控制面板 > 电源选项 > 高级电源设置 > 睡眠 > 允许使用唤醒定时器。部分诊断需要管理员权限；应用不会自动提权。"}),
    )
}

// 按控制台或系统 OEM 代码页解码重定向输出，兼容非 UTF-8 的系统工具。
fn console_text(bytes: &[u8]) -> String {
    use windows::Win32::{Globalization::*, System::Console::GetConsoleOutputCP};
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_owned();
    }
    unsafe {
        let console = GetConsoleOutputCP();
        let codepage = if console == 0 { CP_OEMCP } else { console };
        let count = MultiByteToWideChar(codepage, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), bytes, None);
        let mut text = vec![0u16; count.max(0) as usize];
        if count > 0 {
            MultiByteToWideChar(
                codepage,
                MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0),
                bytes,
                Some(&mut text),
            );
        }
        String::from_utf16_lossy(&text)
    }
}
