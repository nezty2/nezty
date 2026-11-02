//! Houdt in de gaten welke games draaien. Automatisch boosten bestaat niet meer; staat er nog een
//! boost van een oudere versie aan, dan wordt die bij het afsluiten van de game teruggezet.

use crate::boost;
use crate::models::{GameEvent, Msg, WatcherStatus};
use crate::state::{lock, AppState};
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
            // Fijnere timer (0,5 ms) alleen zolang er een game draait en de tweak aan staat.
            let timer_on = crate::tweaks::status(&state, "timer_resolution", None).state == crate::models::TweakState::On;
            crate::timer::hold(timer_on && !now.is_empty());

            for id in now.difference(&prev) {
                let Some(game) = games.iter().find(|g| &g.id == id) else { continue };
                let _ = app.emit("game-started", GameEvent { game_id: id.clone(), name: game.name.clone() });
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
