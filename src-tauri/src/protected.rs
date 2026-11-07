//! Trusted locations shared by the elevated task and the recovery journal.
use crate::error::{AppError, AppResult};
use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{LocalFree, GENERIC_ALL, GENERIC_WRITE};
use windows_sys::Win32::Security::Authorization::{ConvertSidToStringSidW, GetNamedSecurityInfoW, SE_FILE_OBJECT};
use windows_sys::Win32::Security::*;
use windows_sys::Win32::Storage::FileSystem::{
    DELETE, FILE_APPEND_DATA, FILE_ATTRIBUTE_REPARSE_POINT, FILE_DELETE_CHILD, FILE_WRITE_ATTRIBUTES, FILE_WRITE_DATA,
    FILE_WRITE_EA, WRITE_DAC, WRITE_OWNER,
};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::UI::Shell::{FOLDERID_ProgramFilesX64, SHGetKnownFolderPath};

unsafe fn wide_string(ptr: *const u16) -> OsString {
    let mut len = 0;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    OsString::from_wide(std::slice::from_raw_parts(ptr, len))
}

pub fn known_folder(id: &windows_sys::core::GUID) -> AppResult<PathBuf> {
    let mut path = null_mut();
    unsafe {
        if SHGetKnownFolderPath(id, 0, null_mut(), &mut path) < 0 {
            return Err(AppError::msg("Windows folder unavailable"));
        }
        let dir = PathBuf::from(wide_string(path));
        CoTaskMemFree(path.cast());
        Ok(dir)
    }
}

fn program_files() -> AppResult<PathBuf> {
    let dir = known_folder(&FOLDERID_ProgramFilesX64)?;
    check(&dir)?;
    Ok(dir)
}

fn sid_string(sid: PSID) -> AppResult<String> {
    let mut ptr = null_mut();
    unsafe {
        if ConvertSidToStringSidW(sid, &mut ptr) == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let value = wide_string(ptr).to_string_lossy().into_owned();
        LocalFree(ptr.cast());
        Ok(value)
    }
}

pub fn current_sid() -> AppResult<String> {
    // GetCurrentProcessToken() is the Windows pseudo handle -4.
    let token = -4isize as windows_sys::Win32::Foundation::HANDLE;
    let mut buffer = [0usize; 64];
    let mut needed = 0;
    unsafe {
        if GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            std::mem::size_of_val(&buffer) as u32,
            &mut needed,
        ) == 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        sid_string((*(buffer.as_ptr().cast::<TOKEN_USER>())).User.Sid)
    }
}

fn privileged(sid: PSID) -> AppResult<bool> {
    // SYSTEM, Administrators, TrustedInstaller.
    Ok(matches!(
        sid_string(sid)?.as_str(),
        "S-1-5-18" | "S-1-5-32-544" | "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464"
    ))
}

unsafe fn protected_acl(owner: PSID, acl: *const ACL) -> AppResult<bool> {
    const WRITE_RIGHTS: u32 = GENERIC_WRITE
        | GENERIC_ALL
        | DELETE
        | WRITE_DAC
        | WRITE_OWNER
        | FILE_APPEND_DATA
        | FILE_DELETE_CHILD
        | FILE_WRITE_ATTRIBUTES
        | FILE_WRITE_DATA
        | FILE_WRITE_EA;
    if acl.is_null() || !privileged(owner)? {
        return Ok(false);
    }
    for index in 0..(*acl).AceCount {
        let mut ace = null_mut();
        if GetAce(acl, index.into(), &mut ace) == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let header = &*ace.cast::<ACE_HEADER>();
        if header.AceType == 1 {
            // ACCESS_DENIED_ACE
            continue;
        }
        if header.AceType != 0 {
            // Only ordinary ACCESS_ALLOWED_ACE grants are supported.
            return Ok(false);
        }
        let grant = &*ace.cast::<ACCESS_ALLOWED_ACE>();
        let sid = (&grant.SidStart as *const u32).cast_mut().cast();
        if grant.Mask & WRITE_RIGHTS == 0 || privileged(sid)? {
            continue;
        }
        // CREATOR OWNER on future children is safe: each ancestor already
        // prevents ordinary users from creating children, and we check ownership.
        if header.AceFlags as u32 & INHERIT_ONLY_ACE == 0 || sid_string(sid)? != "S-1-3-0" {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn check(path: &Path) -> AppResult<()> {
    let insecure = || AppError::msg(format!("Beheerdersbeveiliging ontbreekt op {}", path.display()));
    if std::fs::symlink_metadata(path)?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(insecure());
    }
    let wide: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let (mut owner, mut acl, mut descriptor) = (null_mut(), null_mut(), null_mut());
    unsafe {
        let err = GetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            &mut acl,
            null_mut(),
            &mut descriptor,
        );
        if err != 0 {
            return Err(std::io::Error::from_raw_os_error(err as i32).into());
        }
        let result = protected_acl(owner, acl);
        LocalFree(descriptor);
        if !result? {
            return Err(insecure());
        }
    }
    Ok(())
}

pub fn task_exe() -> AppResult<PathBuf> {
    let dir = program_files()?.join("nezty");
    let expected = dir.join("nezty.exe");
    if !std::env::current_exe()?
        .as_os_str()
        .eq_ignore_ascii_case(expected.as_os_str())
    {
        return Err(AppError::msg(
            "Installeer Nezty in Program Files om starten met Windows te gebruiken.",
        ));
    }
    check(&dir)?;
    check(&expected)?;
    Ok(expected)
}

pub fn recovery_dir() -> AppResult<PathBuf> {
    let mut dir = program_files()?;
    for name in ["Nezty Recovery", &current_sid()?] {
        dir.push(name);
        match std::fs::create_dir(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
        check(&dir)?;
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_real_windows_path_and_refuses_workspace_task() {
        assert!(program_files().unwrap().is_absolute());
        assert!(current_sid().unwrap().starts_with("S-1-"));
        assert!(task_exe().is_err());
        let temp = std::env::temp_dir().join(format!("nezty-acl-{}", uuid::Uuid::new_v4()));
        std::fs::File::create_new(&temp).unwrap();
        // Even a file created by an elevated test runner can have a writable
        // per-user ACL. Explicitly grant the current user the write right.
        let sid = format!("*{}:(M)", current_sid().unwrap());
        let grant = std::process::Command::new(crate::powershell::system_tool("icacls.exe").unwrap())
            .arg(&temp).args(["/grant", &sid]).output().unwrap();
        assert!(grant.status.success());
        assert!(check(&temp).is_err());
        std::fs::remove_file(temp).unwrap();
    }
}
