//! Netwerktweaks. Alleen de energiebesparing van de adapter heeft een duidelijk effect; de rest is
//! bewust als "experimenteel" gemarkeerd.

use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, RevertPayload};
use crate::models::{JournalKind, Msg, TweakState};
use crate::powershell::{ps_quote, run_ps};
use crate::registry::{self, Hive, RegData};
use crate::state::{lock, AppState};
use crate::tweaks::regtweak::{self, RegSpec};
use crate::tweaks::{get, revert_via_journal, Status};
use serde::Deserialize;

// ---------- Energiebesparing adapter ----------

#[derive(Deserialize)]
struct AdapterPm {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "v")]
    value: String,
}

fn adapters_pm() -> AppResult<Vec<AdapterPm>> {
    let out = run_ps(
        "$a = Get-NetAdapterPowerManagement -ErrorAction SilentlyContinue | Where-Object { [string]$_.AllowComputerToTurnOffDevice -ne 'Unsupported' } | Select-Object Name, @{n='v';e={[string]$_.AllowComputerToTurnOffDevice}}; ConvertTo-Json -InputObject @($a) -Compress",
    )?;
    let trimmed = out.trim();
    if trimmed.is_empty() {
        return Ok(vec![]);
    }
    Ok(serde_json::from_str::<Vec<AdapterPm>>(trimmed).unwrap_or_default())
}

pub fn adapter_power_status(elevated: bool) -> AppResult<Status> {
    let list = adapters_pm()?;
    if list.is_empty() {
        return Ok(if elevated {
            Status::with_detail(TweakState::Unavailable, Msg::new("adapter_none"))
        } else {
            Status::with_detail(TweakState::Unknown, Msg::new("adapter_needs_admin"))
        });
    }
    let all_off = list.iter().all(|a| a.value.eq_ignore_ascii_case("Disabled"));
    let names: Vec<&str> = list.iter().map(|a| a.name.as_str()).collect();
    Ok(Status::with_detail(
        if all_off { TweakState::On } else { TweakState::Off },
        Msg::new("adapter_list").with("names", names.join(", ")),
    ))
}

pub fn adapter_power_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    if !state.elevated {
        return Err(AppError::NeedsAdmin("Set-NetAdapterPowerManagement".into()));
    }
    let d = get("adapter_power_saving_off")?;
    for a in adapters_pm()? {
        if a.value.eq_ignore_ascii_case("Disabled") {
            continue;
        }
        run_ps(&format!("Set-NetAdapterPowerManagement -Name {} -AllowComputerToTurnOffDevice Disabled", ps_quote(&a.name)))?;
        let mut e = JournalEntry::new(d.id, d.name, JournalKind::Adapter, &a.name);
        e.old_value = Some(a.value.clone());
        e.new_value = Some("Disabled".into());
        e.revertable = true;
        e.session_id = session.map(|s| s.to_string());
        e.payload = RevertPayload::Adapter { name: a.name.clone(), old_value: a.value.clone() };
        lock(&state.journal).record(e)?;
    }
    Ok(())
}

pub fn adapter_power_disable(state: &AppState) -> AppResult<()> {
    if !state.elevated {
        return Err(AppError::NeedsAdmin("Set-NetAdapterPowerManagement".into()));
    }
    if revert_via_journal(state, "adapter_power_saving_off", None)? {
        return Ok(());
    }
    let d = get("adapter_power_saving_off")?;
    for a in adapters_pm()? {
        if a.value.eq_ignore_ascii_case("Enabled") {
            continue;
        }
        run_ps(&format!("Set-NetAdapterPowerManagement -Name {} -AllowComputerToTurnOffDevice Enabled", ps_quote(&a.name)))?;
        let mut e = JournalEntry::new(d.id, d.name, JournalKind::Adapter, &a.name);
        e.old_value = Some(a.value.clone());
        e.new_value = Some("Enabled".into());
        e.revertable = true;
        e.note = Some("manual_off".into());
        e.payload = RevertPayload::Adapter { name: a.name.clone(), old_value: a.value.clone() };
        lock(&state.journal).record(e)?;
    }
    Ok(())
}

// ---------- Nagle ----------

const IFACES: &str = "SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters\\Interfaces";

fn active_interfaces() -> AppResult<Vec<String>> {
    let mut out = vec![];
    for key in registry::subkeys(Hive::Hklm, IFACES)? {
        let path = format!("{IFACES}\\{key}");
        let dhcp = registry::read(Hive::Hklm, &path, "DhcpIPAddress")?;
        let fixed = registry::read(Hive::Hklm, &path, "IPAddress")?;
        let has_dhcp = matches!(&dhcp, RegData::Str(s) if !s.is_empty() && s != "0.0.0.0");
        let has_fixed = matches!(&fixed, RegData::MultiStr(v) if v.iter().any(|s| !s.is_empty() && s != "0.0.0.0"));
        if has_dhcp || has_fixed {
            out.push(path);
        }
    }
    Ok(out)
}

fn nagle_specs() -> AppResult<Vec<RegSpec>> {
    let mut specs = vec![];
    for path in active_interfaces()? {
        specs.push(RegSpec::dword(Hive::Hklm, &path, "TcpAckFrequency", 1, 0).off_absent());
        specs.push(RegSpec::dword(Hive::Hklm, &path, "TCPNoDelay", 1, 0).off_absent());
    }
    Ok(specs)
}

pub fn nagle_status() -> AppResult<Status> {
    let specs = nagle_specs()?;
    Ok(Status::with_detail(regtweak::state_of(&specs)?, Msg::new("nagle_ifaces").with("count", specs.len() / 2)))
}
pub fn nagle_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    regtweak::apply(state, get("nagle_off")?, &nagle_specs()?, session, None, None)
}
pub fn nagle_disable(state: &AppState) -> AppResult<()> {
    regtweak::disable(state, get("nagle_off")?, &nagle_specs()?, None, None)
}

// ---------- Netwerkbeperking ----------

fn throttling_specs() -> Vec<RegSpec> {
    vec![RegSpec::dword(
        Hive::Hklm,
        "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile",
        "NetworkThrottlingIndex",
        0xffff_ffff,
        10,
    )]
}
pub fn throttling_status() -> AppResult<Status> {
    Ok(Status::simple(regtweak::state_of(&throttling_specs())?))
}
pub fn throttling_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    regtweak::apply(state, get("network_throttling_off")?, &throttling_specs(), session, None, None)
}
pub fn throttling_disable(state: &AppState) -> AppResult<()> {
    regtweak::disable(state, get("network_throttling_off")?, &throttling_specs(), None, None)
}
