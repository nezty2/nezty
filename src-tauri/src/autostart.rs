//! Starten met Windows. nezty draait altijd met administratorrechten. Windows slaat zulke programma's
//! over in de gewone Run-sleutel, dus de opstart loopt via een geplande taak bij het inloggen.
//! Oudere versies gebruikten de Run-sleutel: die waarde wordt hier eenmalig omgezet.
//!
//! Dezelfde geplande-takenlogica dient voor de stille licentiecontrole bij het inloggen (`--license-check`).
//! De stille taak controleert elke 15 minuten zolang het account in Windows is aangemeld.

use crate::error::{AppError, AppResult};
use std::os::windows::process::CommandExt;
use std::process::Command;
use winreg::enums::*;
use winreg::RegKey;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const TASK: &str = "nezty autostart";
/// Controleert bij het inloggen stil de licentie en zet zo nodig alles terug. Doet verder niets.
pub const LICENSE_TASK: &str = "nezty licentiecontrole";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const AUTOSTART_ARGS: &str = "--minimized";
const LICENSE_ARGS: &str = "--license-check";
/// Vertraging na het inloggen.
const LOGON_DELAY: &str = "PT45S";
/// Hier houdt de Taakplanner bij welke taken er zijn (leesbaar met beheerdersrechten, zonder schtasks te starten).
const TASK_TREE: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Schedule\TaskCache\Tree";

fn schtasks(args: &[&str]) -> AppResult<std::process::Output> {
    Ok(Command::new(crate::powershell::system_tool("schtasks.exe")?).args(args).creation_flags(CREATE_NO_WINDOW).output()?)
}

pub fn is_enabled() -> bool {
    task_exists(TASK)
}

fn task_name(name: &str) -> AppResult<String> {
    if name == LICENSE_TASK { Ok(format!("{name} {}", crate::protected::current_sid()?)) } else { Ok(name.into()) }
}

pub fn task_exists(name: &str) -> bool {
    let Ok(name) = task_name(name) else { return false };
    task_exists_named(&name)
}

fn task_exists_named(name: &str) -> bool {
    match RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(format!("{TASK_TREE}\\{name}"), KEY_READ) {
        Ok(_) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        // Geen leesrechten op de Taakplanner-gegevens: dan toch via schtasks vragen.
        Err(_) => schtasks(&["/Query", "/TN", name]).map(|o| o.status.success()).unwrap_or(false),
    }
}

/// Is de taak van deze versie (met vertraging en naar deze nezty.exe)? Taken van 0.4.0 en ouder hebben geen
/// vertraging. Een onleesbare definitie wordt vanuit de beveiligde installatie vervangen.
fn is_current(name: &str) -> bool {
    let Ok(tasks) = crate::powershell::system_tool("Tasks") else { return false };
    let Ok(bytes) = std::fs::read(tasks.join(task_name(name).unwrap_or_default())) else { return false };
    // De Taakplanner bewaart het bestand als UTF-16 (met BOM). Anders als UTF-8 lezen.
    let xml = if bytes.starts_with(&[0xFF, 0xFE]) {
        let units: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(&bytes).to_string()
    }
    .to_lowercase();
    let Ok(exe) = crate::protected::task_exe() else { return false };
    (name != LICENSE_TASK || xml.contains("<interval>pt15m</interval>")) &&
        xml.contains(&format!("<delay>{}</delay>", LOGON_DELAY.to_lowercase())) &&
        xml.contains(&format!("<command>{}</command>", escape(&exe.to_string_lossy()).to_lowercase()))
}

/// Korte uitleg in de Taakplanner, in de taal van Windows.
fn description(args: &str) -> &'static str {
    let lang = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() } & 0x3ff;
    match (lang, args == LICENSE_ARGS) {
        (0x13, false) => "Start nezty na het inloggen in Windows.",
        (0x13, true) => "Controleert na het inloggen in Windows de licentie van nezty.",
        (0x0c, false) => "Lance nezty après la connexion à Windows.",
        (0x0c, true) => "Vérifie la licence de nezty après la connexion à Windows.",
        (_, false) => "Starts nezty after you sign in to Windows.",
        (_, true) => "Checks the nezty license after you sign in to Windows.",
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn task_xml(exe: &str, user: &str, args: &str) -> String {
    let periodic = if args == LICENSE_ARGS {
        r#"<TimeTrigger><Repetition><Interval>PT15M</Interval></Repetition><StartBoundary>2025-01-01T00:00:00Z</StartBoundary><Enabled>true</Enabled></TimeTrigger>"#
    } else { "" };
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Author>Nezty</Author>
    <Description>{description}</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
      <Delay>{delay}</Delay>
    </LogonTrigger>
    {periodic}
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
      <Arguments>{args}</Arguments>
    </Exec>
  </Actions>
</Task>
"#,
        user = escape(user),
        exe = escape(exe),
        args = escape(args),
        delay = LOGON_DELAY,
        description = escape(description(args))
    )
}

/// Maakt (of vervangt) een geplande taak bij het inloggen die nezty.exe met deze argumenten start.
pub fn create_task(name: &str, args: &str) -> AppResult<()> {
    let exe = crate::protected::task_exe()?.to_string_lossy().to_string();
    let user = crate::protected::current_sid()?;
    let xml = task_xml(&exe, &user, args);
    // schtasks leest het bestand als UTF-16 met BOM.
    let mut bytes: Vec<u8> = vec![0xFF, 0xFE];
    for u in xml.encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    // In de eigen gegevensmap (niet %TEMP%) en meteen weer weg.
    let dir = crate::protected::recovery_dir()?;
    let path = dir.join(format!("task-{}.xml", uuid::Uuid::new_v4()));
    {
        use std::io::Write;
        let mut file = std::fs::File::create_new(&path)?;
        crate::protected::check(&path)?;
        file.write_all(&bytes)?;
    }
    let registered_name = task_name(name)?;
    let result = schtasks(&["/Create", "/TN", &registered_name, "/XML", &path.to_string_lossy(), "/F"]);
    let _ = std::fs::remove_file(&path);
    let out = result?;
    if !out.status.success() {
        return Err(AppError::msg(format!("schtasks /Create failed: {}", String::from_utf8_lossy(&out.stderr).trim())));
    }
    Ok(())
}

pub fn delete_task(name: &str) -> AppResult<()> {
    delete_named(&task_name(name)?)
}

fn delete_named(name: &str) -> AppResult<()> {
    if !task_exists_named(name) {
        return Ok(());
    }
    let out = schtasks(&["/Delete", "/TN", name, "/F"])?;
    if !out.status.success() {
        return Err(AppError::msg(format!("schtasks /Delete failed: {}", String::from_utf8_lossy(&out.stderr).trim())));
    }
    Ok(())
}

/// Starten met Windows aan of uit. Aan: de licentiecontrole zit in die start, de losse taak gaat weg.
/// Uit: de losse taak voor de licentiecontrole komt terug (alleen als de licentie aan staat bij het bouwen).
pub fn set(enabled: bool) -> AppResult<()> {
    ensure_license_task()?;
    if enabled { create_task(TASK, AUTOSTART_ARGS)?; } else { delete_task(TASK)?; }
    remove_run_value();
    Ok(())
}

fn ensure_license_task() -> AppResult<()> {
    if !is_current(LICENSE_TASK) { create_task(LICENSE_TASK, LICENSE_ARGS)?; }
    Ok(())
}

pub fn sync_tasks() {
    let result = ensure_license_task().and_then(|_| {
        if task_exists(TASK) && !is_current(TASK) { create_task(TASK, AUTOSTART_ARGS)?; }
        Ok(())
    });
    if let Err(e) = result { log::warn!("geplande taken bijwerken: {e}"); }
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

/// Alles van nezty uit de Taakplanner en de Run-sleutel. Voor het verwijderen van de app.
pub fn remove_all() -> AppResult<()> {
    delete_task(TASK)?;
    delete_named(LICENSE_TASK)?; // Naam van vóór de controles per Windows-account.
    let dir = crate::protected::recovery_dir()?;
    let root = dir.parent().ok_or_else(|| AppError::msg("Missing recovery directory"))?;
    for entry in std::fs::read_dir(root)? {
        let sid = entry?.file_name();
        if let Some(sid) = sid.to_str().filter(|s| s.starts_with("S-1-") && s[2..].bytes().all(|b| b.is_ascii_digit() || b == b'-')) {
            delete_named(&format!("{LICENSE_TASK} {sid}"))?;
        }
    }
    remove_run_value();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_app_has_periodic_license_checks_and_no_periodic_gui_launch() {
        let check = task_xml("nezty.exe", "S-1-5-21-1", LICENSE_ARGS);
        assert!(check.contains("<Interval>PT15M</Interval>") && check.contains("<Arguments>--license-check</Arguments>"));
        assert!(!task_xml("nezty.exe", "S-1-5-21-1", AUTOSTART_ARGS).contains("<TimeTrigger>"));
        let xml = task_xml("nezty.exe", &crate::protected::current_sid().unwrap(), LICENSE_ARGS);
        let script = format!("$s = New-Object -ComObject Schedule.Service; $s.Connect(); $t = $s.NewTask(0); $t.XmlText = {}; $t.Triggers.Count",
            crate::powershell::ps_quote(&xml));
        assert_eq!(crate::powershell::run_ps(&script).unwrap().trim(), "2");
    }
}
