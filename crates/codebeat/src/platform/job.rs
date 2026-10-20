use anyhow::Result;
#[cfg(windows)]
pub struct ProcessJob(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl ProcessJob {
    pub fn assign(pid: u32) -> Result<Self> {
        use windows_sys::Win32::{
            Foundation::{CloseHandle, GetLastError},
            System::{JobObjects::*, Threading::*},
        };
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(std::io::Error::last_os_error().into());
            }
            let job = Self(handle);
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of_val(&info) as u32,
            ) == 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
            let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, false as i32, pid);
            if process.is_null() {
                return Err(std::io::Error::last_os_error().into());
            }
            let ok = AssignProcessToJobObject(handle, process);
            let code = GetLastError();
            CloseHandle(process);
            if ok == 0 {
                return Err(std::io::Error::from_raw_os_error(code as i32).into());
            }
            Ok(job)
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn dropping_job_terminates_owned_process() {
        let mut child = std::process::Command::new("ping.exe")
            .args(["-n", "30", "127.0.0.1"])
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let job = super::ProcessJob::assign(child.id()).unwrap();
        assert!(child.try_wait().unwrap().is_none());
        drop(job);
        let start = std::time::Instant::now();
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if start.elapsed() > std::time::Duration::from_secs(3) {
                let _ = child.kill();
                panic!("owned process survived job close");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}
#[cfg(windows)]
impl Drop for ProcessJob {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
#[cfg(not(windows))]
pub struct ProcessJob;
#[cfg(not(windows))]
impl ProcessJob {
    pub fn assign(_: u32) -> Result<Self> {
        Ok(Self)
    }
}
