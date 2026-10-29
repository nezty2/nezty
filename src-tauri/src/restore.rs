//! Windows-herstelpunt vóór de eerste tweak.

use crate::models::{now_ms, RestorePointPolicy, RestorePointResult};
use crate::powershell::run_ps;
use crate::registry::{self, Hive, RegData};
use crate::state::{lock, AppState};

/// Vóór de eerste wijziging één keer automatisch een herstelpunt (beleid "Automatisch").
/// Geeft None als er niets te doen is: al gemaakt, beleid staat uit, of het mislukte eerder in deze sessie
/// (dan wacht niet elke volgende tweak opnieuw). `on_start` draait alleen als er echt een herstelpunt
/// gemaakt wordt, want dat duurt even. Lukt het niet, dan gaat de wijziging toch door:
/// het logboek en "Alles terugzetten" blijven werken.
pub fn ensure_before_change(state: &AppState, on_start: impl FnOnce()) -> Option<RestorePointResult> {
    let settings = state.settings_clone();
    if settings.restore_point_policy != RestorePointPolicy::Auto || settings.restore_point_done {
        return None;
    }
    if lock(&state.restore_point).last_result.as_ref().is_some_and(|r| !r.ok) {
        return None;
    }
    if state.elevated && protection_enabled() != Some(false) {
        on_start();
    }
    let r = create(state.elevated);
    if r.ok {
        lock(&state.settings).restore_point_done = true;
        state.save_settings();
    }
    lock(&state.restore_point).last_result = Some(r.clone());
    Some(r)
}

/// Staat Systeembeveiliging aan? (RPSessionInterval = 1). None als onbekend.
pub fn protection_enabled() -> Option<bool> {
    match registry::read(Hive::Hklm, "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\SystemRestore", "RPSessionInterval") {
        Ok(RegData::Dword(v)) => Some(v >= 1),
        _ => None,
    }
}

fn result(ok: bool, key: &str, detail: Option<String>) -> RestorePointResult {
    RestorePointResult { ok, key: key.into(), detail, ts: now_ms() }
}

pub fn create(elevated: bool) -> RestorePointResult {
    if !elevated {
        return result(false, "needs_admin", None);
    }
    if protection_enabled() == Some(false) {
        return result(false, "protection_off", None);
    }
    let script = "$w = $null; Checkpoint-Computer -Description 'nezty: before tweaks' -RestorePointType MODIFY_SETTINGS -WarningVariable w -WarningAction SilentlyContinue; if ($w) { 'WARN: ' + ($w -join ' ') } else { 'OK' }";
    match run_ps(script) {
        Ok(out) => {
            let t = out.trim();
            if t.starts_with("OK") {
                result(true, "created", None)
            } else if t.contains("1440") || t.to_lowercase().contains("already been created") || t.to_lowercase().contains("al is gemaakt") {
                result(true, "recent_exists", None)
            } else {
                result(false, "failed", Some(t.to_string()))
            }
        }
        Err(e) => result(false, "failed", Some(e.to_string())),
    }
}
