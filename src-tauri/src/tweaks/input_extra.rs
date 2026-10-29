//! Extra muis- en toetsenbordtweaks:
//! - Alt+Shift en Ctrl+Shift wisselen geen taal of toetsenbordindeling meer (midden in een game).
//! - Windows mag je muis en toetsenbord niet meer uitzetten om stroom te besparen
//!   (het vinkje "De computer mag dit apparaat uitschakelen" in Apparaatbeheer).
//! Alles gaat via het logboek en is exact terug te zetten.

use crate::error::{AppError, AppResult};
use crate::journal::{JournalEntry, PostAction, RevertPayload};
use crate::models::{JournalKind, Msg, TweakState};
use crate::powershell::run_ps;
use crate::registry::Hive;
use crate::state::{lock, AppState};
use crate::tweaks::regtweak::{self, RegSpec};
use crate::tweaks::{get, revert_via_journal, Status};
use crate::winapi;
use serde::Deserialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};

// ---------- Taalwissel-sneltoetsen ----------

/// "3" betekent: geen sneltoets. Zonder waarde gebruikt Windows Alt+Shift (taal) en Ctrl+Shift (indeling).
fn lang_specs() -> Vec<RegSpec> {
    let p = "Keyboard Layout\\Toggle";
    vec![
        RegSpec::string(Hive::Hkcu, p, "Hotkey", "3", "1").off_absent(),
        RegSpec::string(Hive::Hkcu, p, "Language Hotkey", "3", "1").off_absent(),
        RegSpec::string(Hive::Hkcu, p, "Layout Hotkey", "3", "2").off_absent(),
    ]
}
pub fn lang_hotkey_status() -> AppResult<Status> {
    Ok(Status::simple(regtweak::state_of(&lang_specs())?))
}
pub fn lang_hotkey_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    regtweak::apply(state, get("lang_hotkey_off")?, &lang_specs(), session, None, Some(PostAction::LangToggleSpi))?;
    winapi::apply_lang_toggle_from_registry();
    Ok(())
}
pub fn lang_hotkey_disable(state: &AppState) -> AppResult<()> {
    regtweak::disable(state, get("lang_hotkey_off")?, &lang_specs(), None, Some(PostAction::LangToggleSpi))?;
    winapi::apply_lang_toggle_from_registry();
    Ok(())
}

// ---------- Muis en toetsenbord niet uitzetten ----------

#[derive(Deserialize, Clone, Debug)]
pub struct DevicePower {
    pub instance: String,
    pub enable: bool,
}

/// PowerShell zoekt de muizen en toetsenborden, loopt hun ouders langs (HID en USB) en geeft de bijbehorende
/// energie-instellingen uit WMI terug. Dat is precies het vinkje op het tabblad Energiebeheer.
const LIST_SCRIPT: &str = r#"
$ids = New-Object System.Collections.Generic.List[string]
Get-PnpDevice -PresentOnly -Class Mouse,Keyboard -ErrorAction SilentlyContinue | ForEach-Object {
  $id = $_.InstanceId
  # Muis of toetsenbord, de USB-interface en het USB-apparaat zelf. Niet hoger: daar zitten de hubs.
  for ($i = 0; $i -lt 3 -and $id; $i++) {
    $ids.Add($id)
    $id = (Get-PnpDeviceProperty -InstanceId $id -KeyName 'DEVPKEY_Device_Parent' -ErrorAction SilentlyContinue).Data
  }
}
$out = @(Get-CimInstance -Namespace root\wmi -ClassName MSPower_DeviceEnable -ErrorAction SilentlyContinue | Where-Object {
  $n = $_.InstanceName
  @($ids | Where-Object { $n.StartsWith($_ + '_', [System.StringComparison]::OrdinalIgnoreCase) }).Count -gt 0
} | ForEach-Object { [pscustomobject]@{ instance = $_.InstanceName; enable = [bool]$_.Enable } })
ConvertTo-Json -InputObject $out -Compress
"#;

static CACHE: Mutex<Option<(Instant, Vec<DevicePower>)>> = Mutex::new(None);
const CACHE_TTL: Duration = Duration::from_secs(30);

fn list_devices(fresh: bool) -> AppResult<Vec<DevicePower>> {
    if !fresh {
        if let Some((at, list)) = CACHE.lock().map_err(|_| AppError::msg("cache"))?.as_ref() {
            if at.elapsed() < CACHE_TTL {
                return Ok(list.clone());
            }
        }
    }
    let out = run_ps(LIST_SCRIPT)?;
    let text = out.trim();
    let list: Vec<DevicePower> = if text.is_empty() { Vec::new() } else { serde_json::from_str(text).map_err(|e| AppError::msg(format!("device power: {e}")))? };
    *CACHE.lock().map_err(|_| AppError::msg("cache"))? = Some((Instant::now(), list.clone()));
    Ok(list)
}

fn forget_cache() {
    if let Ok(mut c) = CACHE.lock() {
        *c = None;
    }
}

/// Zet het vinkje voor één apparaat. Gebruikt ook bij het terugzetten vanuit het logboek.
pub fn set_device_power(instance: &str, enable: bool) -> AppResult<()> {
    let inst = instance.replace('\'', "''");
    let val = if enable { "$true" } else { "$false" };
    run_ps(&format!(
        "$d = @(Get-CimInstance -Namespace root\\wmi -ClassName MSPower_DeviceEnable | Where-Object {{ $_.InstanceName -eq '{inst}' }}); if ($d.Count -eq 0) {{ throw 'device not found' }}; $d | Set-CimInstance -Property @{{ Enable = {val} }}"
    ))?;
    forget_cache();
    Ok(())
}

pub fn device_power_status() -> AppResult<Status> {
    let list = list_devices(false)?;
    if list.is_empty() {
        return Ok(Status::with_detail(TweakState::Unavailable, Msg::new("device_power_none")));
    }
    let state = if list.iter().all(|d| !d.enable) { TweakState::On } else { TweakState::Off };
    Ok(Status::with_detail(state, Msg::new("device_power_count").with("n", list.len().to_string())))
}

fn device_set(state: &AppState, enable: bool, session: Option<&str>, note: Option<&str>) -> AppResult<()> {
    let d = get("input_device_power_off")?;
    let list = list_devices(true)?;
    if list.is_empty() {
        return Err(AppError::coded("device_power_none"));
    }
    for dev in list.iter().filter(|x| x.enable != enable) {
        set_device_power(&dev.instance, enable)?;
        let mut e = JournalEntry::new(d.id, d.name, JournalKind::Setting, &format!("{}: allow the computer to turn off this device", dev.instance));
        e.old_value = Some(dev.enable.to_string());
        e.new_value = Some(enable.to_string());
        e.revertable = true;
        e.session_id = session.map(|s| s.to_string());
        e.note = note.map(|s| s.to_string());
        e.payload = RevertPayload::DevicePower { instance: dev.instance.clone(), enable: dev.enable };
        lock(&state.journal).record(e)?;
    }
    forget_cache();
    Ok(())
}

pub fn device_power_apply(state: &AppState, session: Option<&str>) -> AppResult<()> {
    device_set(state, false, session, None)
}
pub fn device_power_disable(state: &AppState) -> AppResult<()> {
    let reverted = revert_via_journal(state, "input_device_power_off", None)?;
    forget_cache();
    if reverted && device_power_status()?.state == TweakState::Off {
        return Ok(());
    }
    device_set(state, true, None, Some("manual_off"))
}
