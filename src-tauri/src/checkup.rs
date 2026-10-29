//! Test-pagina: controleert of alles in nezty werkt. `run` leest alleen uit en wijzigt niets.
//! `tweak_roundtrip` zet twee onschuldige muis- en toetsenbordtweaks aan, controleert ze en zet ze
//! via het logboek exact terug, zodat ook "Alles terugzetten" getest is.

use crate::error::AppResult;
use crate::models::*;
use crate::state::{lock, AppState};
use crate::{autostart, controller, display, ping, restore, tweaks, updater};
use serde::Serialize;
use std::collections::BTreeMap;
use tauri::AppHandle;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TestStatus {
    Ok,
    Warn,
    Fail,
    Info,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub id: String,
    pub status: TestStatus,
    /// Welke tekst de interface toont, bv. "ok" of "off".
    pub key: String,
    pub params: BTreeMap<String, String>,
}

fn check(id: &str, status: TestStatus, key: &str, params: &[(&str, String)]) -> Check {
    Check {
        id: id.into(),
        status,
        key: key.into(),
        params: params.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
    }
}

pub fn run(app: &AppHandle, state: &AppState) -> Vec<Check> {
    use TestStatus::*;
    let mut out = vec![];

    out.push(if state.elevated { check("admin", Ok, "ok", &[]) } else { check("admin", Fail, "missing", &[]) });

    out.push(match restore::protection_enabled() {
        Some(true) => check("restore", Ok, "ok", &[]),
        Some(false) => check("restore", Warn, "off", &[]),
        None => check("restore", Warn, "unknown", &[]),
    });

    out.push(match &lock(&state.journal).backup {
        None => check("journal", Ok, "ok", &[]),
        Some(p) => check("journal", Warn, "backup", &[("file", p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default())]),
    });

    let probe = state.data_dir.join(".nezty-test");
    out.push(match std::fs::write(&probe, b"ok").and_then(|_| std::fs::remove_file(&probe)) {
        Result::Ok(()) => check("data", Ok, "ok", &[]),
        Err(e) => check("data", Fail, "error", &[("error", e.to_string())]),
    });

    let list = tweaks::list(state);
    let unknown = list.iter().filter(|t| t.state == TweakState::Unknown).count();
    let on = list.iter().filter(|t| t.state == TweakState::On).count();
    out.push(check(
        "tweaks",
        if unknown == 0 { Ok } else { Warn },
        if unknown == 0 { "ok" } else { "unknown" },
        &[("total", list.len().to_string()), ("on", on.to_string()), ("unknown", unknown.to_string())],
    ));

    let games = state.all_games().len();
    out.push(if games > 0 { check("games", Ok, "ok", &[("count", games.to_string())]) } else { check("games", Info, "none", &[]) });

    let displays = display::list();
    match displays.iter().find(|d| d.primary).or(displays.first()) {
        Some(d) if d.max_hz > d.current_hz => out.push(check("display", Warn, "low", &[("current", d.current_hz.to_string()), ("max", d.max_hz.to_string())])),
        Some(d) => out.push(check("display", Ok, "ok", &[("current", d.current_hz.to_string())])),
        None => out.push(check("display", Warn, "none", &[])),
    }

    out.push(match ping::quick("1.1.1.1", 1500) {
        Some(ms) => check("network", Ok, "ok", &[("ms", format!("{ms:.0}"))]),
        None => check("network", Fail, "offline", &[]),
    });

    out.push(if !updater::configured(app) {
        check("update", Warn, "unconfigured", &[])
    } else {
        match updater::check(app) {
            Result::Ok(None) => check("update", Ok, "latest", &[]),
            Result::Ok(Some(info)) => check("update", Info, "available", &[("version", info.version)]),
            Err(_) => check("update", Fail, "error", &[]),
        }
    });

    let live = crate::tweaks::input::verify_live();
    let saved = live.iter().filter(|c| c.saved).count();
    let off = live.iter().filter(|c| c.saved && !c.live).count();
    out.push(if off > 0 {
        check("input_live", Warn, "mismatch", &[("count", off.to_string())])
    } else if saved > 0 {
        check("input_live", Ok, "ok", &[("count", saved.to_string())])
    } else {
        check("input_live", Info, "none", &[])
    });

    out.push(if autostart::is_enabled() { check("autostart", Info, "on", &[]) } else { check("autostart", Info, "off", &[]) });

    let pads = controller::list();
    out.push(if pads.is_empty() { check("controller", Info, "none", &[]) } else { check("controller", Ok, "ok", &[("count", pads.len().to_string())]) });

    out
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RoundtripStep {
    pub id: String,
    /// "ok", "skipped" (stond al aan) of "error".
    pub result: String,
    pub restored: bool,
    pub error: Option<String>,
}

/// Onschuldige tweaks in HKCU die direct werken en direct terug kunnen.
const ROUNDTRIP: [&str; 2] = ["mouse_precision_off", "keyboard_fast_repeat"];

pub fn tweak_roundtrip(state: &AppState) -> AppResult<Vec<RoundtripStep>> {
    let session = format!("test-{}", uuid::Uuid::new_v4());
    let mut steps = vec![];
    let mut before = vec![];
    for id in ROUNDTRIP {
        let was = tweaks::status(state, id, None).state;
        before.push(was);
        if was == TweakState::On {
            steps.push(RoundtripStep { id: id.into(), result: "skipped".into(), restored: true, error: None });
            continue;
        }
        let r = tweaks::apply(state, id, None, Some(&session), None).and_then(|_| tweaks::verify(state, id, true, None));
        steps.push(RoundtripStep {
            id: id.into(),
            result: if r.is_ok() { "ok".into() } else { "error".into() },
            restored: false,
            error: r.err().map(|e| e.to_string()),
        });
    }
    // Terugzetten via het logboek, precies zoals "Alles terugzetten" het doet.
    let ids = lock(&state.journal).session_ids(&session);
    lock(&state.journal).revert_ids(&ids);
    for (step, was) in steps.iter_mut().zip(before) {
        if step.result != "skipped" {
            step.restored = tweaks::status(state, &step.id, None).state == was;
        }
    }
    Ok(steps)
}
