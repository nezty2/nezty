//! Netwerktweaks die echt effect hebben: wifi-energiebesparing uit en geen updates meer uploaden naar
//! andere pc's (Delivery Optimization). Windows Update zelf blijft gewoon werken.

use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, RevertPayload};
use crate::models::{JournalKind, Msg, TweakState};
use crate::registry::Hive;
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
    write_power_setting(&scheme, WIFI_SUB, WIFI_SETTING, 0, 0)?;
    let mut e = JournalEntry::new(d.id, d.name, JournalKind::PowerSetting, &format!("{scheme_name}: wireless adapter power saving"));
    e.old_value = Some(format!("AC {old_ac}, DC {old_dc}"));
    e.new_value = Some("AC 0, DC 0".into());
    e.revertable = true;
    e.session_id = session.map(|s| s.to_string());
    e.payload = RevertPayload::PowerSetting { scheme, sub: WIFI_SUB.into(), setting: WIFI_SETTING.into(), ac: old_ac, dc: old_dc };
    lock(&state.journal).record(e)?;
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

// ---------- Delivery Optimization (uploaden naar andere pc's) ----------

/// Dezelfde schakelaar als Instellingen > Windows Update > Geavanceerde opties > Delivery Optimization >
/// "Downloads van andere pc's toestaan". 0 = uit: Windows haalt updates alleen nog bij Microsoft en
/// deelt niets meer met andere pc's.
fn delivery_specs() -> Vec<RegSpec> {
    vec![RegSpec::dword(Hive::Hku, r"S-1-5-20\Software\Microsoft\Windows\CurrentVersion\DeliveryOptimization\Settings", "DownloadMode", 0, 1).off_absent()]
}
pub fn delivery_status() -> AppResult<Status> {
    Ok(Status::simple(regtweak::state_of(&delivery_specs())?))
}
pub fn delivery_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    regtweak::apply(state, get("delivery_optimization_off")?, &delivery_specs(), session, None, None)
}
pub fn delivery_disable(state: &AppState) -> AppResult<()> {
    regtweak::disable(state, get("delivery_optimization_off")?, &delivery_specs(), None, None)
}

/// Is er een wifi-adapter? (WLAN AutoConfig houdt per wifi-adapter een sleutel bij.)
pub fn has_wifi() -> bool {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    winreg::RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\WlanSvc\Interfaces")
        .map(|k| k.enum_keys().count() > 0)
        .unwrap_or(false)
}
