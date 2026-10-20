//! 数据目录访问控制：关闭继承，仅授予当前用户可向子项继承的完全控制权限。
use super::wide;
use anyhow::Result;
use std::path::Path;
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::*,
        Security::{Authorization::*, *},
        Storage::FileSystem::FILE_ALL_ACCESS,
        System::Threading::*,
    },
};

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

struct LocalAllocation(*mut std::ffi::c_void);
impl Drop for LocalAllocation {
    fn drop(&mut self) {
        unsafe {
            let _ = LocalFree(Some(HLOCAL(self.0)));
        }
    }
}

pub(super) fn restrict_directory(path: &Path) -> Result<()> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)?;
        let token = Handle(token);
        let mut size = 0;
        let _ = GetTokenInformation(token.0, TokenUser, None, 0, &mut size);
        if size == 0 {
            anyhow::bail!("Unable to size current-user token");
        }
        // 使用 usize 缓冲区，保证 TOKEN_USER 所需的内存对齐。
        let mut storage = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(storage.as_mut_ptr().cast()),
            size,
            &mut size,
        )?;
        let user = &*storage.as_ptr().cast::<TOKEN_USER>();
        let access = EXPLICIT_ACCESS_W {
            grfAccessPermissions: FILE_ALL_ACCESS.0,
            grfAccessMode: SET_ACCESS,
            grfInheritance: CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE,
            Trustee: TRUSTEE_W {
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_USER,
                ptstrName: PWSTR(user.User.Sid.0.cast()),
                ..Default::default()
            },
        };
        let mut acl = std::ptr::null_mut();
        SetEntriesInAclW(Some(&[access]), None, &mut acl).ok()?;
        let _allocation = LocalAllocation(acl.cast());
        let path = wide(path.as_os_str());
        SetNamedSecurityInfoW(
            PCWSTR(path.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(acl),
            None,
        )
        .ok()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_acl_is_protected_current_user_only_and_inheritable() {
        let dir = tempfile::tempdir().unwrap();
        restrict_directory(dir.path()).unwrap();
        restrict_directory(dir.path()).unwrap();
        let child = dir.path().join("child");
        std::fs::create_dir(&child).unwrap();
        std::fs::write(child.join("data"), b"private").unwrap();
        unsafe {
            for path in [dir.path().to_path_buf(), child.clone(), child.join("data")] {
                let name = wide(path.as_os_str());
                let mut acl = std::ptr::null_mut();
                let mut descriptor = PSECURITY_DESCRIPTOR::default();
                GetNamedSecurityInfoW(
                    PCWSTR(name.as_ptr()),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    None,
                    None,
                    Some(&mut acl),
                    None,
                    &mut descriptor,
                )
                .ok()
                .unwrap();
                let _allocation = LocalAllocation(descriptor.0);
                assert!(!acl.is_null(), "A null DACL would allow everyone");
                assert_eq!((*acl).AceCount, 1);
                let mut ace = std::ptr::null_mut();
                GetAce(acl, 0, &mut ace).unwrap();
                let ace = &*ace.cast::<ACCESS_ALLOWED_ACE>();
                assert_eq!(ace.Header.AceType, 0 /* ACCESS_ALLOWED_ACE_TYPE */);
                assert_eq!(ace.Mask, FILE_ALL_ACCESS.0);
                // 验证权限授予当前用户，而非 Everyone 或 Administrators 用户组。
                let mut token = HANDLE::default();
                OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).unwrap();
                let token = Handle(token);
                let mut size = 0;
                let _ = GetTokenInformation(token.0, TokenUser, None, 0, &mut size);
                let mut buffer =
                    vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
                GetTokenInformation(
                    token.0,
                    TokenUser,
                    Some(buffer.as_mut_ptr().cast()),
                    size,
                    &mut size,
                )
                .unwrap();
                let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
                EqualSid(
                    PSID(std::ptr::addr_of!(ace.SidStart).cast_mut().cast()),
                    user.User.Sid,
                )
                .unwrap();
                if path == dir.path() {
                    let mut control = 0;
                    let mut revision = 0;
                    GetSecurityDescriptorControl(descriptor, &mut control, &mut revision).unwrap();
                    assert_ne!(control & SE_DACL_PROTECTED.0, 0);
                    assert_eq!(
                        u32::from(ace.Header.AceFlags)
                            & (CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE).0,
                        (CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE).0
                    );
                } else {
                    assert_ne!(u32::from(ace.Header.AceFlags) & INHERITED_ACE.0, 0);
                }
            }
        }
    }
}
