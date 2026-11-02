//! Geheugenbewaker, de "FPS Boost" van ExitLag maar alleen als het nodig is. Windows bewaart bestanden in de
//! standby-cache. Is het echt vrije geheugen bijna op, dan moet Windows tijdens het gamen eerst cachepagina's
//! vrijmaken, en dat geeft op sommige pc's haperingen. De bewaker leegt dan de standby-lijst met
//! NtSetSystemInformation (MemoryPurgeStandbyList), dezelfde aanroep als RAMMap van Microsoft Sysinternals.
//! Alleen tijdens een game, alleen onder de grens (minder dan 1 GB vrij en meer dan 1 GB standby) en hooguit eens per
//! 30 seconden. Met genoeg vrij geheugen doet hij niets: legen gooit dan alleen cache weg die Windows straks
//! opnieuw van schijf moet lezen.

use crate::error::{AppError, AppResult};
use crate::models::{now_ms, MemGuardStatus, MemStats, PurgeResult};
use crate::state::{lock, AppState};
use crate::winapi::wide;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use wmi::{Variant, WMIConnection};

const FREE_LIMIT_MB: u64 = 1024;
const STANDBY_MIN_MB: u64 = 1024;
const MIN_GAP: Duration = Duration::from_secs(30);
const MB: u64 = 1024 * 1024;
/// SYSTEM_INFORMATION_CLASS SystemMemoryListInformation en het commando MemoryPurgeStandbyList.
const SYSTEM_MEMORY_LIST_INFORMATION: u32 = 80;
const MEMORY_PURGE_STANDBY_LIST: u32 = 4;
const STATUS_PRIVILEGE_NOT_HELD: u32 = 0xC000_0061;

static PURGES: AtomicU32 = AtomicU32::new(0);
/// Laatste keer legen: (tijdstip in ms, vrijgekomen MB, Instant voor de wachttijd).
static LAST: Mutex<Option<(i64, u64, Instant)>> = Mutex::new(None);

fn variant_u64(v: &Variant) -> Option<u64> {
    match v {
        Variant::UI8(x) => Some(*x),
        Variant::UI4(x) => Some(*x as u64),
        Variant::I8(x) => u64::try_from(*x).ok(),
        Variant::I4(x) => u64::try_from(*x).ok(),
        Variant::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn total_mb() -> u64 {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    unsafe {
        let mut m: MEMORYSTATUSEX = std::mem::zeroed();
        m.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        if GlobalMemoryStatusEx(&mut m) == 0 {
            return 0;
        }
        m.ullTotalPhys / MB
    }
}

/// Geheugen nu, uit de prestatiemeters van Windows (werkt ook zonder beheerdersrechten).
pub fn stats() -> Option<MemStats> {
    let conn = WMIConnection::new().ok()?;
    let rows: Vec<HashMap<String, Variant>> = conn
        .raw_query("SELECT AvailableBytes, FreeAndZeroPageListBytes, StandbyCacheCoreBytes, StandbyCacheNormalPriorityBytes, StandbyCacheReserveBytes FROM Win32_PerfFormattedData_PerfOS_Memory")
        .ok()?;
    let row = rows.first()?;
    let n = |k: &str| row.get(k).and_then(variant_u64).unwrap_or(0);
    Some(MemStats {
        total_mb: total_mb(),
        available_mb: n("AvailableBytes") / MB,
        free_mb: n("FreeAndZeroPageListBytes") / MB,
        standby_mb: (n("StandbyCacheCoreBytes") + n("StandbyCacheNormalPriorityBytes") + n("StandbyCacheReserveBytes")) / MB,
    })
}

fn needed(m: &MemStats) -> bool {
    m.free_mb < FREE_LIMIT_MB && m.standby_mb > STANDBY_MIN_MB
}

/// Zet een recht in het token van nezty aan (beheerders hebben het, maar het staat standaard uit).
fn enable_privilege(name: &str) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, LUID};
    use windows_sys::Win32::Security::{AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut luid = LUID { LowPart: 0, HighPart: 0 };
        let w = wide(name);
        let ok = LookupPrivilegeValueW(std::ptr::null(), w.as_ptr(), &mut luid) != 0 && {
            let tp = TOKEN_PRIVILEGES { PrivilegeCount: 1, Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }] };
            // Ook bij succes kan het recht ontbreken (ERROR_NOT_ALL_ASSIGNED): dat zegt GetLastError.
            AdjustTokenPrivileges(token, 0, &tp, 0, std::ptr::null_mut(), std::ptr::null_mut()) != 0 && GetLastError() == 0
        };
        CloseHandle(token);
        ok
    }
}

fn purge_standby() -> AppResult<()> {
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    if !enable_privilege("SeProfileSingleProcessPrivilege") {
        return Err(AppError::NeedsAdmin("SeProfileSingleProcessPrivilege".into()));
    }
    type NtSetSystemInformation = unsafe extern "system" fn(u32, *mut c_void, u32) -> i32;
    unsafe {
        let ntdll = GetModuleHandleW(wide("ntdll.dll").as_ptr());
        let Some(f) = GetProcAddress(ntdll, c"NtSetSystemInformation".as_ptr() as *const u8) else {
            return Err(AppError::msg("NtSetSystemInformation not found"));
        };
        let f: NtSetSystemInformation = std::mem::transmute(f);
        let mut command = MEMORY_PURGE_STANDBY_LIST;
        match f(SYSTEM_MEMORY_LIST_INFORMATION, &mut command as *mut u32 as *mut c_void, 4) as u32 {
            0 => Ok(()),
            STATUS_PRIVILEGE_NOT_HELD => Err(AppError::NeedsAdmin("SeProfileSingleProcessPrivilege".into())),
            s => Err(AppError::msg(format!("NtSetSystemInformation: 0x{s:08X}"))),
        }
    }
}

fn record(before: &MemStats, after: &MemStats) {
    PURGES.fetch_add(1, Ordering::Relaxed);
    *lock(&LAST) = Some((now_ms(), after.free_mb.saturating_sub(before.free_mb), Instant::now()));
}

/// Nu legen (de knop), met de cijfers ervoor en erna.
pub fn purge() -> AppResult<PurgeResult> {
    let before = stats().ok_or_else(|| AppError::msg("memory counters unavailable"))?;
    purge_standby()?;
    std::thread::sleep(Duration::from_millis(400));
    let after = stats().unwrap_or(before);
    record(&before, &after);
    Ok(PurgeResult { before, after })
}

pub fn status(state: &AppState) -> MemGuardStatus {
    let stats = stats();
    let last = *lock(&LAST);
    MemGuardStatus {
        enabled: state.settings_clone().mem_guard,
        stats,
        purges: PURGES.load(Ordering::Relaxed),
        last_purge_ts: last.map(|l| l.0),
        last_freed_mb: last.map(|l| l.1),
        game_running: !lock(&state.running_games).is_empty(),
        needed: stats.as_ref().is_some_and(needed),
        elevated: state.elevated,
    }
}

/// De bewaker: elke 3 seconden kijken, alleen als hij aan staat en er een game draait.
pub fn start(app: AppHandle, state: Arc<AppState>) {
    let _ = std::thread::Builder::new().name("memguard".into()).spawn(move || loop {
        std::thread::sleep(Duration::from_secs(3));
        if !state.settings_clone().mem_guard || !state.elevated || lock(&state.running_games).is_empty() || crate::license::require(&state).is_err() {
            continue;
        }
        if lock(&LAST).is_some_and(|l| l.2.elapsed() < MIN_GAP) {
            continue;
        }
        let Some(before) = stats() else { continue };
        if !needed(&before) {
            continue;
        }
        match purge_standby() {
            Ok(()) => {
                std::thread::sleep(Duration::from_millis(400));
                let after = stats().unwrap_or(before);
                record(&before, &after);
                log::info!("geheugenbewaker: standby geleegd ({} MB vrij, nu {} MB)", before.free_mb, after.free_mb);
                let _ = app.emit("memguard", status(&state));
            }
            Err(e) => log::warn!("geheugenbewaker: legen mislukt: {e}"),
        }
    });
}
