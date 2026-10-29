//! "Game Boost": past het profiel in één keer toe, met herstelpunt, logboek en een eerlijke
//! voor/na-vergelijking (vrij geheugen, aantal processen, CPU-gebruik en optioneel FPS).

use crate::error::{AppError, AppResult};
use crate::models::*;
use crate::powershell::spawn_detached;
use crate::state::{lock, AppState};
use crate::tweaks::per_game;
use crate::{fps, processes, restore, tweaks};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate};
use tauri::{AppHandle, Emitter};

pub fn snapshot(state: &AppState) -> BoostSnapshot {
    let mut sys = lock(&state.procs);
    sys.refresh_memory();
    sys.refresh_cpu_usage();
    std::thread::sleep(Duration::from_millis(300));
    sys.refresh_cpu_usage();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    BoostSnapshot {
        ts: now_ms(),
        free_ram_mb: sys.available_memory() / 1_048_576,
        process_count: sys.processes().len() as u32,
        cpu_pct: (sys.global_cpu_usage() * 10.0).round() / 10.0,
    }
}

pub fn current_state(state: &AppState) -> BoostState {
    let report = lock(&state.boost).clone();
    BoostState { active: report.is_some(), report }
}

pub fn emit_state(app: &AppHandle, state: &AppState) {
    let _ = app.emit("boost-state", current_state(state));
}

pub fn notify(app: &AppHandle, level: &str, msg: Msg) {
    let _ = app.emit("notice", Notice::new(level, msg));
}

fn progress(app: &AppHandle, stage: &str) {
    let mut params = BTreeMap::new();
    params.insert("stage".to_string(), stage.to_string());
    let _ = app.emit("boost-progress", params);
}

fn reason_of(e: &AppError) -> Msg {
    match e {
        AppError::NeedsAdmin(_) => Msg::new("needs_admin"),
        AppError::Coded { key, params } => Msg { key: key.clone(), params: params.clone() },
        other => Msg::new("error").with("detail", other.to_string()),
    }
}

/// Voert de boost uit. Met `measure_fps` wordt (als de game draait) vóór en na 10 seconden FPS gemeten.
pub fn run_boost(app: &AppHandle, state: &Arc<AppState>, game_id: Option<&str>, measure_fps: bool) -> AppResult<BoostReport> {
    if lock(&state.boost).is_some() {
        return Err(AppError::coded("already_boosting"));
    }
    let game = game_id.and_then(|id| state.all_games().into_iter().find(|g| g.id == id));
    if game_id.is_some() && game.is_none() {
        return Err(AppError::NotFound("game".into()));
    }
    let settings = state.settings_clone();
    let profile = game.as_ref().map(|g| state.profile_for(&g.id));
    let game_running = game.as_ref().map(|g| !per_game::running_pids(state, g).is_empty()).unwrap_or(false);
    let session_id = uuid::Uuid::new_v4().to_string();

    // FPS vooraf meten (alleen als de game draait).
    let mut fps_before = None;
    let mut warnings: Vec<Msg> = vec![];
    if measure_fps {
        match (&game, game_running) {
            (Some(g), true) => {
                progress(app, "measure_before");
                match fps::measure(app, state, g, fps::DEFAULT_SECONDS) {
                    Ok(r) => fps_before = Some(r),
                    Err(e) => warnings.push(reason_of(&e)),
                }
            }
            _ => warnings.push(Msg::new("fps_game_not_running")),
        }
    }

    progress(app, "applying");
    let before = snapshot(state);
    let mut report = BoostReport {
        session_id: session_id.clone(),
        game_id: game.as_ref().map(|g| g.id.clone()),
        game_name: game.as_ref().map(|g| g.name.clone()),
        started_at: now_ms(),
        before: before.clone(),
        after: before,
        applied: vec![],
        already_ok: vec![],
        skipped: vec![],
        closed_processes: vec![],
        restore_point: None,
        game_running,
        warnings,
        fps_before,
        fps_after: None,
    };

    // 1. Herstelpunt vóór de eerste tweak. Mislukte het eerder in deze sessie, dan toont het rapport die uitkomst.
    report.restore_point = restore::ensure_before_change(state, || {}).or_else(|| {
        let s = state.settings_clone();
        (s.restore_point_policy == RestorePointPolicy::Auto && !s.restore_point_done)
            .then(|| lock(&state.restore_point).last_result.clone())
            .flatten()
    });

    // 2. Systeemtweaks uit het profiel. Tweaks die pas na een pc-herstart werken horen niet in een
    //    boost (ze worden aan het eind weer teruggezet en zouden dus nooit effect hebben).
    let tweak_map = profile.as_ref().map(|p| p.global_tweaks.clone()).unwrap_or_else(|| settings.default_profile_tweaks.clone());
    for d in tweaks::CATALOG.iter().filter(|d| d.scope == TweakScope::Global && d.kind == TweakKind::Toggle) {
        if !tweak_map.get(d.id).copied().unwrap_or(false) {
            continue;
        }
        if d.timing == Timing::PcRestart {
            report.skipped.push(SkippedTweak { id: d.id.into(), reason: Msg::new("pc_restart_only") });
            continue;
        }
        let st = tweaks::status(state, d.id, None);
        if st.state == TweakState::On {
            report.already_ok.push(d.id.into());
            continue;
        }
        let option = st.current_option.clone();
        match tweaks::apply(state, d.id, option.as_deref(), Some(&session_id), None).and_then(|_| tweaks::verify(state, d.id, true, None)) {
            Ok(()) => report.applied.push(AppliedTweak { id: d.id.into(), timing: d.timing, option, count: None }),
            Err(e) => report.skipped.push(SkippedTweak { id: d.id.into(), reason: reason_of(&e) }),
        }
    }

    // 3. Per-game tweaks.
    if let (Some(g), Some(p)) = (&game, &profile) {
        for (id, enabled) in [("fullscreen_opt_off", p.fullscreen_opt_off), ("gpu_high_perf", p.gpu_high_perf)] {
            if !enabled {
                continue;
            }
            let d = tweaks::get(id)?;
            if tweaks::status(state, id, Some(g)).state == TweakState::On {
                report.already_ok.push(id.into());
                continue;
            }
            match tweaks::apply(state, id, None, Some(&session_id), Some(g)).and_then(|_| tweaks::verify(state, id, true, Some(g))) {
                Ok(()) => report.applied.push(AppliedTweak { id: id.into(), timing: d.timing, option: None, count: None }),
                Err(e) => report.skipped.push(SkippedTweak { id: id.into(), reason: reason_of(&e) }),
            }
        }
        if p.high_priority {
            match per_game::priority_apply(state, Some(g), Some(&session_id)) {
                Ok(0) => report.skipped.push(SkippedTweak { id: "high_priority".into(), reason: Msg::new("game_not_running_later") }),
                Ok(n) => report.applied.push(AppliedTweak { id: "high_priority".into(), timing: Timing::Instant, option: None, count: Some(n as u32) }),
                Err(e) => report.skipped.push(SkippedTweak { id: "high_priority".into(), reason: reason_of(&e) }),
            }
        }
    }

    // 4. Achtergrond-apps sluiten die de gebruiker heeft gekozen.
    let mut names: Vec<String> = vec![];
    match &profile {
        Some(p) => {
            if p.use_global_close_list {
                names.extend(settings.close_during_gaming.iter().cloned());
            }
            names.extend(p.close_apps.iter().cloned());
        }
        None => names.extend(settings.close_during_gaming.iter().cloned()),
    }
    let mut seen = HashSet::new();
    names.retain(|n| seen.insert(n.to_lowercase()));
    let closed = processes::close_many(state, &names, Some(&session_id), game.as_ref().map(|g| g.id.as_str()));
    report.closed_processes = closed.closed;

    // 5. Even wachten tot het geheugen is vrijgegeven, dan de na-meting.
    std::thread::sleep(Duration::from_millis(1500));
    report.after = snapshot(state);

    if measure_fps && report.fps_before.is_some() {
        if let Some(g) = &game {
            progress(app, "measure_after");
            match fps::measure(app, state, g, fps::DEFAULT_SECONDS) {
                Ok(r) => report.fps_after = Some(r),
                Err(e) => report.warnings.push(reason_of(&e)),
            }
        }
    }
    progress(app, "done");

    *lock(&state.boost) = Some(report.clone());
    emit_state(app, state);
    Ok(report)
}

pub fn unboost(app: &AppHandle, state: &AppState) -> AppResult<RevertResult> {
    let report = lock(&state.boost).take().ok_or_else(|| AppError::coded("no_boost"))?;
    let ids = lock(&state.journal).session_ids(&report.session_id);
    let result = lock(&state.journal).revert_ids(&ids);
    lock(&state.priority_applied).clear();
    emit_state(app, state);
    Ok(result)
}

/// Start de apps die het laatst zijn gesloten opnieuw (als het pad bekend is).
pub fn restart_closed_apps(state: &AppState) -> AppResult<u32> {
    let list = lock(&state.last_closed).clone();
    if list.is_empty() {
        return Err(AppError::coded("nothing_to_restart"));
    }
    let mut n = 0;
    let mut done = HashSet::new();
    for cp in list {
        let Some(exe) = cp.exe_path else { continue };
        if !done.insert(exe.to_lowercase()) {
            continue;
        }
        let cwd = Path::new(&exe).parent().map(|p| p.to_string_lossy().to_string());
        if spawn_detached(&exe, &[], cwd.as_deref()).is_ok() {
            n += 1;
        }
    }
    Ok(n)
}
