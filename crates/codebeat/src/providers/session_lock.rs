//! 检查 Codex 的实际写入锁；不恢复会话，不启动 CLI，也不发送模型请求。
use crate::{ProviderId, SessionLockState};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
pub struct SessionIdentity {
    pub provider: ProviderId,
    pub session_id: String,
}

pub fn state(provider: ProviderId, id: &str) -> SessionLockState {
    if provider != ProviderId::Codex {
        return SessionLockState::Unsupported;
    }
    if uuid::Uuid::parse_str(id).is_err() {
        return SessionLockState::Unknown;
    }
    let root = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(|home| PathBuf::from(home).join(".codex"))
        });
    let Some(root) = root else {
        return SessionLockState::Unknown;
    };
    probe(&root.join("thread-writer-locks").join(format!("{id}.lock")))
}

#[cfg(windows)]
fn probe(path: &Path) -> SessionLockState {
    use std::{fs::OpenOptions, os::windows::io::AsRawHandle};
    use windows_sys::Win32::{
        Foundation::{ERROR_LOCK_VIOLATION, ERROR_SHARING_VIOLATION},
        Storage::FileSystem::{
            LockFileEx, UnlockFileEx, LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY,
        },
        System::IO::OVERLAPPED,
    };
    let file = match OpenOptions::new().read(true).write(true).open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return SessionLockState::Available
        }
        Err(error) if error.raw_os_error() == Some(ERROR_SHARING_VIOLATION as i32) => {
            return SessionLockState::InUse
        }
        Err(_) => return SessionLockState::Unknown,
    };
    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    // 非阻塞尝试锁住整个文件；锁文件存在但已释放不代表会话仍在使用。
    let acquired = unsafe {
        LockFileEx(
            file.as_raw_handle(),
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            u32::MAX,
            u32::MAX,
            &mut overlapped,
        )
    };
    if acquired == 0 {
        return if std::io::Error::last_os_error().raw_os_error()
            == Some(ERROR_LOCK_VIOLATION as i32)
        {
            SessionLockState::InUse
        } else {
            SessionLockState::Unknown
        };
    }
    let released =
        unsafe { UnlockFileEx(file.as_raw_handle(), 0, u32::MAX, u32::MAX, &mut overlapped) };
    // 即使解锁失败，文件句柄也会随作用域释放；不能把失败返回成空闲。
    if released != 0 {
        SessionLockState::Available
    } else {
        SessionLockState::Unknown
    }
}

#[cfg(not(windows))]
fn probe(_path: &Path) -> SessionLockState {
    SessionLockState::Unknown
}

pub fn require_known(provider: ProviderId, id: &str) -> anyhow::Result<SessionLockState> {
    let state = state(provider, id);
    if state == SessionLockState::Unknown {
        anyhow::bail!("SESSION_LOCK_UNKNOWN: 无法确认会话占用状态，请刷新后重试");
    }
    Ok(state)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::{Storage::FileSystem::*, System::IO::OVERLAPPED};

    #[test]
    fn checks_system_lock_and_releases_its_probe_without_creating_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.lock");
        assert_eq!(probe(&path), SessionLockState::Available);
        assert!(!path.exists());
        let file = std::fs::File::create(&path).unwrap();
        assert_eq!(probe(&path), SessionLockState::Available);
        let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
        assert_ne!(
            unsafe {
                LockFileEx(
                    file.as_raw_handle(),
                    LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
                    0,
                    u32::MAX,
                    u32::MAX,
                    &mut overlapped,
                )
            },
            0
        );
        assert_eq!(probe(&path), SessionLockState::InUse);
        assert_ne!(
            unsafe { UnlockFileEx(file.as_raw_handle(), 0, u32::MAX, u32::MAX, &mut overlapped) },
            0
        );
        assert_eq!(probe(&path), SessionLockState::Available);
        assert_eq!(probe(directory.path()), SessionLockState::Unknown);
        assert_eq!(
            state(ProviderId::Codex, "../invalid"),
            SessionLockState::Unknown
        );
        assert_eq!(
            state(ProviderId::ClaudeCode, "unused"),
            SessionLockState::Unsupported
        );
    }
}
