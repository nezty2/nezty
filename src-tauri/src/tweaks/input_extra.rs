//! Extra toetsenbordtweak: Alt+Shift en Ctrl+Shift wisselen geen taal of toetsenbordindeling meer
//! (midden in een game). Gaat via het logboek en is exact terug te zetten.

use crate::error::AppResult;
use crate::journal::PostAction;
use crate::powershell::{ps_quote, run_ps};
use crate::registry::Hive;
use crate::state::AppState;
use crate::tweaks::regtweak::{self, RegSpec};
use crate::tweaks::{get, Status};
use crate::winapi;

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

// ---------- Muis en toetsenbord niet uitzetten (weggehaald in 0.5.0) ----------

/// Zet het vinkje "De computer mag dit apparaat uitschakelen" voor één apparaat. Alleen nog nodig om oude
/// logboekregels van de weggehaalde tweak exact terug te zetten.
pub fn set_device_power(instance: &str, enable: bool) -> AppResult<()> {
    let inst = ps_quote(instance);
    let val = if enable { "$true" } else { "$false" };
    run_ps(&format!(
        "$d = @(Get-CimInstance -Namespace root\\wmi -ClassName MSPower_DeviceEnable | Where-Object {{ $_.InstanceName -eq {inst} }}); if ($d.Count -ne 1) {{ throw 'device missing or ambiguous' }}; $d | Set-CimInstance -Property @{{ Enable = {val} }}"
    ))?;
    Ok(())
}
