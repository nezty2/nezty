//! Starten met Windows. nezty draait altijd met administratorrechten. Windows slaat zulke programma's
//! over in de gewone Run-sleutel, dus de opstart loopt via een geplande taak bij het inloggen.
//! Oudere versies gebruikten de Run-sleutel: die waarde wordt hier eenmalig omgezet.

use crate::error::{AppError, AppResult};
use std::os::windows::process::CommandExt;
use std::process::Command;
use winreg::enums::*;
use winreg::RegKey;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const TASK: &str = "nezty autostart";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

fn schtasks(args: &[&str]) -> AppResult<std::process::Output> {
    Ok(Command::new("schtasks.exe").args(args).creation_flags(CREATE_NO_WINDOW).output()?)
}

pub fn is_enabled() -> bool {
    schtasks(&["/Query", "/TN", TASK]).map(|o| o.status.success()).unwrap_or(false)
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn task_xml(exe: &str, user: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <StartWhenAvailable>true</StartWhenAvailable>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
      <Arguments>--minimized</Arguments>
    </Exec>
  </Actions>
</Task>
"#,
        user = escape(user),
        exe = escape(exe)
    )
}

pub fn set(enabled: bool) -> AppResult<()> {
    if !enabled {
        if is_enabled() {
            let out = schtasks(&["/Delete", "/TN", TASK, "/F"])?;
            if !out.status.success() {
                return Err(AppError::msg(format!("schtasks /Delete failed: {}", String::from_utf8_lossy(&out.stderr).trim())));
            }
        }
        remove_run_value();
        return Ok(());
    }
    let exe = std::env::current_exe()?.to_string_lossy().to_string();
    let domain = std::env::var("USERDOMAIN").unwrap_or_default();
    let name = std::env::var("USERNAME").unwrap_or_default();
    let user = if domain.is_empty() { name } else { format!("{domain}\\{name}") };
    let xml = task_xml(&exe, &user);
    // schtasks leest het bestand als UTF-16 met BOM.
    let mut bytes: Vec<u8> = vec![0xFF, 0xFE];
    for u in xml.encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    let path = std::env::temp_dir().join("nezty-autostart.xml");
    std::fs::write(&path, bytes)?;
    let result = schtasks(&["/Create", "/TN", TASK, "/XML", &path.to_string_lossy(), "/F"]);
    let _ = std::fs::remove_file(&path);
    let out = result?;
    if !out.status.success() {
        return Err(AppError::msg(format!("schtasks /Create failed: {}", String::from_utf8_lossy(&out.stderr).trim())));
    }
    remove_run_value();
    Ok(())
}

fn remove_run_value() {
    if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE) {
        let _ = key.delete_value("nezty");
    }
}

/// Zette starten met Windows uit de oude Run-sleutel (nezty en nzty) om naar de geplande taak.
pub fn migrate_old() {
    let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY, KEY_READ | KEY_SET_VALUE) else {
        return;
    };
    let had_nezty = key.get_value::<String, _>("nezty").is_ok();
    let had_nzty = key.get_value::<String, _>("nzty").is_ok();
    if !had_nezty && !had_nzty {
        return;
    }
    if had_nzty {
        let _ = key.delete_value("nzty");
    }
    if let Err(e) = set(true) {
        log::warn!("autostart migreren: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_has_highest_run_level_and_escapes() {
        let x = task_xml(r"C:\A&B\nezty.exe", r"PC\me");
        assert!(x.contains("HighestAvailable"));
        assert!(x.contains("C:\\A&amp;B\\nezty.exe"));
        assert!(x.contains("--minimized"));
    }
}
