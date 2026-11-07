//! Netwerktweaks: wifi-energiebesparing uit, Multimedia-scheduler & netwerkthrottling uit (MMCSS) en
//! energiebesparing / Green Ethernet op fysieke netwerkkaarten uit. Alles omkeerbaar via het logboek.

use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, RevertPayload};
use crate::models::{JournalKind, Msg, TweakState};
use crate::registry::{self, Hive, RegData};
use crate::state::{lock, AppState};
use crate::tweaks::input::{read_power_setting, write_power_setting};
use crate::tweaks::regtweak::{self, RegSpec};
use crate::tweaks::{get, revert_via_journal, system, Status};

// ---------- Wifi-energiebesparing ----------

/// Energiebeheer > Instellingen draadloze adapter > Energiebesparingsmodus. 0 = maximale prestaties.
const WIFI_SUB: &str = "19cbb8fa-5279-450e-9fac-8a3d5fedd0c1";
const WIFI_SETTING: &str = "12bbebe6-58d6-4636-95bb-3217ef867c1a";

pub fn wifi_status(has_wifi: bool) -> AppResult<Status> {
    if !has_wifi {
        return Ok(Status::with_detail(TweakState::Unavailable, Msg::new("wifi_none")));
    }
    let (scheme, _) = system::active_scheme()?;
    match read_power_setting(&scheme, WIFI_SUB, WIFI_SETTING)? {
        None => Ok(Status::with_detail(TweakState::Unavailable, Msg::new("wifi_none"))),
        Some((ac, dc)) => Ok(Status::simple(if ac == 0 && dc == 0 { TweakState::On } else { TweakState::Off })),
    }
}

pub fn wifi_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    let d = get("wifi_power_max")?;
    let (scheme, scheme_name) = system::active_scheme()?;
    let Some((old_ac, old_dc)) = read_power_setting(&scheme, WIFI_SUB, WIFI_SETTING)? else {
        return Err(AppError::coded("wifi_none"));
    };
    if old_ac == 0 && old_dc == 0 {
        return Ok(());
    }
    let mut e = JournalEntry::new(d.id, d.name, JournalKind::PowerSetting, &format!("{scheme_name}: wireless adapter power saving"));
    e.old_value = Some(format!("AC {old_ac}, DC {old_dc}"));
    e.new_value = Some("AC 0, DC 0".into());
    e.revertable = true;
    e.session_id = session.map(|s| s.to_string());
    e.payload = RevertPayload::PowerSetting { scheme, sub: WIFI_SUB.into(), setting: WIFI_SETTING.into(), ac: old_ac, dc: old_dc };
    lock(&state.journal).write_power(e, 0, 0)?;
    Ok(())
}

pub fn wifi_disable(state: &AppState) -> AppResult<()> {
    if revert_via_journal(state, "wifi_power_max", None)? {
        return Ok(());
    }
    // Geen logboekregel: terug naar de standaard van Windows (op netstroom maximale prestaties, op accu gemiddeld).
    let (scheme, _) = system::active_scheme()?;
    write_power_setting(&scheme, WIFI_SUB, WIFI_SETTING, 0, 2)
}

/// Is er een wifi-adapter? (WLAN AutoConfig houdt per wifi-adapter een sleutel bij.)
pub fn has_wifi() -> bool {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    winreg::RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\WlanSvc\Interfaces")
        .map(|k| k.enum_keys().count() > 0)
        .unwrap_or(false)
}

// ---------- Multimedia Class Scheduler (MMCSS) & netwerkthrottling ----------

fn mmcss_specs() -> Vec<RegSpec> {
    let sys = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile";
    let games = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks\Games";
    vec![
        // Standaard knijpt Windows niet-multimedia netwerkverkeer tot 10 pakketten/ms zodra audio/game draait.
        // 0xffffffff zet die pakketrem uit zodat UDP-gamepakketten en voice niet in wachtrijen belanden.
        RegSpec::dword(Hive::Hklm, sys, "NetworkThrottlingIndex", 0xFFFF_FFFF, 10),
        // Reserveer minimaal 90% CPU-tijd voor games/multimedia (10% achtergrond in plaats van standaard 20%).
        RegSpec::dword(Hive::Hklm, sys, "SystemResponsiveness", 10, 20),
        RegSpec::dword(Hive::Hklm, games, "GPU Priority", 8, 8),
        RegSpec::dword(Hive::Hklm, games, "Priority", 6, 2),
        RegSpec::string(Hive::Hklm, games, "Scheduling Category", "High", "Medium"),
        RegSpec::string(Hive::Hklm, games, "SFIO Priority", "High", "Normal"),
    ]
}

pub fn mmcss_status() -> AppResult<Status> {
    Ok(Status::simple(regtweak::state_of(&mmcss_specs())?))
}

pub fn mmcss_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    regtweak::apply(state, get("mmcss_gaming")?, &mmcss_specs(), session, None, None)
}

pub fn mmcss_disable(state: &AppState) -> AppResult<()> {
    regtweak::disable(state, get("mmcss_gaming")?, &mmcss_specs(), None, None)
}

// ---------- Ethernet-energiebesparing & Green Ethernet uit ----------

const NET_CLASS: &str = r"SYSTEM\CurrentControlSet\Control\Class\{4d36e972-e325-11ce-bfc1-08002be10318}";
const ETHERNET_POWER_KEYWORDS: &[&str] = &[
    "*EEE",
    "AdvancedEEE",
    "EnableGreenEthernet",
    "GreenEthernet",
    "GigaLite",
    "PowerSavingMode",
    "EEELinkAdvertisement",
    "ULPMode",
    "ReduceSpeedOnPowerDown",
];

fn is_physical_ethernet_key(sub_path: &str) -> bool {
    let if_type = match registry::read(Hive::Hklm, sub_path, "*IfType") {
        Ok(RegData::Dword(6)) => true,
        Ok(RegData::Str(s)) if s.trim() == "6" => true,
        _ => false,
    };
    if !if_type {
        return false;
    }
    let hw_id = registry::read(Hive::Hklm, sub_path, "ComponentId")
        .ok()
        .and_then(|d| d.as_str().map(|s| s.to_lowercase()))
        .or_else(|| {
            registry::read(Hive::Hklm, sub_path, "MatchingDeviceId")
                .ok()
                .and_then(|d| d.as_str().map(|s| s.to_lowercase()))
        })
        .unwrap_or_default();
    hw_id.starts_with("pci\\") || hw_id.starts_with("usb\\")
}

fn ethernet_power_specs() -> AppResult<Vec<RegSpec>> {
    let subs = registry::subkeys(Hive::Hklm, NET_CLASS)?;
    let mut specs = Vec::new();
    for sub in subs {
        if sub.len() != 4 || !sub.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let sub_path = format!("{NET_CLASS}\\{sub}");
        if !is_physical_ethernet_key(&sub_path) {
            continue;
        }
        let params_path = format!("{sub_path}\\Ndi\\params");
        if let Ok(params) = registry::subkeys(Hive::Hklm, &params_path) {
            for kw in ETHERNET_POWER_KEYWORDS {
                let Some(actual) = params.iter().find(|p| p.eq_ignore_ascii_case(kw)) else {
                    continue;
                };
                let def_val = registry::read(Hive::Hklm, &format!("{params_path}\\{actual}"), "default")
                    .ok()
                    .and_then(|d| d.as_str().map(|s| s.trim().to_string()))
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "1".to_string());
                specs.push(RegSpec::string(Hive::Hklm, &sub_path, actual, "0", &def_val));
            }
        }
        // PnPCapabilities = 24 (0x18) staat gelijk aan het uitvinken van
        // "De computer mag dit apparaat uitschakelen om energie te besparen" in Apparaatbeheer.
        specs.push(RegSpec::dword(Hive::Hklm, &sub_path, "PnPCapabilities", 24, 0));
    }
    Ok(specs)
}

pub fn ethernet_power_status() -> AppResult<Status> {
    let specs = ethernet_power_specs()?;
    if specs.is_empty() {
        return Ok(Status::with_detail(TweakState::Unavailable, Msg::new("ethernet_none")));
    }
    Ok(Status::simple(regtweak::state_of(&specs)?))
}

pub fn ethernet_power_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    let specs = ethernet_power_specs()?;
    if specs.is_empty() {
        return Err(AppError::coded("ethernet_none"));
    }
    regtweak::apply(state, get("ethernet_power_off")?, &specs, session, None, None)
}

pub fn ethernet_power_disable(state: &AppState) -> AppResult<()> {
    let specs = ethernet_power_specs()?;
    if specs.is_empty() {
        return Ok(());
    }
    regtweak::disable(state, get("ethernet_power_off")?, &specs, None, None)
}
