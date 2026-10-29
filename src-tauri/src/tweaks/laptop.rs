//! Tweaks die alleen op een laptop zin hebben:
//! - Windows-energiemodus op "Beste prestaties". Laptops met Modern Standby hebben vaak alleen het plan
//!   Gebalanceerd; daar is de energiemodus (de schuif in Instellingen) de knop die telt.
//! - Batterijbesparing gaat niet vanzelf aan tijdens het gamen (drempel op de batterij naar 0 procent).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_roundtrip() {
        let g = parse_guid(OVERLAY_BEST).unwrap();
        assert_eq!(guid_string(&g), OVERLAY_BEST);
    }
}
