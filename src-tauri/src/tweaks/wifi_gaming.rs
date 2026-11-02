//! Wifi zonder pingpieken. Windows zoekt op de achtergrond regelmatig naar wifi-netwerken. Tijdens zo'n
//! zoekronde staat je verbinding even stil: dat zie je in games als een pingpiek, vaak elke minuut.
//! nezty zet via de officiële WLAN-API de achtergrondzoektocht uit en de streamingmodus aan.
//! Dat geldt alleen zolang nezty draait: sluit nezty, dan zet Windows het vanzelf terug.
//! Bij het opstarten van Windows is wifi vaak nog niet verbonden. Daarom probeert nezty het elke 15 seconden
//! opnieuw, en zet het na een nieuwe verbinding (ander netwerk, slaapstand) meteen weer goed.

use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, RevertPayload};
use crate::models::{JournalKind, Msg, TweakState};
use crate::state::{lock, AppState};
use crate::tweaks::{get, revert_via_journal, Status};
use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use windows_sys::core::GUID;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::NetworkManagement::WiFi::{
    wlan_intf_opcode_background_scan_enabled, wlan_intf_opcode_media_streaming_mode, WlanCloseHandle, WlanEnumInterfaces,
    WlanFreeMemory, WlanOpenHandle, WlanSetInterface, WLAN_INTERFACE_INFO_LIST,
};

/// Open WLAN-sessie. Zolang die open is, blijven de instellingen actief.
struct Session(HANDLE);
// De handle is een ondoorzichtige waarde van wlanapi; gebruik vanaf andere threads is toegestaan.
unsafe impl Send for Session {}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);
/// De gebruiker wil de wifi-gamestand aan (volgens het logboek). Alleen te wijzigen met SESSION op slot.
static WANTED: AtomicBool = AtomicBool::new(false);
/// Draait de bewaker al?
static KEEPER: AtomicBool = AtomicBool::new(false);
const KEEP_EVERY: Duration = Duration::from_secs(15);

fn open() -> Option<HANDLE> {
    let mut version = 0u32;
    let mut h: HANDLE = null_mut();
    let rc = unsafe { WlanOpenHandle(2, null(), &mut version, &mut h) };
    (rc == 0 && !h.is_null()).then_some(h)
}

fn interfaces(h: HANDLE) -> Vec<GUID> {
    let mut list: *mut WLAN_INTERFACE_INFO_LIST = null_mut();
    if unsafe { WlanEnumInterfaces(h, null(), &mut list) } != 0 || list.is_null() {
        return vec![];
    }
    let out = unsafe {
        let n = (*list).dwNumberOfItems as usize;
        let first = (*list).InterfaceInfo.as_ptr();
        (0..n).map(|i| (*first.add(i)).InterfaceGuid).collect()
    };
    unsafe { WlanFreeMemory(list as *const c_void) };
    out
}

/// Aantal wifi-adapters (0: geen wifi of de WLAN-service draait niet).
pub fn adapter_count() -> usize {
    let Some(h) = open() else { return 0 };
    let n = interfaces(h).len();
    unsafe { WlanCloseHandle(h, null()) };
    n
}

pub fn is_active() -> bool {
    SESSION.lock().map(|s| s.is_some()).unwrap_or(false)
}

/// Zet op elke wifi-adapter de achtergrondzoektocht uit en de streamingmodus aan. Geeft het aantal adapters
/// waarbij dat lukte (0 als er geen is, of als wifi nog niet verbonden is).
fn set_all(h: HANDLE) -> usize {
    let off: i32 = 0;
    let on: i32 = 1;
    let size = std::mem::size_of::<i32>() as u32;
    let mut done = 0usize;
    for g in &interfaces(h) {
        let a = unsafe { WlanSetInterface(h, g, wlan_intf_opcode_background_scan_enabled, size, &off as *const i32 as *const c_void, null()) };
        let b = unsafe { WlanSetInterface(h, g, wlan_intf_opcode_media_streaming_mode, size, &on as *const i32 as *const c_void, null()) };
        if a == 0 || b == 0 {
            done += 1;
        }
    }
    done
}

fn start_locked(guard: &mut Option<Session>) -> AppResult<usize> {
    if guard.is_some() {
        return Ok(adapter_count());
    }
    let h = open().ok_or_else(|| AppError::coded("wifi_none"))?;
    if interfaces(h).is_empty() {
        unsafe { WlanCloseHandle(h, null()) };
        return Err(AppError::coded("wifi_none"));
    }
    let done = set_all(h);
    if done == 0 {
        unsafe { WlanCloseHandle(h, null()) };
        return Err(AppError::msg("WlanSetInterface failed"));
    }
    *guard = Some(Session(h));
    Ok(done)
}

/// Zet de achtergrondzoektocht uit en de streamingmodus aan, op elke wifi-adapter. Geeft het aantal adapters terug.
pub fn start() -> AppResult<usize> {
    let mut guard = SESSION.lock().map_err(|_| AppError::msg("wifi session"))?;
    start_locked(&mut guard)
}

/// Sessie sluiten: Windows zet zoeken en streamingmodus dan zelf terug zoals ze waren. De bewaker stopt ook.
pub fn stop() {
    if let Ok(mut guard) = SESSION.lock() {
        WANTED.store(false, Ordering::SeqCst);
        if let Some(s) = guard.take() {
            unsafe { WlanCloseHandle(s.0, null()) };
        }
    }
}

/// Eén ronde van de bewaker: nog niet actief, dan opnieuw proberen. Wel actief, dan de instellingen opnieuw
/// zetten (na een nieuwe verbinding zet Windows het zoeken soms weer aan).
fn keep_once() -> AppResult<()> {
    let mut guard = SESSION.lock().map_err(|_| AppError::msg("wifi session"))?;
    if !WANTED.load(Ordering::SeqCst) {
        return Ok(());
    }
    match guard.as_ref() {
        Some(s) => {
            set_all(s.0);
            Ok(())
        }
        None => start_locked(&mut guard).map(|_| ()),
    }
}

/// Start de bewaker (één tegelijk). Hij stopt vanzelf zodra de wifi-gamestand uit gaat.
fn keep_active() {
    if let Ok(guard) = SESSION.lock() {
        WANTED.store(true, Ordering::SeqCst);
        drop(guard);
    }
    if KEEPER.swap(true, Ordering::SeqCst) {
        return;
    }
    let spawned = std::thread::Builder::new().name("wifi-gaming".into()).spawn(|| {
        let mut warned = false;
        while WANTED.load(Ordering::SeqCst) {
            if let Err(e) = keep_once() {
                if !warned {
                    log::info!("wifi-gamestand nog niet actief, nieuwe poging over 15 seconden: {e}");
                    warned = true;
                }
            }
            std::thread::sleep(KEEP_EVERY);
        }
        KEEPER.store(false, Ordering::SeqCst);
    });
    if spawned.is_err() {
        KEEPER.store(false, Ordering::SeqCst);
    }
}

pub fn status() -> AppResult<Status> {
    if is_active() {
        return Ok(Status::with_detail(TweakState::On, Msg::new("wifi_gaming_active")));
    }
    if adapter_count() == 0 {
        return Ok(Status::with_detail(TweakState::Unavailable, Msg::new("wifi_none")));
    }
    Ok(Status::simple(TweakState::Off))
}

pub fn apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    let d = get("wifi_gaming_mode")?;
    let was_active = is_active();
    let n = start()?;
    keep_active();
    if was_active {
        return Ok(());
    }
    let mut e = JournalEntry::new(d.id, d.name, JournalKind::Setting, &format!("Wi-Fi ({n}): background scan off, streaming mode on, while nezty runs"));
    e.old_value = Some("background scan on, streaming mode off".into());
    e.new_value = Some("background scan off, streaming mode on".into());
    e.revertable = true;
    e.session_id = session.map(|s| s.to_string());
    e.payload = RevertPayload::WifiGaming;
    lock(&state.journal).record(e)?;
    Ok(())
}

pub fn disable(state: &AppState) -> AppResult<()> {
    revert_via_journal(state, "wifi_gaming_mode", None)?;
    stop();
    Ok(())
}

/// Bij het starten van nezty: stond de wifi-gamestand aan (volgens het logboek), dan weer activeren. Lukt
/// dat nog niet (wifi nog niet verbonden), dan blijft de bewaker het proberen.
pub fn resume_if_active(state: &AppState) {
    if lock(&state.journal).has_active("wifi_gaming_mode", None) {
        if let Err(e) = start() {
            log::info!("wifi-gamestand nog niet hervat, nezty probeert het opnieuw: {e}");
        }
        keep_active();
    }
}
