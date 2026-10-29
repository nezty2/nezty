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

/// `--selftest-roundtrip <uitvoer.json>`: zet aanbevolen waarden in KOPIEËN van echte configbestanden
/// (Fortnite, Discord, browsers), zet ze via het logboek terug en controleert dat alles weer gelijk is.
/// Leest daarnaast de echte Fortnite-, overlay- en hardwareversnellingsstatus uit. Wijzigt niets echts.
/// `--selftest-network <uitvoer.json>`: bufferbloat-test, verkeer per programma (3 s) en de status van de
/// nieuwe netwerktweaks. Wijzigt niets.
pub fn run_network(out: &str) {
    let state = AppState::new(data_dir(true), winapi::is_elevated());
    let mut report = serde_json::Map::new();
    report.insert("elevated".into(), json!(state.elevated));
    report.insert("bandwidth".into(), json!(crate::bandwidth::measure(&state, 3.0)));
    // Analyse van een nagebootste PresentMon-meting: 1000 frames van 5 ms met elke 100 frames een piek van 25 ms.
    let mut csv = String::from("Application,ProcessID,FrameTime,CPUBusy,GPUBusy\n");
    for i in 0..1000 {
        let ft = if i % 100 == 50 { 25.0 } else { 5.0 };
        csv.push_str(&format!("game.exe,1,{ft},1.0,4.8\n"));
    }
    let detail = crate::measure::analyze(&csv, 10.0);
    report.insert("analyze".into(), json!(detail.map(|d| json!({"avg": d.avg_fps, "low1": d.low1_fps, "stutters": d.stutters, "perMin": d.stutters_per_min, "median": d.median_ms, "gpuBusy": d.gpu_busy, "cpuBusy": d.cpu_busy, "graphLen": d.graph.len(), "graphMax": d.graph.iter().cloned().fold(0f32, f32::max)}))));
    report.insert("bufferbloat".into(), json!(crate::bufferbloat::run_with(|_| {}).map_err(|e| format!("{e:?}"))));
    for id in ["wifi_power_max", "delivery_optimization_off", "adapter_power_saving_off"] {
        let s = tweaks::status(&state, id, None);
        report.insert(id.into(), json!({ "state": s.state, "detail": s.detail }));
    }
    let _ = std::fs::write(out, serde_json::to_string_pretty(&serde_json::Value::Object(report)).unwrap_or_default());
    let _ = std::fs::remove_dir_all(&state.data_dir);
}

pub fn run_roundtrip(out: &str) {
    use crate::journal::{execute_revert, RevertPayload};
    use crate::{fortnite, hwaccel, jsonfile, overlays};
    let state = AppState::new(data_dir(true), winapi::is_elevated());
    *lock(&state.games) = games::detect_all();
    let tmp = state.data_dir.join("copies");
    let _ = std::fs::create_dir_all(&tmp);
    let mut report = serde_json::Map::new();
    report.insert("fortnite_overview".into(), json!(fortnite::overview(&state).map_err(|e| e.to_string())));
    report.insert("overlays".into(), json!(overlays::list(&state)));
    report.insert("hw_accel".into(), json!(hwaccel::list(&state)));

    // Fortnite: alle aanbevelingen op een kopie, daarna terugzetten.
    let ini = fortnite::ini_path();
    if ini.is_file() {
        let copy = tmp.join("GameUserSettings.ini");
        let result = (|| -> crate::error::AppResult<serde_json::Value> {
            std::fs::copy(&ini, &copy)?;
            let original = std::fs::read(&copy)?;
            let (text, _) = crate::ini::read_text(&copy)?;
            let ids: Vec<String> = ["vsync", "fps_limit", "reflex", "render_mode"].iter().map(|s| s.to_string()).collect();
            let changes = fortnite::changes_for(&fortnite::checks_for(&text, true, 144), &ids);
            let entries = fortnite::write_changes(&copy, &changes)?;
            let after = fortnite::checks_for(&crate::ini::read_text(&copy)?.0, true, 144);
            for e in entries.iter().rev() {
                execute_revert(&e.payload)?;
            }
            Ok(json!({
                "changed": changes.iter().map(|(s, k, v)| format!("{s} {k}={v}")).collect::<Vec<_>>(),
                "all_ok_after_apply": after.iter().all(|c| c.ok),
                "restored_identical": std::fs::read(&copy)? == original,
            }))
        })();
        report.insert("fortnite_roundtrip".into(), json!(result.map_err(|e| e.to_string())));
    }

    // JSON-instellingen (Discord en Chrome): uitzetten op een kopie, daarna terugzetten.
    let files = [
        ("discord", std::env::var("APPDATA").unwrap_or_default() + r"\discord\settings.json", "/enableHardwareAcceleration", true),
        ("chrome", std::env::var("LOCALAPPDATA").unwrap_or_default() + r"\Google\Chrome\User Data\Local State", "/hardware_acceleration_mode/enabled", false),
    ];
    for (id, path, pointer, pretty) in files {
        if !std::path::Path::new(&path).is_file() {
            continue;
        }
        let copy = tmp.join(format!("{id}.json"));
        let result = (|| -> crate::error::AppResult<serde_json::Value> {
            std::fs::copy(&path, &copy)?;
            let before = jsonfile::read(&copy)?;
            let old = jsonfile::get(&copy, pointer)?;
            jsonfile::set_pointer(&copy, pointer, Some(serde_json::Value::Bool(false)), pretty)?;
            let set_ok = jsonfile::get(&copy, pointer)? == Some(serde_json::Value::Bool(false));
            execute_revert(&RevertPayload::JsonKey {
                path: copy.to_string_lossy().to_string(),
                pointer: pointer.into(),
                old: old.as_ref().map(|v| v.to_string()),
                pretty,
            })?;
            Ok(json!({ "old": old, "set_false": set_ok, "restored_equal": jsonfile::read(&copy)? == before }))
        })();
        report.insert(format!("{id}_roundtrip"), json!(result.map_err(|e| e.to_string())));
    }

    // Andere games: V-Sync in een kopie van het configbestand zetten en weer terugzetten.
    for (game_id, format, key, value) in [("minecraft:java", "colon", "enableVsync", "true"), ("steam:cs2", "vdf", "setting.mat_vsync", "1")] {
        report.insert(format!("{game_id}_config"), json!(crate::gameconfig::list(game_id)));
        let Some(first) = crate::gameconfig::list(game_id).into_iter().next() else { continue };
        let copy = tmp.join(format!("{}.txt", game_id.replace(':', "_")));
        let result = (|| -> crate::error::AppResult<serde_json::Value> {
            std::fs::copy(&first.file, &copy)?;
            let original = std::fs::read(&copy)?;
            let text = std::fs::read_to_string(&copy)?;
            let old = crate::gameconfig::read_key(&text, format, key).unwrap_or_default();
            // Zet de tegenovergestelde waarde, dan terug: de opmaak moet exact gelijk blijven.
            let changed = crate::gameconfig::write_key(&text, format, key, value).ok_or_else(|| crate::error::AppError::msg("key not found"))?;
            std::fs::write(&copy, &changed)?;
            let now = crate::gameconfig::read_key(&std::fs::read_to_string(&copy)?, format, key);
            execute_revert(&RevertPayload::TextKey { path: copy.to_string_lossy().to_string(), format: format.into(), key: key.into(), old })?;
            Ok(json!({ "written": now.as_deref() == Some(value), "restored_identical": std::fs::read(&copy)? == original }))
        })();
        report.insert(format!("{game_id}_roundtrip"), json!(result.map_err(|e| e.to_string())));
    }
    report.insert("input_live".into(), json!(crate::tweaks::input::verify_live()));
    for id in ["timer_resolution", "interrupt_moderation_off"] {
        let s = tweaks::status(&state, id, None);
        report.insert(id.into(), json!({ "state": s.state, "detail": s.detail }));
    }
    let _ = std::fs::write(out, serde_json::to_string_pretty(&serde_json::Value::Object(report)).unwrap_or_default());
    let _ = std::fs::remove_dir_all(&state.data_dir);
}
