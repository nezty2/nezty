//! Tweaks die alleen op een laptop zin hebben:
//! - Windows-energiemodus op "Beste prestaties". Laptops met Modern Standby hebben vaak alleen het plan
//!   Gebalanceerd; daar is de energiemodus (de schuif in Instellingen) de knop die telt.
//! - Batterijbesparing gaat niet vanzelf aan tijdens het gamen (drempel op de batterij naar 0 procent).
//! - Processor op volle snelheid, snelle reactie (EPP), games op de snelle kernen.
//! - PCI Express en USB wakker op de lader, touchpad uit als er een muis is.
//! Alles gaat via het logboek en is exact terug te zetten.

use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, PostAction, RevertPayload};
use crate::models::{JournalKind, Msg, TweakState};
use crate::registry::{self, Hive, RegData};
use crate::state::{lock, AppState};
use crate::tweaks::input::{read_power_setting, write_power_setting};
use crate::tweaks::regtweak::{self, RegSpec};
use crate::tweaks::{get, revert_via_journal, system, Status};
use crate::winapi::wide;
use std::ffi::c_void;
use windows_sys::core::GUID;
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

// ---------- Energiemodus ----------

/// Overlay "Beste prestaties". Nul-GUID is "Gebalanceerd", 961cc777... is "Beste energie-efficiëntie".
pub const OVERLAY_BEST: &str = "ded574b5-45a0-4f42-8737-46345c09c238";
const OVERLAY_BALANCED: &str = "00000000-0000-0000-0000-000000000000";
const SCHEMES_KEY: &str = "SYSTEM\\CurrentControlSet\\Control\\Power\\User\\PowerSchemes";

fn parse_guid(s: &str) -> Option<GUID> {
    let hex: String = s.trim().trim_matches(|c| c == '{' || c == '}').chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 {
        return None;
    }
    u128::from_str_radix(&hex, 16).ok().map(GUID::from_u128)
}

fn guid_string(g: &GUID) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        g.data1, g.data2, g.data3, g.data4[0], g.data4[1], g.data4[2], g.data4[3], g.data4[4], g.data4[5], g.data4[6], g.data4[7]
    )
}

type OverlayFn = unsafe extern "system" fn(*mut GUID) -> u32;

/// powrprof.dll exporteert de overlay-functies wel, maar niet elke importbibliotheek kent ze. Daarom
/// zoeken we ze op tijdens het draaien.
fn overlay_fn(name: &str) -> Option<OverlayFn> {
    unsafe {
        let lib = LoadLibraryW(wide("powrprof.dll").as_ptr());
        if lib.is_null() {
            return None;
        }
        let cname = std::ffi::CString::new(name).ok()?;
        let f = GetProcAddress(lib, cname.as_ptr() as *const u8)?;
        Some(std::mem::transmute::<unsafe extern "system" fn() -> isize, OverlayFn>(f))
    }
}

fn effective_overlay() -> Option<String> {
    let f = overlay_fn("PowerGetEffectiveOverlayScheme")?;
    let mut g: GUID = GUID::from_u128(0);
    let rc = unsafe { f(&mut g) };
    (rc == 0).then(|| guid_string(&g))
}

fn set_overlay(guid: &str) -> AppResult<()> {
    let f = overlay_fn("PowerSetActiveOverlayScheme").ok_or_else(|| AppError::coded("power_mode_unavailable"))?;
    let mut g = parse_guid(guid).ok_or_else(|| AppError::msg("guid"))?;
    let rc = unsafe { f(&mut g) };
    if rc != 0 {
        return Err(AppError::msg(format!("PowerSetActiveOverlayScheme: {rc}")));
    }
    Ok(())
}

fn on_ac() -> bool {
    let mut ps: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    let ok = unsafe { GetSystemPowerStatus(&mut ps) } != 0;
    !ok || ps.ACLineStatus != 0
}

/// Na het terugzetten van het register: de bewaarde energiemodus voor de huidige stroombron weer toepassen.
pub fn apply_overlay_from_registry() {
    let name = if on_ac() { "ActiveOverlayAcPowerScheme" } else { "ActiveOverlayDcPowerScheme" };
    let guid = match registry::read(Hive::Hklm, SCHEMES_KEY, name) {
        Ok(RegData::Str(s)) if parse_guid(&s).is_some() => s,
        _ => OVERLAY_BALANCED.to_string(),
    };
    let _ = set_overlay(&guid);
}

fn mode_specs() -> Vec<RegSpec> {
    vec![
        RegSpec::string(Hive::Hklm, SCHEMES_KEY, "ActiveOverlayAcPowerScheme", OVERLAY_BEST, OVERLAY_BALANCED).off_absent(),
        RegSpec::string(Hive::Hklm, SCHEMES_KEY, "ActiveOverlayDcPowerScheme", OVERLAY_BEST, OVERLAY_BALANCED).off_absent(),
    ]
}

pub fn power_mode_status() -> AppResult<Status> {
    // Met een energieplan voor hoge prestaties staat de energiemodus al op het hoogste (Windows verbergt de schuif dan).
    if let Ok((guid, name)) = system::active_scheme() {
        if guid != system::GUID_BALANCED {
            let high = guid == system::GUID_HIGH || name.to_lowercase().contains("ultimate");
            if high {
                return Ok(Status::with_detail(TweakState::On, Msg::new("power_mode_via_plan").with("plan", name)));
            }
        }
    }
    let Some(current) = effective_overlay() else {
        return Ok(Status::with_detail(TweakState::Unavailable, Msg::new("power_mode_unavailable")));
    };
    let registry_on = regtweak::state_of(&mode_specs())? == TweakState::On;
    if current.eq_ignore_ascii_case(OVERLAY_BEST) && registry_on {
        return Ok(Status::simple(TweakState::On));
    }
    // Op deze stroombron al op het hoogste, maar op de andere niet: dat telt nog als uit.
    Ok(Status::simple(TweakState::Off))
}

pub fn power_mode_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    regtweak::apply(state, get("power_mode_best")?, &mode_specs(), session, None, Some(PostAction::PowerOverlay))?;
    set_overlay(OVERLAY_BEST)
}

pub fn power_mode_disable(state: &AppState) -> AppResult<()> {
    regtweak::disable(state, get("power_mode_best")?, &mode_specs(), None, Some(PostAction::PowerOverlay))?;
    apply_overlay_from_registry();
    Ok(())
}

// ---------- Batterijbesparing ----------

/// Subgroep "Energiebesparing" en de instelling "Batterijbesparing gaat aan bij ... procent".
const SAVER_SUB: &str = "de830923-a562-41af-a086-e3a2c6bad2da";
const SAVER_THRESHOLD: &str = "e69653ca-cf7f-4f05-aa73-cb833fa90ad4";

fn saver_active_now() -> bool {
    let mut ps: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    unsafe { GetSystemPowerStatus(&mut ps) != 0 && ps.SystemStatusFlag == 1 }
}

pub fn battery_saver_status() -> AppResult<Status> {
    let (scheme, _) = system::active_scheme()?;
    let Some((_, dc)) = read_power_setting(&scheme, SAVER_SUB, SAVER_THRESHOLD)? else {
        return Ok(Status::with_detail(TweakState::Unavailable, Msg::new("battery_saver_unavailable")));
    };
    let state = if dc == 0 { TweakState::On } else { TweakState::Off };
    if saver_active_now() {
        return Ok(Status::with_detail(state, Msg::new("battery_saver_active_now")));
    }
    if dc > 0 {
        return Ok(Status::with_detail(state, Msg::new("battery_saver_threshold").with("pct", dc)));
    }
    Ok(Status::simple(state))
}

fn saver_set(state: &AppState, dc_value: u32, session: Option<&str>, note: Option<&str>) -> AppResult<()> {
    let d = get("battery_saver_off")?;
    let (scheme, scheme_name) = system::active_scheme()?;
    let Some((old_ac, old_dc)) = read_power_setting(&scheme, SAVER_SUB, SAVER_THRESHOLD)? else {
        return Err(AppError::coded("battery_saver_unavailable"));
    };
    if old_dc == dc_value {
        return Ok(());
    }
    write_power_setting(&scheme, SAVER_SUB, SAVER_THRESHOLD, old_ac, dc_value)?;
    let mut e = JournalEntry::new(d.id, d.name, JournalKind::PowerSetting, &format!("{scheme_name}: battery saver threshold"));
    e.old_value = Some(format!("{old_dc}%"));
    e.new_value = Some(format!("{dc_value}%"));
    e.revertable = true;
    e.session_id = session.map(|s| s.to_string());
    e.note = note.map(|s| s.to_string());
    e.payload = RevertPayload::PowerSetting { scheme, sub: SAVER_SUB.into(), setting: SAVER_THRESHOLD.into(), ac: old_ac, dc: old_dc };
    lock(&state.journal).record(e)?;
    Ok(())
}

pub fn battery_saver_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    saver_set(state, 0, session, None)
}

pub fn battery_saver_disable(state: &AppState) -> AppResult<()> {
    if revert_via_journal(state, "battery_saver_off", None)? && battery_saver_status()?.state == TweakState::Off {
        return Ok(());
    }
    // Windows-standaard: batterijbesparing gaat aan bij 20 procent.
    saver_set(state, 20, None, Some("manual_off"))
}

// ---------- Processor, PCI Express en USB (energie-instellingen) ----------
//
// Veel laptopmerken remmen in hun energieplan de processor af: maximaal 99 procent (dan valt turbo weg),
// een vaste kloklimiet, een zuinige turbostand of een trage reactie (EPP). Op de lader willen we bij het
// gamen het snelste. Alles hieronder schrijft alleen in het actieve energieplan en logt elke oude waarde.

const SUB_PROCESSOR: &str = "54533251-82be-4824-96c1-47b60b740d00";
const PROCTHROTTLEMAX: &str = "bc5038f7-23e0-4960-96da-33abaf5935ec";
const PROCTHROTTLEMAX1: &str = "bc5038f7-23e0-4960-96da-33abaf5935ed";
const PROCFREQMAX: &str = "75b0ae3f-bce0-45a7-8c89-c9611c25e100";
const PROCFREQMAX1: &str = "75b0ae3f-bce0-45a7-8c89-c9611c25e101";
const PERFBOOSTMODE: &str = "be337238-0d82-4146-a960-4f3749d470c7";
const PERFEPP: &str = "36687f9e-e3a5-4dbf-b1dc-15eb381c6863";
const PERFEPP1: &str = "36687f9e-e3a5-4dbf-b1dc-15eb381c6864";
const SCHEDPOLICY: &str = "93b8b6dc-0698-4d1c-9ee4-0644e900c85d";
const SHORTSCHEDPOLICY: &str = "bae08b81-2d5e-4688-ad6a-13243356654b";
const SUB_PCIE: &str = "501a4d13-42af-4429-9fd1-a8218c268e20";
const ASPM: &str = "ee12f906-d277-404b-b6da-e5fa1a576df5";
const SUB_USB: &str = "2a737441-1930-4402-8d77-b2bebba308a3";
const USB_SELECTIVE: &str = "48e6b7a6-50f5-4782-a5d4-53bb8f07e226";

/// Eén energie-instelling met de gewenste waarde op de lader (ac) en op de accu (dc). None: die kant niet aanraken.
#[derive(Clone, Copy)]
struct Want {
    sub: &'static str,
    setting: &'static str,
    label: &'static str,
    ac: Option<u32>,
    dc: Option<u32>,
    /// Waarden tot en met deze grens tellen ook als goed (EPP 10 is al vrijwel het snelst).
    ok_max: Option<u32>,
    /// Bestaat niet op elke pc (bv. instellingen voor zuinige kernen): ontbreekt hij, dan overslaan.
    optional: bool,
}

const fn want(sub: &'static str, setting: &'static str, label: &'static str, ac: Option<u32>, dc: Option<u32>) -> Want {
    Want { sub, setting, label, ac, dc, ok_max: None, optional: false }
}

const FULL_SPEED: [Want; 5] = [
    want(SUB_PROCESSOR, PROCTHROTTLEMAX, "maximum processor state", Some(100), Some(100)),
    Want { optional: true, ..want(SUB_PROCESSOR, PROCTHROTTLEMAX1, "maximum processor state (efficient cores)", Some(100), Some(100)) },
    want(SUB_PROCESSOR, PROCFREQMAX, "maximum processor frequency", Some(0), Some(0)),
    Want { optional: true, ..want(SUB_PROCESSOR, PROCFREQMAX1, "maximum processor frequency (efficient cores)", Some(0), Some(0)) },
    want(SUB_PROCESSOR, PERFBOOSTMODE, "processor performance boost mode", Some(2), Some(2)),
];

const EPP: [Want; 2] = [
    Want { ok_max: Some(10), ..want(SUB_PROCESSOR, PERFEPP, "processor energy performance preference", Some(0), None) },
    Want { ok_max: Some(10), optional: true, ..want(SUB_PROCESSOR, PERFEPP1, "processor energy performance preference (efficient cores)", Some(0), None) },
];

const PCORES: [Want; 2] = [
    want(SUB_PROCESSOR, SCHEDPOLICY, "heterogeneous thread scheduling policy", Some(2), Some(2)),
    want(SUB_PROCESSOR, SHORTSCHEDPOLICY, "heterogeneous short running thread scheduling policy", Some(2), Some(2)),
];

const PCIE: [Want; 1] = [want(SUB_PCIE, ASPM, "PCI Express link state power management", Some(0), None)];

const USB_AC: [Want; 1] = [want(SUB_USB, USB_SELECTIVE, "USB selective suspend", Some(0), None)];

fn good(w: &Want, cur: u32, target: Option<u32>) -> bool {
    let Some(t) = target else { return true };
    // Turbo: "Agressief" (2) en "Agressief op gegarandeerd" (5) zijn allebei de snelle stand.
    if w.setting == PERFBOOSTMODE {
        return cur == 2 || cur == 5;
    }
    // Kernen: "Snelle kernen" (1) en "Liefst snelle kernen" (2) zijn allebei goed.
    if w.setting == SCHEDPOLICY || w.setting == SHORTSCHEDPOLICY {
        return cur == 1 || cur == 2;
    }
    match w.ok_max {
        Some(m) => cur <= m,
        None => cur == t,
    }
}

fn read_wants(scheme: &str, wants: &[Want]) -> AppResult<Vec<(Want, Option<(u32, u32)>)>> {
    wants.iter().map(|w| Ok((*w, read_power_setting(scheme, w.sub, w.setting)?))).collect()
}

/// None: een verplichte instelling ontbreekt in dit energieplan.
fn wants_state(scheme: &str, wants: &[Want]) -> AppResult<Option<TweakState>> {
    let mut all = true;
    for (w, cur) in read_wants(scheme, wants)? {
        match cur {
            None if w.optional => continue,
            None => return Ok(None),
            Some((ac, dc)) => {
                if !(good(&w, ac, w.ac) && good(&w, dc, w.dc)) {
                    all = false;
                }
            }
        }
    }
    Ok(Some(if all { TweakState::On } else { TweakState::Off }))
}

fn apply_wants(state: &AppState, id: &str, wants: &[Want], session: Option<&str>) -> AppResult<()> {
    let d = get(id)?;
    let (scheme, scheme_name) = system::active_scheme()?;
    for (w, cur) in read_wants(&scheme, wants)? {
        let Some((old_ac, old_dc)) = cur else {
            if w.optional {
                continue;
            }
            return Err(AppError::coded("power_setting_missing"));
        };
        let new_ac = match w.ac {
            Some(t) if !good(&w, old_ac, Some(t)) => t,
            _ => old_ac,
        };
        let new_dc = match w.dc {
            Some(t) if !good(&w, old_dc, Some(t)) => t,
            _ => old_dc,
        };
        if new_ac == old_ac && new_dc == old_dc {
            continue;
        }
        write_power_setting(&scheme, w.sub, w.setting, new_ac, new_dc)?;
        let mut e = JournalEntry::new(d.id, d.name, JournalKind::PowerSetting, &format!("{scheme_name}: {}", w.label));
        e.old_value = Some(format!("AC {old_ac}, DC {old_dc}"));
        e.new_value = Some(format!("AC {new_ac}, DC {new_dc}"));
        e.revertable = true;
        e.session_id = session.map(|s| s.to_string());
        e.payload = RevertPayload::PowerSetting { scheme: scheme.clone(), sub: w.sub.into(), setting: w.setting.into(), ac: old_ac, dc: old_dc };
        lock(&state.journal).record(e)?;
    }
    Ok(())
}

/// Uitzetten: terug naar wat er stond. Stond het al zo voordat nezty iets deed, dan valt er niets terug te zetten.
fn disable_wants(state: &AppState, id: &str, wants: &[Want]) -> AppResult<()> {
    if revert_via_journal(state, id, None)? {
        return Ok(());
    }
    let (scheme, _) = system::active_scheme()?;
    if wants_state(&scheme, wants)? == Some(TweakState::On) {
        return Err(AppError::coded("already_default"));
    }
    Ok(())
}

fn missing() -> AppResult<Status> {
    Ok(Status::with_detail(TweakState::Unavailable, Msg::new("power_setting_missing")))
}

pub fn cpu_full_speed_status() -> AppResult<Status> {
    let (scheme, _) = system::active_scheme()?;
    let Some(st) = wants_state(&scheme, &FULL_SPEED)? else { return missing() };
    if st == TweakState::On {
        return Ok(Status::simple(st));
    }
    if let Some((ac, dc)) = read_power_setting(&scheme, SUB_PROCESSOR, PROCTHROTTLEMAX)? {
        if ac.min(dc) < 100 {
            return Ok(Status::with_detail(st, Msg::new("cpu_capped").with("pct", ac.min(dc))));
        }
    }
    if let Some((ac, dc)) = read_power_setting(&scheme, SUB_PROCESSOR, PROCFREQMAX)? {
        if let Some(cap) = [ac, dc].into_iter().filter(|v| *v > 0).min() {
            return Ok(Status::with_detail(st, Msg::new("cpu_freq_capped").with("mhz", cap)));
        }
    }
    Ok(Status::with_detail(st, Msg::new("cpu_boost_soft")))
}
pub fn cpu_full_speed_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    apply_wants(state, "cpu_full_speed", &FULL_SPEED, session)
}
pub fn cpu_full_speed_disable(state: &AppState) -> AppResult<()> {
    disable_wants(state, "cpu_full_speed", &FULL_SPEED)
}

pub fn cpu_epp_status() -> AppResult<Status> {
    let (scheme, _) = system::active_scheme()?;
    let Some(st) = wants_state(&scheme, &EPP)? else { return missing() };
    if st == TweakState::Off {
        if let Some((ac, _)) = read_power_setting(&scheme, SUB_PROCESSOR, PERFEPP)? {
            return Ok(Status::with_detail(st, Msg::new("cpu_epp_now").with("pct", ac)));
        }
    }
    Ok(Status::simple(st))
}
pub fn cpu_epp_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    apply_wants(state, "cpu_epp_performance", &EPP, session)
}
pub fn cpu_epp_disable(state: &AppState) -> AppResult<()> {
    disable_wants(state, "cpu_epp_performance", &EPP)
}

/// Heeft de processor snelle en zuinige kernen (Intel vanaf 12e generatie, sommige AMD)? We kijken of er
/// kernen met verschillende efficiëntieklassen zijn.
pub fn cpu_is_hybrid() -> bool {
    use windows_sys::Win32::System::SystemInformation::{GetLogicalProcessorInformationEx, RelationProcessorCore};
    let mut len = 0u32;
    unsafe { GetLogicalProcessorInformationEx(RelationProcessorCore, std::ptr::null_mut(), &mut len) };
    if len == 0 {
        return false;
    }
    let mut buf = vec![0u8; len as usize];
    if unsafe { GetLogicalProcessorInformationEx(RelationProcessorCore, buf.as_mut_ptr() as *mut _, &mut len) } == 0 {
        return false;
    }
    // Opbouw per item: Relationship (u32), Size (u32), daarna PROCESSOR_RELATIONSHIP met Flags (u8) en EfficiencyClass (u8).
    let mut classes = std::collections::HashSet::new();
    let mut off = 0usize;
    while off + 10 <= len as usize {
        let rel = u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]);
        let size = u32::from_le_bytes([buf[off + 4], buf[off + 5], buf[off + 6], buf[off + 7]]) as usize;
        if rel == RelationProcessorCore as u32 {
            classes.insert(buf[off + 9]);
        }
        if size == 0 {
            break;
        }
        off += size;
    }
    classes.len() > 1
}

pub fn cpu_pcores_status() -> AppResult<Status> {
    if !cpu_is_hybrid() {
        return Ok(Status::with_detail(TweakState::Unavailable, Msg::new("cpu_not_hybrid")));
    }
    let (scheme, _) = system::active_scheme()?;
    match wants_state(&scheme, &PCORES)? {
        None => missing(),
        Some(st) => Ok(Status::simple(st)),
    }
}
pub fn cpu_pcores_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    if !cpu_is_hybrid() {
        return Err(AppError::coded("cpu_not_hybrid"));
    }
    apply_wants(state, "cpu_prefer_pcores", &PCORES, session)
}
pub fn cpu_pcores_disable(state: &AppState) -> AppResult<()> {
    disable_wants(state, "cpu_prefer_pcores", &PCORES)
}

pub fn pcie_status() -> AppResult<Status> {
    let (scheme, _) = system::active_scheme()?;
    match wants_state(&scheme, &PCIE)? {
        None => missing(),
        Some(TweakState::Off) => Ok(Status::with_detail(TweakState::Off, Msg::new("aspm_on"))),
        Some(st) => Ok(Status::simple(st)),
    }
}
pub fn pcie_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    apply_wants(state, "pcie_aspm_off", &PCIE, session)
}
pub fn pcie_disable(state: &AppState) -> AppResult<()> {
    disable_wants(state, "pcie_aspm_off", &PCIE)
}

pub fn usb_ac_status() -> AppResult<Status> {
    let (scheme, _) = system::active_scheme()?;
    match wants_state(&scheme, &USB_AC)? {
        None => missing(),
        Some(TweakState::Off) => Ok(Status::with_detail(TweakState::Off, Msg::new("usb_ac_on"))),
        Some(st) => Ok(Status::simple(st)),
    }
}
pub fn usb_ac_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    apply_wants(state, "usb_suspend_ac_off", &USB_AC, session)
}
pub fn usb_ac_disable(state: &AppState) -> AppResult<()> {
    disable_wants(state, "usb_suspend_ac_off", &USB_AC)
}

// ---------- Touchpad uit als er een muis is ----------
//
// Met je hand op de touchpad schiet je richtkruis weg. Windows kan de touchpad uitzetten zolang er een muis
// is aangesloten. Dat gaat via SystemParametersInfo met TOUCHPAD_PARAMETERS (Windows 11).

const SPI_GETTOUCHPADPARAMETERS: u32 = 0x00AE;
const SPI_SETTOUCHPADPARAMETERS: u32 = 0x00AF;

/// TOUCHPAD_PARAMETERS (versie 1) uit winuser.h. De BOOL-bitvelden zitten in twee 32-bits woorden.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct TouchpadParameters {
    version_number: u32,
    max_supported_contacts: u32,
    legacy_touchpad_features: i32,
    /// bit 0 touchpadPresent, bit 1 legacyTouchpadPresent, bit 2 externalMousePresent, ...
    flags1: u32,
    /// bit 0 allowActiveWhenMousePresent, ...
    flags2: u32,
    sensitivity_level: i32,
    cursor_speed: u32,
    feedback_intensity: u32,
    click_force_sensitivity: u32,
    right_click_zone_width: u32,
    right_click_zone_height: u32,
}

fn touchpad_get() -> Option<TouchpadParameters> {
    use windows_sys::Win32::UI::WindowsAndMessaging::SystemParametersInfoW;
    let mut p = TouchpadParameters { version_number: 1, ..Default::default() };
    let ok = unsafe { SystemParametersInfoW(SPI_GETTOUCHPADPARAMETERS, std::mem::size_of::<TouchpadParameters>() as u32, &mut p as *mut _ as *mut c_void, 0) } != 0;
    (ok && p.flags1 & 1 != 0).then_some(p)
}

/// Zet "touchpad aan laten als er een muis is" aan of uit, en controleert het resultaat.
pub fn touchpad_set_allow(allow: bool) -> AppResult<()> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE};
    let mut p = touchpad_get().ok_or_else(|| AppError::coded("touchpad_none"))?;
    p.version_number = 1;
    if allow {
        p.flags2 |= 1;
    } else {
        p.flags2 &= !1;
    }
    let ok = unsafe {
        SystemParametersInfoW(SPI_SETTOUCHPADPARAMETERS, std::mem::size_of::<TouchpadParameters>() as u32, &mut p as *mut _ as *mut c_void, SPIF_UPDATEINIFILE | SPIF_SENDCHANGE)
    } != 0;
    if !ok {
        return Err(AppError::msg("SPI_SETTOUCHPADPARAMETERS"));
    }
    match touchpad_get() {
        Some(now) if (now.flags2 & 1 != 0) == allow => Ok(()),
        _ => Err(AppError::coded("verify_failed")),
    }
}

pub fn touchpad_status() -> AppResult<Status> {
    match touchpad_get() {
        None => Ok(Status::with_detail(TweakState::Unavailable, Msg::new("touchpad_none"))),
        Some(p) => Ok(Status::simple(if p.flags2 & 1 == 0 { TweakState::On } else { TweakState::Off })),
    }
}

pub fn touchpad_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    let d = get("touchpad_off_with_mouse")?;
    let p = touchpad_get().ok_or_else(|| AppError::coded("touchpad_none"))?;
    let old_allow = p.flags2 & 1 != 0;
    if !old_allow {
        return Ok(());
    }
    touchpad_set_allow(false)?;
    let mut e = JournalEntry::new(d.id, d.name, JournalKind::Setting, "Touchpad: leave on when a mouse is connected");
    e.old_value = Some(old_allow.to_string());
    e.new_value = Some("false".into());
    e.revertable = true;
    e.session_id = session.map(|s| s.to_string());
    e.payload = RevertPayload::TouchpadMouse { allow: old_allow };
    lock(&state.journal).record(e)?;
    Ok(())
}

pub fn touchpad_disable(state: &AppState) -> AppResult<()> {
    if revert_via_journal(state, "touchpad_off_with_mouse", None)? && touchpad_status()?.state == TweakState::Off {
        return Ok(());
    }
    // Windows-standaard: touchpad blijft aan met een muis erbij.
    touchpad_set_allow(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_roundtrip() {
        let g = parse_guid(OVERLAY_BEST).unwrap();
        assert_eq!(guid_string(&g), OVERLAY_BEST);
    }
}
