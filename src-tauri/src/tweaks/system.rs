//! Systeemtweaks: Game Mode, Game Bar/DVR, energieplan, HAGS en visuele effecten.

use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, PostAction, RevertPayload};
use crate::models::{JournalKind, Msg, TweakState};
use crate::powershell::run_cmd;
use crate::registry::{self, Hive, RegData};
use crate::state::{lock, AppState};
use crate::tweaks::regtweak::{self, RegSpec};
use crate::tweaks::{get, revert_via_journal, Status};
use crate::winapi;

// ---------- Game Mode ----------

fn game_mode_specs() -> Vec<RegSpec> {
    let p = "Software\\Microsoft\\GameBar";
    vec![
        RegSpec::dword(Hive::Hkcu, p, "AutoGameModeEnabled", 1, 0).absent_on(),
        RegSpec::dword(Hive::Hkcu, p, "AllowAutoGameMode", 1, 0).absent_on(),
    ]
}
pub fn game_mode_status() -> AppResult<Status> {
    Ok(Status::simple(regtweak::state_of(&game_mode_specs())?))
}
pub fn game_mode_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    regtweak::apply(state, get("game_mode")?, &game_mode_specs(), session, None, None)
}
pub fn game_mode_disable(state: &AppState) -> AppResult<()> {
    regtweak::disable(state, get("game_mode")?, &game_mode_specs(), None, None)
}

// ---------- Game Bar en DVR ----------

fn game_bar_specs() -> Vec<RegSpec> {
    let dvr = "Software\\Microsoft\\Windows\\CurrentVersion\\GameDVR";
    let bar = "Software\\Microsoft\\GameBar";
    vec![
        RegSpec::dword(Hive::Hkcu, dvr, "AppCaptureEnabled", 0, 1),
        RegSpec::dword(Hive::Hkcu, "System\\GameConfigStore", "GameDVR_Enabled", 0, 1),
        RegSpec::dword(Hive::Hkcu, dvr, "HistoricalCaptureEnabled", 0, 0).absent_on().off_absent(),
        RegSpec::dword(Hive::Hkcu, bar, "UseNexusForGameBarEnabled", 0, 1),
        RegSpec::dword(Hive::Hkcu, bar, "ShowStartupPanel", 0, 1),
    ]
}
pub fn game_bar_status() -> AppResult<Status> {
    Ok(Status::simple(regtweak::state_of(&game_bar_specs())?))
}
pub fn game_bar_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    regtweak::apply(state, get("game_bar_dvr_off")?, &game_bar_specs(), session, None, None)
}
pub fn game_bar_disable(state: &AppState) -> AppResult<()> {
    regtweak::disable(state, get("game_bar_dvr_off")?, &game_bar_specs(), None, None)
}

// ---------- Meldingen ----------

/// Windows-meldingen (pop-ups rechtsonder, ook van Discord en andere apps) uit. Dezelfde schakelaar als
/// Instellingen > Systeem > Meldingen. Windows 11 leest de nieuwe sleutel (Notifications\Settings), oudere
/// onderdelen de oude (PushNotifications), daarom allebei. Meldingen komen nog wel in het meldingencentrum.
fn notifications_specs() -> Vec<RegSpec> {
    vec![
        RegSpec::dword(Hive::Hkcu, r"Software\Microsoft\Windows\CurrentVersion\Notifications\Settings", "NOC_GLOBAL_SETTING_TOASTS_ENABLED", 0, 1).off_absent(),
        RegSpec::dword(Hive::Hkcu, r"Software\Microsoft\Windows\CurrentVersion\PushNotifications", "ToastEnabled", 0, 1),
    ]
}

/// Apps met een eigen meldingsstijl die Windows niet tegenhoudt: naam en procesnamen.
const OWN_POPUPS: &[(&str, &[&str])] = &[("Teams", &["ms-teams.exe", "teams.exe"]), ("Steam", &["steam.exe"])];

pub fn notifications_status(state: &AppState) -> AppResult<Status> {
    let st = regtweak::state_of(&notifications_specs())?;
    if st != TweakState::On {
        return Ok(Status::simple(st));
    }
    // Staat de tweak aan, zeg er dan eerlijk bij welke draaiende apps toch nog pop-ups tonen.
    let running = crate::processes::running_names(state);
    let own: Vec<&str> = OWN_POPUPS.iter().filter(|(_, procs)| procs.iter().any(|p| running.contains(*p))).map(|(name, _)| *name).collect();
    if own.is_empty() {
        Ok(Status::simple(st))
    } else {
        Ok(Status::with_detail(st, Msg::new("notif_own_popups").with("apps", own.join(", "))))
    }
}
pub fn notifications_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    regtweak::apply(state, get("notifications_off")?, &notifications_specs(), session, None, None)
}
pub fn notifications_disable(state: &AppState) -> AppResult<()> {
    regtweak::disable(state, get("notifications_off")?, &notifications_specs(), None, None)
}

// ---------- Timerresolutie (experimenteel) ----------

/// Laat het timerverzoek van nezty (0,5 ms, alleen tijdens het gamen) voor alle programma's gelden.
fn timer_specs() -> Vec<RegSpec> {
    vec![RegSpec::dword(Hive::Hklm, r"SYSTEM\CurrentControlSet\Control\Session Manager\kernel", "GlobalTimerResolutionRequests", 1, 0).off_absent()]
}
pub fn timer_status() -> AppResult<Status> {
    Ok(Status::simple(regtweak::state_of(&timer_specs())?))
}
pub fn timer_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    regtweak::apply(state, get("timer_resolution")?, &timer_specs(), session, None, None)
}
pub fn timer_disable(state: &AppState) -> AppResult<()> {
    crate::timer::hold(false);
    regtweak::disable(state, get("timer_resolution")?, &timer_specs(), None, None)
}

// ---------- HAGS ----------

fn hags_specs() -> Vec<RegSpec> {
    vec![RegSpec::dword(Hive::Hklm, "SYSTEM\\CurrentControlSet\\Control\\GraphicsDrivers", "HwSchMode", 2, 1)]
}
pub fn hags_status() -> AppResult<Status> {
    let specs = hags_specs();
    let cur = registry::read(specs[0].hive, &specs[0].path, &specs[0].name)?;
    let state = regtweak::state_of(&specs)?;
    Ok(match cur {
        RegData::Absent => Status::with_detail(state, Msg::new("hags_default")),
        _ => Status::simple(state),
    })
}
pub fn hags_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    regtweak::apply(state, get("hags")?, &hags_specs(), session, None, None)
}
pub fn hags_disable(state: &AppState) -> AppResult<()> {
    regtweak::disable(state, get("hags")?, &hags_specs(), None, None)
}

// ---------- Visuele effecten ----------

fn visual_specs() -> Vec<RegSpec> {
    let adv = "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced";
    vec![
        RegSpec::dword(Hive::Hkcu, "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\VisualEffects", "VisualFXSetting", 2, 0),
        RegSpec {
            hive: Hive::Hkcu,
            path: "Control Panel\\Desktop".to_string(),
            name: "UserPreferencesMask".to_string(),
            on: RegData::Binary(vec![0x90, 0x12, 0x03, 0x80, 0x10, 0x00, 0x00, 0x00]),
            off: RegData::Binary(vec![0x9E, 0x3E, 0x07, 0x80, 0x12, 0x00, 0x00, 0x00]),
            absent_is_on: false,
        },
        RegSpec::string(Hive::Hkcu, "Control Panel\\Desktop\\WindowMetrics", "MinAnimate", "0", "1"),
        RegSpec::dword(Hive::Hkcu, adv, "TaskbarAnimations", 0, 1),
        RegSpec::dword(Hive::Hkcu, adv, "ListviewAlphaSelect", 0, 1),
        RegSpec::dword(Hive::Hkcu, adv, "ListviewShadow", 0, 1),
        RegSpec::dword(Hive::Hkcu, "Software\\Microsoft\\Windows\\DWM", "EnableAeroPeek", 0, 1),
    ]
}
pub fn visual_effects_status() -> AppResult<Status> {
    // De hoofdschakelaar bepaalt de status; de rest volgt.
    let cur = registry::read(Hive::Hkcu, "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\VisualEffects", "VisualFXSetting")?;
    Ok(Status::simple(if cur == RegData::Dword(2) { TweakState::On } else { TweakState::Off }))
}
pub fn visual_effects_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    regtweak::apply(state, get("visual_effects")?, &visual_specs(), session, None, Some(PostAction::UiEffects))?;
    winapi::set_ui_effects(false);
    winapi::set_min_animate(false);
    winapi::broadcast_setting_change("WindowMetrics");
    Ok(())
}
pub fn visual_effects_disable(state: &AppState) -> AppResult<()> {
    regtweak::disable(state, get("visual_effects")?, &visual_specs(), None, Some(PostAction::UiEffects))?;
    winapi::set_ui_effects(true);
    winapi::set_min_animate(true);
    winapi::broadcast_setting_change("WindowMetrics");
    Ok(())
}

// ---------- Energieplan ----------

pub const GUID_BALANCED: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";
pub const GUID_HIGH: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";
pub const GUID_ULTIMATE: &str = "e9a42b02-d5df-448d-aa00-03f14749eb61";
pub const GUID_SAVER: &str = "a1841308-3541-4fab-bc81-f71556f20b4a";

/// Leest "<label>: <guid>  (<naam>)" ongeacht de Windows-taal: het GUID-patroon is overal gelijk.
fn parse_scheme_line(line: &str) -> Option<(String, String)> {
    let re = regex::Regex::new(r"(?i)([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})").ok()?;
    let m = re.find(line)?;
    let guid = m.as_str().to_lowercase();
    let rest = &line[m.end()..];
    let name = rest
        .find('(')
        .and_then(|a| rest.rfind(')').map(|b| (a, b)))
        .filter(|(a, b)| b > a)
        .map(|(a, b)| rest[a + 1..b].trim().to_string())
        .unwrap_or_default();
    Some((guid, name))
}

pub fn active_scheme() -> AppResult<(String, String)> {
    let (ok, out, err) = run_cmd("powercfg", &["/getactivescheme"])?;
    if !ok {
        return Err(AppError::msg(format!("powercfg: {}", err.trim())));
    }
    out.lines().find_map(parse_scheme_line).ok_or_else(|| AppError::msg("powercfg: no active scheme"))
}

pub fn list_schemes() -> AppResult<Vec<(String, String)>> {
    let (ok, out, err) = run_cmd("powercfg", &["/list"])?;
    if !ok {
        return Err(AppError::msg(format!("powercfg: {}", err.trim())));
    }
    Ok(out.lines().filter_map(parse_scheme_line).collect())
}

fn is_ultimate(guid: &str, name: &str) -> bool {
    guid == GUID_ULTIMATE || name.to_lowercase().contains("ultim")
}

pub fn power_plan_status() -> AppResult<Status> {
    let (guid, name) = active_scheme()?;
    let (state, current_option) = if guid == GUID_HIGH {
        (TweakState::On, Some("high".to_string()))
    } else if is_ultimate(&guid, &name) {
        (TweakState::On, Some("ultimate".to_string()))
    } else {
        (TweakState::Off, Some("high".to_string()))
    };
    Ok(Status { state, detail: Some(Msg::new("power_current").with("name", name)), current_option })
}

fn set_active(guid: &str) -> AppResult<()> {
    let (ok, _, err) = run_cmd("powercfg", &["/setactive", guid])?;
    if !ok {
        let msg = err.trim().to_string();
        if msg.to_lowercase().contains("denied") || msg.to_lowercase().contains("geweigerd") {
            return Err(AppError::NeedsAdmin("powercfg".into()));
        }
        return Err(AppError::msg(format!("powercfg: {msg}")));
    }
    Ok(())
}

fn duplicate_scheme(state: &AppState, source: &str) -> AppResult<String> {
    if !state.elevated {
        return Err(AppError::NeedsAdmin("powercfg -duplicatescheme".into()));
    }
    let (ok, out, err) = run_cmd("powercfg", &["-duplicatescheme", source])?;
    if !ok {
        return Err(AppError::msg(format!("powercfg: {}", err.trim())));
    }
    out.lines().find_map(parse_scheme_line).map(|(g, _)| g).ok_or_else(|| AppError::msg("powercfg: no new scheme"))
}

pub fn power_plan_apply(state: &AppState, option: &str, session: Option<&str>) -> AppResult<()> {
    let (old_guid, old_name) = active_scheme()?;
    let schemes = list_schemes()?;
    let mut created: Option<String> = None;
    let target = if option == "ultimate" {
        if is_ultimate(&old_guid, &old_name) {
            return Ok(());
        }
        match schemes.iter().find(|(g, n)| is_ultimate(g, n)) {
            Some((g, _)) => g.clone(),
            None => {
                let g = duplicate_scheme(state, GUID_ULTIMATE)?;
                created = Some(g.clone());
                g
            }
        }
    } else {
        if old_guid == GUID_HIGH {
            return Ok(());
        }
        match schemes.iter().find(|(g, _)| g == GUID_HIGH) {
            Some((g, _)) => g.clone(),
            None => {
                let g = duplicate_scheme(state, GUID_HIGH)?;
                created = Some(g.clone());
                g
            }
        }
    };
    set_active(&target)?;
    let (_, new_name) = active_scheme().unwrap_or((target.clone(), target.clone()));
    let d = get("power_plan")?;
    let mut e = JournalEntry::new(d.id, d.name, JournalKind::PowerPlan, "powercfg /setactive");
    e.old_value = Some(format!("{old_name} ({old_guid})"));
    e.new_value = Some(format!("{new_name} ({target})"));
    e.revertable = true;
    e.session_id = session.map(|s| s.to_string());
    e.payload = RevertPayload::PowerPlan { old_guid, created_guid: created };
    lock(&state.journal).record(e)?;
    Ok(())
}

pub fn power_plan_disable(state: &AppState) -> AppResult<()> {
    if revert_via_journal(state, "power_plan", None)? {
        if power_plan_status()?.state == TweakState::Off {
            return Ok(());
        }
    }
    let (old_guid, old_name) = active_scheme()?;
    if old_guid == GUID_BALANCED {
        return Ok(());
    }
    set_active(GUID_BALANCED)?;
    let d = get("power_plan")?;
    let mut e = JournalEntry::new(d.id, d.name, JournalKind::PowerPlan, "powercfg /setactive");
    e.old_value = Some(format!("{old_name} ({old_guid})"));
    e.new_value = Some(format!("Balanced ({GUID_BALANCED})"));
    e.revertable = true;
    e.note = Some("manual_off".to_string());
    e.payload = RevertPayload::PowerPlan { old_guid, created_guid: None };
    lock(&state.journal).record(e)?;
    Ok(())
}

// ---------- Optimalisaties voor games in een venster ----------
// Windows 11-instelling (Instellingen > Systeem > Beeldscherm > Grafische kaart). Laat DirectX 10/11-games in
// een venster of borderless het snellere "flip"-model gebruiken: minder vertraging, soms iets meer FPS.
// Staat samen met andere waarden (bv. AutoHDREnable) in één tekst; die blijven ongemoeid.

const DX_PATH: &str = "Software\\Microsoft\\DirectX\\UserGpuPreferences";
const DX_VALUE: &str = "DirectXUserGlobalSettings";
const DX_KEY: &str = "SwapEffectUpgradeEnable";

fn dx_settings() -> AppResult<RegData> {
    registry::read(Hive::Hkcu, DX_PATH, DX_VALUE)
}

/// Waarde van één sleutel in een tekst als "A=1;B=0;".
fn dx_get(text: &str, key: &str) -> Option<String> {
    text.split(';').filter_map(|p| p.split_once('=')).find(|(k, _)| k.trim().eq_ignore_ascii_case(key)).map(|(_, v)| v.trim().to_string())
}

/// Zet één sleutel en laat de rest van de tekst staan.
fn dx_set(text: &str, key: &str, value: &str) -> String {
    let mut parts: Vec<String> = text
        .split(';')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .filter(|p| !p.split_once('=').map(|(k, _)| k.trim().eq_ignore_ascii_case(key)).unwrap_or(false))
        .map(|p| p.to_string())
        .collect();
    parts.push(format!("{key}={value}"));
    parts.iter().map(|p| format!("{p};")).collect()
}

fn dx_text(data: &RegData) -> String {
    match data {
        RegData::Str(s) | RegData::ExpandStr(s) => s.clone(),
        _ => String::new(),
    }
}

pub fn windowed_opt_status() -> AppResult<Status> {
    let on = dx_get(&dx_text(&dx_settings()?), DX_KEY).as_deref() == Some("1");
    Ok(Status::simple(if on { TweakState::On } else { TweakState::Off }))
}

fn windowed_opt_write(state: &AppState, value: &str, session: Option<&str>, note: Option<&str>) -> AppResult<()> {
    let def = get("windowed_opt")?;
    let old = dx_settings()?;
    let new_text = dx_set(&dx_text(&old), DX_KEY, value);
    if dx_text(&old) == new_text {
        return Ok(());
    }
    let new = RegData::Str(new_text);
    registry::write(Hive::Hkcu, DX_PATH, DX_VALUE, &new)?;
    let mut e = JournalEntry::new(def.id, def.name, JournalKind::Registry, &registry::full_path(Hive::Hkcu, DX_PATH, DX_VALUE));
    e.old_value = Some(old.display());
    e.new_value = Some(new.display());
    e.revertable = true;
    e.session_id = session.map(|s| s.to_string());
    e.note = note.map(|n| n.to_string());
    e.payload = RevertPayload::Registry { hive: Hive::Hkcu, path: DX_PATH.to_string(), name: DX_VALUE.to_string(), old, post: None };
    lock(&state.journal).record(e)
}

pub fn windowed_opt_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    if dx_get(&dx_text(&dx_settings()?), DX_KEY).as_deref() == Some("1") {
        return Ok(());
    }
    windowed_opt_write(state, "1", session, None)
}

pub fn windowed_opt_disable(state: &AppState) -> AppResult<()> {
    if revert_via_journal(state, "windowed_opt", None)? && windowed_opt_status()?.state == TweakState::Off {
        return Ok(());
    }
    windowed_opt_write(state, "0", None, Some("manual_off"))
}

#[cfg(test)]
mod dx_tests {
    use super::*;
    #[test]
    fn keeps_other_values() {
        assert_eq!(dx_set("", DX_KEY, "1"), "SwapEffectUpgradeEnable=1;");
        assert_eq!(dx_set("AutoHDREnable=1;SwapEffectUpgradeEnable=0;", DX_KEY, "1"), "AutoHDREnable=1;SwapEffectUpgradeEnable=1;");
        assert_eq!(dx_get("VRROptimizeEnable=0;SwapEffectUpgradeEnable=1;", DX_KEY).as_deref(), Some("1"));
    }
}
