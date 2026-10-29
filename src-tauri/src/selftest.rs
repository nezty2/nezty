//! Verborgen zelftest voor ontwikkelaars: `nzty.exe --selftest <uitvoer.json>` leest alle statussen en
//! controles uit zonder iets te wijzigen. `--selftest-apply <uitvoer.json>` zet daarnaast de muis- en
//! toetsenbordtweaks aan, controleert ze en zet ze exact terug (met een apart, tijdelijk logboek).

use crate::models::*;
use crate::state::{lock, AppState};
use crate::{bigwins, display, games, processes, startup, tweaks, winapi};
use serde_json::json;
use std::path::PathBuf;

fn data_dir(temp: bool) -> PathBuf {
    if temp {
        std::env::temp_dir().join(format!("nzty-selftest-{}", uuid::Uuid::new_v4()))
    } else {
        PathBuf::from(std::env::var("APPDATA").unwrap_or_default()).join("com.nzty.desktop")
    }
}

pub fn run(out: &str, apply: bool) {
    let state = AppState::new(data_dir(apply), winapi::is_elevated());
    *lock(&state.games) = games::detect_all();
    let mut report = serde_json::Map::new();
    report.insert("elevated".into(), json!(state.elevated));
    report.insert("tweaks".into(), json!(tweaks::list(&state)));
    report.insert(
        "displays".into(),
        json!(display::list().iter().map(|d| json!({"device": d.device, "monitor": d.monitor, "w": d.width, "h": d.height, "current": d.current_hz, "max": d.max_hz})).collect::<Vec<_>>()),
    );
    let layout = bigwins::gpu_layout();
    report.insert(
        "gpus".into(),
        json!(layout.gpus.iter().map(|g| json!({"name": g.name, "vendor": g.vendor, "discrete": g.discrete, "driver": g.driver_version, "date": g.driver_date.map(|d| d.to_string())})).collect::<Vec<_>>()),
    );
    report.insert("hybrid".into(), json!(layout.hybrid));
    report.insert("big_wins".into(), json!(bigwins::all(&state)));
    report.insert(
        "games".into(),
        json!(state.all_games().iter().map(|g| json!({"id": g.id, "name": g.name, "exe": g.exe_path, "steam": g.steam_app_id, "procs": g.process_names})).collect::<Vec<_>>()),
    );
    let procs = processes::list(&state);
    report.insert("process_count".into(), json!(procs.len()));
    report.insert("processes_top".into(), json!(procs.iter().take(8).collect::<Vec<_>>()));
    report.insert("startup_count".into(), json!(startup::list().map(|l| l.len()).unwrap_or(0)));
    report.insert("store_background_allowed".into(), json!(processes::store_background_allowed().ok()));

    if apply {
        let mut steps = vec![];
        for id in ["mouse_precision_off", "keyboard_fast_repeat", "accessibility_hotkeys_off", "usb_suspend_off"] {
            let before = tweaks::status(&state, id, None).state;
            let applied = if before == TweakState::On {
                "already_on".to_string()
            } else {
                match tweaks::apply(&state, id, None, Some("selftest"), None).and_then(|_| tweaks::verify(&state, id, true, None)) {
                    Ok(()) => "ok".into(),
                    Err(e) => format!("error: {e}"),
                }
            };
            let after_apply = tweaks::status(&state, id, None).state;
            steps.push(json!({"id": id, "before": before, "apply": applied, "after_apply": after_apply}));
        }
        // Alles terugzetten via het (tijdelijke) logboek en controleren dat de oude status terug is.
        let ids = lock(&state.journal).session_ids("selftest");
        let revert = lock(&state.journal).revert_ids(&ids);
        for step in steps.iter_mut() {
            let id = step["id"].as_str().unwrap_or_default().to_string();
            let now = tweaks::status(&state, &id, None).state;
            step["after_revert"] = json!(now);
            step["restored"] = json!(step["before"] == json!(now));
        }
        report.insert("apply_steps".into(), json!(steps));
        report.insert("revert".into(), json!(revert));
        let _ = std::fs::remove_dir_all(&state.data_dir);
    }
    let _ = std::fs::write(out, serde_json::to_string_pretty(&serde_json::Value::Object(report)).unwrap_or_default());
}
