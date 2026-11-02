//! Starten met Windows. nezty draait altijd met administratorrechten. Windows slaat zulke programma's
//! over in de gewone Run-sleutel, dus de opstart loopt via een geplande taak bij het inloggen.
//! Oudere versies gebruikten de Run-sleutel: die waarde wordt hier eenmalig omgezet.
//!
//! Dezelfde geplande-takenlogica dient voor de stille licentiecontrole bij het inloggen (`--license-check`).
//! Er staat altijd hooguit één taak: met starten met Windows aan doet "nezty autostart" ook de licentiecontrole
//! (de gewone start draait `license::enforce`), alleen met starten met Windows uit bestaat "nezty licentiecontrole".
//! Beide starten 45 seconden na het inloggen, zodat Windows en het netwerk eerst klaar zijn.
//! Het taakbestand (XML) staat heel even in de eigen gegevensmap en niet in %TEMP%.

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
    Ok(Command::new("schtasks.exe").args(args).creation_flags(CREATE_NO_WINDOW).output()?)
}

pub fn is_enabled() -> bool {
    task_exists(TASK)
}

pub fn task_exists(name: &str) -> bool {
    match RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(format!("{TASK_TREE}\\{name}"), KEY_READ) {
        Ok(_) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        // Geen leesrechten op de Taakplanner-gegevens: dan toch via schtasks vragen.
        Err(_) => schtasks(&["/Query", "/TN", name]).map(|o| o.status.success()).unwrap_or(false),
    }
}

/// Is de taak van deze versie (met vertraging en naar deze nezty.exe)? Taken van 0.4.0 en ouder hebben geen
/// vertraging. Is het taakbestand niet leesbaar, dan geldt hij als goed (niet bij elke start opnieuw maken).
fn is_current(name: &str) -> bool {
    let windir = std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into());
    let Ok(bytes) = std::fs::read(windir.join("System32\\Tasks").join(name)) else { return true };
    // De Taakplanner bewaart het bestand als UTF-16 (met BOM). Anders als UTF-8 lezen.
    let xml = if bytes.starts_with(&[0xFF, 0xFE]) {
        let units: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(&bytes).to_string()
    }
    .to_lowercase();
    let exe = std::env::current_exe().map(|p| escape(&p.to_string_lossy()).to_lowercase()).unwrap_or_default();
    xml.contains(&format!("<delay>{}</delay>", LOGON_DELAY.to_lowercase())) && (exe.is_empty() || xml.contains(&exe))
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
    let exe = std::env::current_exe()?.to_string_lossy().to_string();
    let domain = std::env::var("USERDOMAIN").unwrap_or_default();
    let user_name = std::env::var("USERNAME").unwrap_or_default();
    let user = if domain.is_empty() { user_name } else { format!("{domain}\\{user_name}") };
    let xml = task_xml(&exe, &user, args);
    // schtasks leest het bestand als UTF-16 met BOM.
    let mut bytes: Vec<u8> = vec![0xFF, 0xFE];
    for u in xml.encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    // In de eigen gegevensmap (niet %TEMP%) en meteen weer weg.
    let dir = crate::standalone_data_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("nezty-task-{}.xml", name.replace(' ', "-")));
    std::fs::write(&path, bytes)?;
    let result = schtasks(&["/Create", "/TN", name, "/XML", &path.to_string_lossy(), "/F"]);
    let _ = std::fs::remove_file(&path);
    let out = result?;
    if !out.status.success() {
        return Err(AppError::msg(format!("schtasks /Create failed: {}", String::from_utf8_lossy(&out.stderr).trim())));
    }
    Ok(())
}

pub fn delete_task(name: &str) -> AppResult<()> {
    if !task_exists(name) {
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
    if !enabled {
        delete_task(TASK)?;
        remove_run_value();
        if crate::license::enabled() {
            if let Err(e) = create_task(LICENSE_TASK, LICENSE_ARGS) {
                log::warn!("taak licentiecontrole maken: {e}");
            }
        }
        return Ok(());
    }
    create_task(TASK, AUTOSTART_ARGS)?;
    remove_run_value();
    if let Err(e) = delete_task(LICENSE_TASK) {
        log::warn!("taak licentiecontrole verwijderen: {e}");
    }
    Ok(())
}

/// Bij het starten van de app: precies één taak bij het inloggen, van deze versie. Een taak van een oudere versie
/// (zonder vertraging of naar een ander pad) wordt één keer opnieuw gemaakt. Start schtasks alleen als er iets
/// te veranderen is.
pub fn sync_tasks() {
    let result = if task_exists(TASK) {
        let mut r = Ok(());
        if !is_current(TASK) {
            r = create_task(TASK, AUTOSTART_ARGS);
        }
        r.and(delete_task(LICENSE_TASK))
    } else if crate::license::enabled() {
        if task_exists(LICENSE_TASK) && is_current(LICENSE_TASK) {
            Ok(())
        } else {
            create_task(LICENSE_TASK, LICENSE_ARGS)
        }
    } else {
        delete_task(LICENSE_TASK)
    };
    if let Err(e) = result {
        log::warn!("geplande taken bijwerken: {e}");
    }
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
pub fn remove_all() {
    for name in [TASK, LICENSE_TASK] {
        if let Err(e) = delete_task(name) {
            log::warn!("taak {name} verwijderen: {e}");
        }
    }
    remove_run_value();
}
