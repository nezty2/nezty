//! Systeemtweaks: Game Mode, Game Bar/DVR, meldingen, energieplan, HAGS en games in een venster.

use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, RevertPayload};
use crate::models::{JournalKind, Msg, TweakState};
use crate::powershell::run_cmd;
use crate::registry::{self, Hive, RegData};
use crate::state::{lock, AppState};
use crate::tweaks::regtweak::{self, RegSpec};
use crate::tweaks::{get, revert_via_journal, Status};
use std::sync::OnceLock;

/// Buildnummer van Windows (bv. 22631), 0 als onbekend.
pub fn windows_build() -> u32 {
    static BUILD: OnceLock<u32> = OnceLock::new();
    *BUILD.get_or_init(|| sysinfo::System::kernel_version().and_then(|k| k.trim().parse().ok()).unwrap_or(0))
}

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

// ---------- HAGS ----------

const HAGS_PATH: &str = "SYSTEM\\CurrentControlSet\\Control\\GraphicsDrivers";

fn hags_specs() -> Vec<RegSpec> {
    vec![RegSpec::dword(Hive::Hklm, HAGS_PATH, "HwSchMode", 2, 1)]
}

/// HAGS volgens de videodriver zelf: (ondersteund, nu actief). None als Windows het niet kan zeggen
/// (ouder dan Windows 10 2004). "Nu actief" verandert pas na een herstart van de pc.
fn hags_caps() -> Option<(bool, bool)> {
    use std::ffi::c_void;
    use windows_sys::Wdk::Graphics::Direct3D::{
        D3DKMTCloseAdapter, D3DKMTEnumAdapters2, D3DKMTQueryAdapterInfo, D3DKMT_ADAPTERINFO, D3DKMT_CLOSEADAPTER, D3DKMT_ENUMADAPTERS2,
        D3DKMT_QUERYADAPTERINFO, D3DKMT_WDDM_2_7_CAPS, KMTQAITYPE_WDDM_2_7_CAPS,
    };
    unsafe {
        let mut e: D3DKMT_ENUMADAPTERS2 = std::mem::zeroed();
        if D3DKMTEnumAdapters2(&mut e) != 0 || e.NumAdapters == 0 {
            return None;
        }
        let mut list: Vec<D3DKMT_ADAPTERINFO> = (0..e.NumAdapters).map(|_| std::mem::zeroed()).collect();
        e.pAdapters = list.as_mut_ptr();
        if D3DKMTEnumAdapters2(&mut e) != 0 {
            return None;
        }
        let (mut answered, mut supported, mut enabled) = (false, false, false);
        for a in list.iter().take(e.NumAdapters as usize) {
            let mut caps: D3DKMT_WDDM_2_7_CAPS = std::mem::zeroed();
            let mut q = D3DKMT_QUERYADAPTERINFO {
                hAdapter: a.hAdapter,
                Type: KMTQAITYPE_WDDM_2_7_CAPS,
                pPrivateDriverData: &mut caps as *mut D3DKMT_WDDM_2_7_CAPS as *mut c_void,
                PrivateDriverDataSize: std::mem::size_of::<D3DKMT_WDDM_2_7_CAPS>() as u32,
            };
            if D3DKMTQueryAdapterInfo(&mut q) == 0 {
                // Bit 0: HwSchSupported, bit 1: HwSchEnabled.
                let v = caps.Anonymous.Value;
                answered = true;
                supported |= v & 1 != 0;
                enabled |= v & 2 != 0;
            }
            D3DKMTCloseAdapter(&D3DKMT_CLOSEADAPTER { hAdapter: a.hAdapter });
        }
        answered.then_some((supported, enabled))
    }
}

/// Zonder waarde in het register kiest Windows zelf: dat is niet hetzelfde als uit.
pub fn hags_status() -> AppResult<Status> {
    let cur = registry::read(Hive::Hklm, HAGS_PATH, "HwSchMode")?;
    let caps = hags_caps();
    if let Some((false, _)) = caps {
        return Ok(Status::with_detail(TweakState::Unavailable, Msg::new("hags_unsupported")));
    }
    let live = caps.map(|(_, on)| on);
    let mut st = match cur {
        RegData::Dword(2) => Status::simple(TweakState::On),
        RegData::Absent if live == Some(true) => Status::with_detail(TweakState::On, Msg::new("hags_default_on")),
        RegData::Absent => Status::with_detail(TweakState::Off, Msg::new("hags_default")),
        _ => Status::simple(TweakState::Off),
    };
    st.live = live;
    Ok(st)
}
pub fn hags_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    regtweak::apply(state, get("hags")?, &hags_specs(), session, None, None)
}
pub fn hags_disable(state: &AppState) -> AppResult<()> {
    regtweak::disable(state, get("hags")?, &hags_specs(), None, None)
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

/// Naam van het Ultieme-prestatiesplan in onder meer Engels, Duits, Nederlands en Frans.
fn is_ultimate(guid: &str, name: &str) -> bool {
    let n = name.to_lowercase();
    guid == GUID_ULTIMATE || n.contains("ultim") || n.contains("ultiem") || n.contains("performances optimales")
}

/// Plannen die nezty zelf heeft gemaakt (een kopie, als Windows het plan niet had) en die nog bestaan
/// volgens het logboek: (guid, "high" of "ultimate"). Zo maakt nezty er nooit nog een kopie bij.
fn created_plans(state: &AppState) -> Vec<(String, Option<String>)> {
    lock(&state.journal)
        .entries
        .iter()
        .filter(|e| !e.reverted)
        .filter_map(|e| match &e.payload {
            RevertPayload::PowerPlan { created_guid: Some(g), created_option, .. } => Some((g.to_lowercase(), created_option.clone())),
            _ => None,
        })
        .collect()
}

/// Welke optie hoort bij dit plan: "high", "ultimate" of geen van beide.
fn plan_option(guid: &str, name: &str, created: &[(String, Option<String>)]) -> Option<&'static str> {
    if guid == GUID_HIGH {
        return Some("high");
    }
    if let Some((_, opt)) = created.iter().find(|(g, _)| g == guid) {
        return Some(match opt.as_deref() {
            Some("ultimate") => "ultimate",
            Some(_) => "high",
            None if is_ultimate(guid, name) => "ultimate",
            None => "high",
        });
    }
    is_ultimate(guid, name).then_some("ultimate")
}

pub fn power_plan_status(state: &AppState) -> AppResult<Status> {
    let (guid, name) = active_scheme()?;
    let (st, current_option) = match plan_option(&guid, &name, &created_plans(state)) {
        Some(opt) => (TweakState::On, Some(opt.to_string())),
        None => (TweakState::Off, Some("high".to_string())),
    };
    Ok(Status { state: st, detail: Some(Msg::new("power_current").with("name", name)), current_option, live: None })
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
    let option = if option == "ultimate" { "ultimate" } else { "high" };
    let (old_guid, old_name) = active_scheme()?;
    let made = created_plans(state);
    if plan_option(&old_guid, &old_name, &made) == Some(option) {
        return Ok(());
    }
    // Eerst een bestaand plan zoeken (ook een kopie die nezty eerder maakte), pas daarna een nieuwe kopie.
    let schemes = list_schemes()?;
    let mut created: Option<String> = None;
    let target = match schemes.iter().find(|(g, n)| plan_option(g, n, &made) == Some(option)) {
        Some((g, _)) => g.clone(),
        None => {
            let g = duplicate_scheme(state, if option == "ultimate" { GUID_ULTIMATE } else { GUID_HIGH })?;
            created = Some(g.clone());
            g
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
    let created_option = created.as_ref().map(|_| option.to_string());
    e.payload = RevertPayload::PowerPlan { old_guid, created_guid: created, created_option };
    lock(&state.journal).record(e)?;
    Ok(())
}

pub fn power_plan_disable(state: &AppState) -> AppResult<()> {
    if revert_via_journal(state, "power_plan", None)? {
        if power_plan_status(state)?.state == TweakState::Off {
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
    e.payload = RevertPayload::PowerPlan { old_guid, created_guid: None, created_option: None };
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

/// Windows gebruikt deze instelling pas vanaf Windows 11 22H2 (build 22621). Daaronder doet hij niets.
fn windowed_opt_supported() -> bool {
    windows_build() >= 22621
}

pub fn windowed_opt_status() -> AppResult<Status> {
    if !windowed_opt_supported() {
        return Ok(Status::with_detail(TweakState::Unavailable, Msg::new("windowed_opt_os")));
    }
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
    if !windowed_opt_supported() {
        return Err(AppError::coded("windowed_opt_os"));
    }
    if dx_get(&dx_text(&dx_settings()?), DX_KEY).as_deref() == Some("1") {
        return Ok(());
    }
    windowed_opt_write(state, "1", session, None)
}

pub fn windowed_opt_disable(state: &AppState) -> AppResult<()> {
    let reverted = revert_via_journal(state, "windowed_opt", None)?;
    // Op een Windows-versie zonder deze instelling valt er na het terugzetten niets meer te doen.
    if !windowed_opt_supported() || (reverted && windowed_opt_status()?.state == TweakState::Off) {
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
