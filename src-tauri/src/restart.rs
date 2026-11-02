//! Herstart van de pc: welke tweaks wachten nog op een herstart, en de pc echt opnieuw opstarten.
//!
//! Herkennen gaat met een vluchtige registersleutel (REG_OPTION_VOLATILE) per tweak. Windows houdt zo'n sleutel
//! alleen in het geheugen bij: na een echte herstart is hij weg. Bij "Afsluiten" met Snel opstarten blijft hij
//! bestaan, en dat klopt: dan laadt Windows de kernel niet opnieuw en werkt de tweak ook nog niet.
//! De sleutel staat in HKLM, niet in HKCU: een vluchtige sleutel in HKCU verdwijnt al bij afmelden.
//! Lukt het maken niet, dan vergelijkt nezty zoals vroeger de opstarttijd van Windows met het tijdstip van de wijziging.

use crate::error::{AppError, AppResult};
use crate::models::{RestartStatus, Timing, TweakScope};
use crate::registry::{self, Hive, RegData};
use crate::state::{lock, AppState};
use crate::tweaks;
use crate::winapi::wide;
use std::ptr::{null, null_mut};
use windows_sys::Win32::System::Registry::{RegCloseKey, RegCreateKeyExW, HKEY, HKEY_LOCAL_MACHINE, KEY_WRITE, REG_OPTION_VOLATILE};

/// Vluchtige sleutel (bestaat alleen tot de volgende herstart), met daaronder één sleutel per tweak.
const MARKER_ROOT: &str = "SOFTWARE\\nezty-pending-restart";

fn create_volatile(path: &str) -> bool {
    let w = wide(path);
    let mut key: HKEY = null_mut();
    let rc = unsafe { RegCreateKeyExW(HKEY_LOCAL_MACHINE, w.as_ptr(), 0, null(), REG_OPTION_VOLATILE, KEY_WRITE, null(), &mut key, null_mut()) };
    if rc != 0 {
        return false;
    }
    unsafe { RegCloseKey(key) };
    true
}

/// Maakt de markering voor een tweak. Geeft false als dat niet lukt (bv. zonder beheerdersrechten).
fn mark(tweak_id: &str) -> bool {
    create_volatile(MARKER_ROOT) && create_volatile(&format!("{MARKER_ROOT}\\{tweak_id}"))
}

fn marker_exists(tweak_id: &str) -> bool {
    Hive::Hklm.root().open_subkey(format!("{MARKER_ROOT}\\{tweak_id}")).is_ok()
}

/// Haalt de markering weg (de tweak is uitgezet).
pub fn clear_marker(tweak_id: &str) {
    let _ = Hive::Hklm.root().delete_subkey(format!("{MARKER_ROOT}\\{tweak_id}"));
}

/// Na het toepassen van een tweak die een herstart nodig heeft: zijn er nieuwe logboekregels (vanaf `from`),
/// dan een markering maken en dat bij die regels onthouden.
pub fn mark_new_entries(state: &AppState, tweak_id: &str, from: usize) {
    let has_new = lock(&state.journal).entries.get(from..).is_some_and(|new| new.iter().any(|e| e.tweak_id == tweak_id));
    if !has_new || !mark(tweak_id) {
        return;
    }
    let mut j = lock(&state.journal);
    if let Some(new) = j.entries.get_mut(from..) {
        for e in new.iter_mut().filter(|e| e.tweak_id == tweak_id) {
            e.restart_marker = true;
        }
    }
    if let Err(e) = j.save() {
        log::error!("logboek opslaan mislukt: {e}");
    }
}

/// Wacht deze (aangezette) tweak nog op een herstart van de pc?
pub fn pending(state: &AppState, tweak_id: &str) -> bool {
    let latest = lock(&state.journal).latest_active(tweak_id, None).map(|e| (e.ts, e.restart_marker));
    match latest {
        None => false,
        Some((_, true)) => marker_exists(tweak_id),
        // Oudere wijziging zonder markering: dan de opstarttijd van Windows.
        Some((ts, false)) => sysinfo::System::boot_time() <= (ts / 1000) as u64,
    }
}

/// Staat Snel opstarten aan? Dan start "Afsluiten" Windows niet echt opnieuw.
pub fn fast_startup() -> bool {
    matches!(registry::read(Hive::Hklm, "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Power", "HiberbootEnabled"), Ok(RegData::Dword(1)))
}

/// Eén vlag voor de hele app: moet de pc herstarten, en voor welke tweaks.
pub fn status(state: &AppState) -> RestartStatus {
    let waiting: Vec<String> = tweaks::CATALOG
        .iter()
        .filter(|d| d.timing == Timing::PcRestart && d.scope == TweakScope::Global)
        .filter(|d| tweaks::info(state, d, None).active.as_deref() == Some("after_pc_restart"))
        .map(|d| d.id.to_string())
        .collect();
    RestartStatus { required: !waiting.is_empty(), tweaks: waiting, fast_startup: fast_startup() }
}

/// Start Windows opnieuw op (een echte herstart, ook met Snel opstarten). Daarvoor moet het proces het
/// afsluitrecht (SeShutdownPrivilege) eerst aanzetten.
pub fn restart_pc() -> AppResult<()> {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, LUID};
    use windows_sys::Win32::Security::{
        AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED, SE_SHUTDOWN_NAME, TOKEN_ADJUST_PRIVILEGES,
        TOKEN_PRIVILEGES, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Shutdown::{ExitWindowsEx, EWX_REBOOT, SHTDN_REASON_FLAG_PLANNED, SHTDN_REASON_MAJOR_APPLICATION, SHTDN_REASON_MINOR_RECONFIG};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) == 0 {
            return Err(AppError::msg(format!("OpenProcessToken: {}", GetLastError())));
        }
        let mut luid = LUID { LowPart: 0, HighPart: 0 };
        if LookupPrivilegeValueW(null(), SE_SHUTDOWN_NAME, &mut luid) == 0 {
            let err = GetLastError();
            CloseHandle(token);
            return Err(AppError::msg(format!("LookupPrivilegeValue: {err}")));
        }
        let tp = TOKEN_PRIVILEGES { PrivilegeCount: 1, Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }] };
        let ok = AdjustTokenPrivileges(token, 0, &tp, 0, null_mut(), null_mut());
        // Ook bij succes kan het recht ontbreken (ERROR_NOT_ALL_ASSIGNED): dat zegt GetLastError.
        let err = GetLastError();
        CloseHandle(token);
        if ok == 0 || err != 0 {
            return Err(AppError::msg(format!("AdjustTokenPrivileges: {err}")));
        }
        if ExitWindowsEx(EWX_REBOOT, SHTDN_REASON_MAJOR_APPLICATION | SHTDN_REASON_MINOR_RECONFIG | SHTDN_REASON_FLAG_PLANNED) == 0 {
            return Err(AppError::msg(format!("ExitWindowsEx: {}", GetLastError())));
        }
    }
    Ok(())
}
