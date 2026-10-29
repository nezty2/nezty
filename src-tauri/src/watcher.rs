//! Houdt in de gaten welke games draaien. Boost automatisch bij de start en zet terug bij het
//! afsluiten, als dat in het profiel staat.

use crate::boost;
use crate::models::{GameEvent, Msg, WatcherStatus};
use crate::state::{lock, AppState};
use crate::tweaks::per_game;
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
use tauri::{AppHandle, Emitter};

pub fn status(state: &AppState) -> WatcherStatus {
    let watching = lock(&state.profiles).iter().filter(|p| p.auto_boost).count() as u32;
    WatcherStatus { running: state.watcher_running.load(Ordering::Relaxed), watching }
}

pub fn start(app: AppHandle, state: Arc<AppState>) {
    let builder = std::thread::Builder::new().name("game-watcher".into());
    let _ = builder.spawn(move || {
        let mut sys = System::new();
        let mut prev: HashSet<String> = HashSet::new();
        state.watcher_running.store(true, Ordering::Relaxed);
        let _ = app.emit("watcher-status", status(&state));
        loop {
            std::thread::sleep(Duration::from_secs(3));
            let games = state.all_games();
            if games.is_empty() {
                continue;
            }
            sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
            let running: HashSet<String> = sys.processes().values().map(|p| p.name().to_string_lossy().to_lowercase()).collect();
            let now: HashSet<String> = games
                .iter()
                .filter(|g| g.process_names.iter().any(|n| running.contains(&n.to_lowercase())))
                .map(|g| g.id.clone())
                .collect();
            *lock(&state.running_games) = now.clone();
            // Experimentele timerresolutie: alleen vasthouden terwijl er een game draait.
            let timer_on = crate::tweaks::status(&state, "timer_resolution", None).state == crate::models::TweakState::On;
            crate::timer::hold(timer_on && !now.is_empty());
            let settings = state.settings_clone();

            for id in now.difference(&prev) {
                let Some(game) = games.iter().find(|g| &g.id == id) else { continue };
                let _ = app.emit("game-started", GameEvent { game_id: id.clone(), name: game.name.clone() });
                let profile = state.profile_for(id);
                let boost_active = lock(&state.boost).is_some();
                if settings.auto_boost_enabled && profile.auto_boost && !boost_active {
                    match boost::run_boost(&app, &state, Some(id), false) {
                        Ok(r) => boost::notify(
                            &app,
                            "success",
                            Msg::new("auto_boost_done")
                                .with("game", &game.name)
                                .with("tweaks", r.applied.len())
                                .with("apps", r.closed_processes.len()),
                        ),
                        Err(e) => boost::notify(&app, "error", Msg::new("auto_boost_failed").with("detail", e.to_string())),
                    }
                }
            }

            // Prioriteit toepassen op nieuwe processen van de geboostte game.
            let active = lock(&state.boost).clone();
            if let Some(report) = active {
                if let Some(gid) = &report.game_id {
                    if now.contains(gid) && state.profile_for(gid).high_priority {
                        if let Some(g) = games.iter().find(|g| &g.id == gid) {
                            let _ = per_game::priority_apply(&state, Some(g), Some(&report.session_id));
                        }
                    }
                }
            }

            for id in prev.difference(&now) {
                let name = games.iter().find(|g| &g.id == id).map(|g| g.name.clone()).unwrap_or_else(|| id.clone());
                let _ = app.emit("game-stopped", GameEvent { game_id: id.clone(), name: name.clone() });
                let active_game = lock(&state.boost).as_ref().and_then(|r| r.game_id.clone());
                if active_game.as_deref() == Some(id.as_str()) && state.profile_for(id).revert_on_exit {
                    match boost::unboost(&app, &state) {
                        Ok(r) => boost::notify(&app, "info", Msg::new("auto_revert_done").with("game", &name).with("count", r.reverted)),
                        Err(e) => boost::notify(&app, "error", Msg::new("auto_revert_failed").with("detail", e.to_string())),
                    }
                }
            }
            prev = now;
        }
    });
}
