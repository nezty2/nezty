//! Houdt in de gaten welke games draaien. Automatisch boosten bestaat niet meer; staat er nog een
//! boost van een oudere versie aan, dan wordt die bij het afsluiten van de game teruggezet.

use crate::boost;
use crate::models::{GameEvent, Msg, TweakState, WatcherStatus};
use crate::state::{lock, AppState};
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
use tauri::{AppHandle, Emitter};
use windows_sys::Win32::Foundation::{CloseHandle, HWND};
use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

/// Bestandsnaam (bv. "cs2.exe") van het proces achter een venster.
fn exe_name_of(hwnd: HWND) -> Option<String> {
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return None;
        }
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len) != 0;
        CloseHandle(h);
        if !ok {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit(['\\', '/']).next().map(|s| s.to_lowercase())
    }
}

/// Het venster van een game dat nu de focus heeft, als dat er is: dan ben je echt aan het spelen.
/// Zonder draaiende game wordt er niets opgevraagd.
pub fn foreground_game(state: &AppState) -> Option<HWND> {
    if lock(&state.running_games).is_empty() {
        return None;
    }
    let fg = unsafe { GetForegroundWindow() };
    if fg.is_null() {
        return None;
    }
    let name = exe_name_of(fg)?;
    let known = |g: &crate::models::Game| g.process_names.iter().any(|n| n.eq_ignore_ascii_case(&name));
    // Na elkaar, niet tegelijk vergrendeld.
    let detected = lock(&state.games).iter().any(known);
    let is_game = detected || lock(&state.settings).custom_games.iter().any(known);
    is_game.then_some(fg)
}

/// Staat er nu een game op de voorgrond? Dan pauzeren metingen die tijdens het spelen niets toevoegen.
pub fn playing(state: &AppState) -> bool {
    foreground_game(state).is_some()
}

const FORTNITE: &str = "epic:fortnite";

pub fn status(state: &AppState) -> WatcherStatus {
    let watching = lock(&state.profiles).iter().filter(|p| p.auto_boost).count() as u32;
    WatcherStatus { running: state.watcher_running.load(Ordering::Relaxed), watching }
}

pub fn start(app: AppHandle, state: Arc<AppState>) {
    let builder = std::thread::Builder::new().name("game-watcher".into());
    let _ = builder.spawn(move || {
        let mut sys = System::new();
        let mut prev: HashSet<String> = HashSet::new();
        // Rondes op rij zonder Fortnite: voor de instellingen die klaarstaan voor na het afsluiten.
        let mut fortnite_gone: u32 = 0;
        state.watcher_running.store(true, Ordering::Relaxed);
        let _ = app.emit("watcher-status", status(&state));
        loop {
            std::thread::sleep(Duration::from_secs(3));
            let games = state.all_games();
            if games.is_empty() {
                crate::tweaks::smoothness::release_all_bg_tamer();
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
            let timer_on = crate::tweaks::status(&state, "timer_resolution", None).state == TweakState::On;
            crate::timer::hold(timer_on && !now.is_empty());

            // Achtergrond-Temmer: zet bekende achtergrond-apps op EcoQoS / lage prioriteit tijdens het spelen.
            // Raakt nooit het gameproces zelf aan (nul anti-cheat risico).
            let bg_tamer_on = crate::tweaks::status(&state, "background_tamer", None).state == TweakState::On;
            let active_game_procs: HashSet<String> = games
                .iter()
                .filter(|g| now.contains(&g.id))
                .flat_map(|g| g.process_names.iter().map(|n| n.to_lowercase()))
                .collect();
            crate::tweaks::smoothness::sync_bg_tamer(bg_tamer_on && !now.is_empty(), &sys, &active_game_procs);

            // Fortnite schrijft zijn ini bij het afsluiten. Pas na twee rondes zonder Fortnite (zo'n 6 seconden)
            // zetten wat tijdens het spelen is gekozen, zodat Fortnite het niet meer overschrijft.
            fortnite_gone = if now.contains(FORTNITE) { 0 } else { fortnite_gone.saturating_add(1) };
            if fortnite_gone >= 2 && !lock(&state.settings).fortnite_pending.is_empty() {
                match crate::fortnite::apply_pending(&state) {
                    Some(Ok(_)) => boost::notify(&app, "success", Msg::new("fortnite_pending_done")),
                    Some(Err(e)) => boost::notify(&app, "error", Msg::new("fortnite_pending_failed").with("detail", e.to_string())),
                    None => {}
                }
            }

            for id in now.difference(&prev) {
                let Some(game) = games.iter().find(|g| &g.id == id) else { continue };
                let _ = app.emit("game-started", GameEvent { game_id: id.clone(), name: game.name.clone() });
                // Apps die bij een spelstart dicht moeten (Achtergrond-apps, vinkje "Sluiten bij spelstart").
                let settings = state.settings_clone();
                if (!settings.close_during_gaming.is_empty() || settings.auto_close_unneeded) && crate::license::require(&state).is_ok() {
                    let own: Vec<String> = game.process_names.iter().map(|n| n.to_lowercase()).collect();
                    let mut names: Vec<String> = settings.close_during_gaming.clone();
                    if settings.auto_close_unneeded {
                        names.extend(crate::processes::list(&state).into_iter().filter(|p| p.unneeded).map(|p| p.name));
                    }
                    let names: Vec<String> = names.into_iter().filter(|n| !own.contains(&n.to_lowercase())).collect();
                    let r = crate::processes::close_many(&state, &names, None, Some(id));
                    if !r.closed.is_empty() {
                        boost::notify(&app, "info", Msg::new("apps_closed_for_game").with("game", &game.name).with("count", r.closed.len()));
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
