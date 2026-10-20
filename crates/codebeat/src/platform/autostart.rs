//! 当前用户的开机自启设置，通过 Run 注册表值保存带引号的程序路径。
#[cfg(test)]
use super::wide;
use anyhow::Result;
use std::{os::windows::ffi::OsStrExt, path::Path};
use windows::{
    core::PCWSTR,
    Win32::{Foundation::*, System::Registry::*},
};

struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

/// 设置当前用户的自启项；重复开启或删除不存在的值均可安全执行。
pub(super) fn autostart(enabled: bool) -> Result<()> {
    set_autostart(
        windows::core::w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
        enabled,
        &std::env::current_exe()?,
    )
}

fn set_autostart(subkey: PCWSTR, enabled: bool, exe: &Path) -> Result<()> {
    unsafe {
        let mut key = HKEY::default();
        let status = RegOpenKeyExW(HKEY_CURRENT_USER, subkey, None, KEY_SET_VALUE, &mut key);
        if !enabled && status == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        if enabled && status == ERROR_FILE_NOT_FOUND {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                subkey,
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut key,
                None,
            )
            .ok()?;
        } else {
            status.ok()?;
        }
        let key = Key(key);
        if enabled {
            let mut value = vec![b'"' as u16];
            value.extend(exe.as_os_str().encode_wide());
            value.extend("\" --background".encode_utf16());
            value.push(0);
            let bytes: Vec<_> = value.into_iter().flat_map(u16::to_le_bytes).collect();
            RegSetValueExW(
                key.0,
                windows::core::w!("codebeat"),
                None,
                REG_SZ,
                Some(&bytes),
            )
            .ok()?;
        } else {
            let status = RegDeleteValueW(key.0, windows::core::w!("codebeat"));
            if status != ERROR_FILE_NOT_FOUND {
                status.ok()?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autostart_roundtrip_in_isolated_registry_key() {
        let name = format!("Software\\CodeBeatTests\\{}", uuid::Uuid::new_v4());
        let subkey = wide(std::ffi::OsStr::new(&name));
        let subkey = PCWSTR(subkey.as_ptr());
        struct Cleanup(PCWSTR);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                unsafe {
                    let _ = RegDeleteTreeW(HKEY_CURRENT_USER, self.0);
                }
            }
        }
        let _cleanup = Cleanup(subkey);
        let exe = Path::new("C:\\含空格 路径\\codebeat.exe");
        set_autostart(subkey, false, exe).unwrap();
        set_autostart(subkey, true, exe).unwrap();
        set_autostart(subkey, true, exe).unwrap();
        unsafe {
            let mut key = HKEY::default();
            RegOpenKeyExW(HKEY_CURRENT_USER, subkey, None, KEY_QUERY_VALUE, &mut key)
                .ok()
                .unwrap();
            let key = Key(key);
            let mut data = [0u16; 512];
            let mut size = std::mem::size_of_val(&data) as u32;
            let mut kind = REG_VALUE_TYPE::default();
            RegQueryValueExW(
                key.0,
                windows::core::w!("codebeat"),
                None,
                Some(&mut kind),
                Some(data.as_mut_ptr().cast()),
                Some(&mut size),
            )
            .ok()
            .unwrap();
            assert_eq!(kind, REG_SZ);
            assert_eq!(
                String::from_utf16_lossy(&data[..size as usize / 2 - 1]),
                format!("\"{}\" --background", exe.display())
            );
        }
        set_autostart(subkey, false, exe).unwrap();
        set_autostart(subkey, false, exe).unwrap();
    }
}
