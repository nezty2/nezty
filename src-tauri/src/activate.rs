//! "Alles activeren": in één keer alles wat op dit apparaat echt helpt. Elk onderdeel staat los: mislukt er
//! één, dan gaat de rest gewoon door. Wat niet bij het apparaat past, slaat nezty over. Alles komt in het
//! logboek en is exact terug te zetten.

use crate::boost::reason_of;
use crate::models::*;
use crate::state::{lock, AppState};
use crate::{bigwins, fortnite, gameconfig, tweaks};
use tauri::{AppHandle, Emitter};

/// Big-win-controles met een veilige knop. Energiebron eerst: die wisselt eventueel het energieplan, en de
/// laptop- en wifi-instellingen daarna komen dan in het juiste plan terecht.
const BIG_WINS: &[&str] = &["power_source", "refresh_rate"];
/// Fortnite-instellingen die in "Alles activeren" zitten (Reflex alleen met een NVIDIA-kaart).
const FORTNITE_IDS: &[&str] = &["vsync", "reflex", "fullscreen"];
const FORTNITE_GAME: &str = "epic:fortnite";

fn item(id: &str, game_id: Option<&str>, outcome: ApplyOutcome, reason: Option<Msg>) -> ApplyItem {
    ApplyItem { id: id.into(), game_id: game_id.map(|g| g.to_string()), outcome, reason }
}

fn progress(app: &AppHandle, step: &str, done: u32, total: u32) {
    let _ = app.emit("apply-progress", ApplyProgress { step: step.into(), done, total });
}

// ---------- Onderdelen ----------

/// Een tweak (algemeen of voor één game) aanzetten als dat kan en nog niet zo is.
fn tweak(state: &AppState, id: &str, game: Option<&Game>) -> ApplyItem {
    let game_id = game.map(|g| g.id.as_str());
    let d = match tweaks::get(id) {
        Ok(d) => d,
        Err(e) => return item(id, game_id, ApplyOutcome::Failed, Some(reason_of(&e))),
    };
    if tweaks::platform_locked(state, d) {
        return item(id, game_id, ApplyOutcome::Skipped, Some(Msg::new("platform_locked")));
    }
    let st = tweaks::status(state, id, game);
    match st.state {
        TweakState::On => {
            // Staat hij alleen aan door een lopende Game Boost, dan zou de boost hem bij het sluiten van de game
            // terugzetten. Los van de boost maken: dan blijft hij aan, net als de rest van Alles activeren.
            if lock(&state.journal).detach_from_session(id, game_id) {
                log::info!("{id} losgemaakt van Game Boost: blijft aan na de boost");
            }
            item(id, game_id, ApplyOutcome::Already, None)
        }
        TweakState::Unavailable => item(id, game_id, ApplyOutcome::Skipped, Some(st.detail.unwrap_or_else(|| Msg::new("not_available")))),
        _ => match tweaks::apply(state, id, st.current_option.as_deref(), None, game).and_then(|_| tweaks::verify(state, id, true, game)) {
            Ok(()) => item(id, game_id, ApplyOutcome::Applied, None),
            Err(e) => item(id, game_id, ApplyOutcome::Failed, Some(reason_of(&e))),
        },
    }
}

/// Een big-win-controle oplossen als er iets op te lossen valt en nezty dat veilig zelf kan.
fn big_win(state: &AppState, id: &str) -> ApplyItem {
    let Some(w) = bigwins::check_one(state, id) else {
        return item(id, None, ApplyOutcome::Skipped, Some(Msg::new("not_available")));
    };
    match (w.status, w.action) {
        (CheckStatus::Ok, _) => item(id, None, ApplyOutcome::Already, None),
        (CheckStatus::Issue, CheckAction::Fix) => match bigwins::fix(state, id) {
            Ok(()) => item(id, None, ApplyOutcome::Applied, None),
            Err(e) => item(id, None, ApplyOutcome::Failed, Some(reason_of(&e))),
        },
        (CheckStatus::Unavailable, _) => item(id, None, ApplyOutcome::Skipped, Some(Msg::new("not_available"))),
        // Bv. op de accu: dat moet de gebruiker zelf doen.
        _ => item(id, None, ApplyOutcome::Skipped, Some(Msg::new("manual_fix"))),
    }
}

/// De game op de snelle videokaart. Alleen zinvol met twee videokaarten.
fn gpu_games(state: &AppState, games: &[Game], out: &mut Vec<ApplyItem>) {
    if !bigwins::gpu_layout().hybrid {
        out.push(item("gpu_high_perf", None, ApplyOutcome::Skipped, Some(Msg::new("not_hybrid"))));
        return;
    }
    for g in games.iter().filter(|g| g.exe_path.is_some()) {
        out.push(tweak(state, "gpu_high_perf", Some(g)));
    }
}

/// V-Sync uit en NVIDIA Reflex aan in Fortnite. Alleen als het instellingenbestand er is en Fortnite dicht is.
fn fortnite_checks(state: &AppState, out: &mut Vec<ApplyItem>) {
    if !fortnite::ini_path().is_file() {
        return;
    }
    let ov = match fortnite::overview(state) {
        Ok(ov) => ov,
        Err(e) => {
            out.push(item("fortnite_vsync", Some(FORTNITE_GAME), ApplyOutcome::Failed, Some(reason_of(&e))));
            return;
        }
    };
    let wanted: Vec<&FortniteCheck> = ov.checks.iter().filter(|c| FORTNITE_IDS.contains(&c.id.as_str())).collect();
    let todo: Vec<String> = wanted.iter().filter(|c| !c.ok).map(|c| c.id.clone()).collect();
    // Fortnite open: klaarzetten. De watcher zet het zodra Fortnite dicht is.
    if ov.running && !todo.is_empty() {
        if let Err(e) = fortnite::queue(state, &todo) {
            log::warn!("fortnite klaarzetten: {e}");
        }
    }
    let after = if todo.is_empty() || ov.running { Ok(ov.clone()) } else { fortnite::apply_recommended(state, &todo) };
    for c in wanted {
        let id = format!("fortnite_{}", c.id);
        let it = if c.ok {
            item(&id, Some(FORTNITE_GAME), ApplyOutcome::Already, None)
        } else if ov.running {
            item(&id, Some(FORTNITE_GAME), ApplyOutcome::Skipped, Some(Msg::new("fortnite_queued")))
        } else {
            match &after {
                Ok(a) if a.checks.iter().any(|x| x.id == c.id && x.ok) => item(&id, Some(FORTNITE_GAME), ApplyOutcome::Applied, None),
                Ok(_) => item(&id, Some(FORTNITE_GAME), ApplyOutcome::Failed, Some(Msg::new("verify_failed"))),
                Err(e) => item(&id, Some(FORTNITE_GAME), ApplyOutcome::Failed, Some(reason_of(e))),
            }
        };
        out.push(it);
    }
}

/// V-Sync uit in het configbestand van één game (CS2, Minecraft Java), als de game dicht is.
fn game_config(state: &AppState, game_id: &str) -> Option<ApplyItem> {
    let checks = gameconfig::list(game_id);
    if checks.is_empty() {
        return None;
    }
    if checks.iter().all(|c| c.ok) {
        return Some(item("game_vsync", Some(game_id), ApplyOutcome::Already, None));
    }
    if gameconfig::is_running(state, game_id) {
        return Some(item("game_vsync", Some(game_id), ApplyOutcome::Skipped, Some(Msg::new("game_running"))));
    }
    Some(match gameconfig::apply(state, game_id) {
        Ok(after) if after.iter().all(|c| c.ok) => item("game_vsync", Some(game_id), ApplyOutcome::Applied, None),
        Ok(_) => item("game_vsync", Some(game_id), ApplyOutcome::Failed, Some(Msg::new("verify_failed"))),
        Err(e) => item("game_vsync", Some(game_id), ApplyOutcome::Failed, Some(reason_of(&e))),
    })
}

/// Games waarvan het deel per game echt gedaan is. Draait een game nog, of ontbreekt zijn exe-pad of
/// instellingenbestand nog, dan komt hij de volgende keer zoeken opnieuw aan de beurt.
fn finished_games(games: &[Game], items: &[ApplyItem]) -> Vec<String> {
    let hybrid = bigwins::gpu_layout().hybrid;
    let config_ids = gameconfig::game_ids();
    games
        .iter()
        .filter(|g| {
            let busy = items.iter().any(|i| i.game_id.as_deref() == Some(g.id.as_str()) && i.outcome == ApplyOutcome::Skipped && i.reason.as_ref().is_some_and(|r| r.key == "game_running"));
            let no_exe = hybrid && g.exe_path.is_none();
            let no_ini = g.id == FORTNITE_GAME && !fortnite::ini_path().is_file();
            let no_cfg = config_ids.contains(&g.id.as_str()) && gameconfig::list(&g.id).is_empty();
            !(busy || no_exe || no_ini || no_cfg)
        })
        .map(|g| g.id.clone())
        .collect()
}

// ---------- Alles activeren ----------

/// Voert "Alles activeren" uit, met voortgang via het event "apply-progress".
pub fn run(app: &AppHandle, state: &AppState) -> ApplyAllReport {
    let games = state.all_games();
    // Stappen: herstelpunt, big wins, de tweaks, videokaart per game, Fortnite, andere games.
    let total = (1 + BIG_WINS.len() + tweaks::ACTIVATE_ALL.len() + 3) as u32;
    let mut done = 0u32;
    let mut step = |name: &str| {
        progress(app, name, done, total);
        done += 1;
    };
    let mut items = vec![];

    step("restore_point");
    let restore_point = crate::restore_point_first(app, state);

    for id in BIG_WINS {
        step(id);
        items.push(big_win(state, id));
    }
    for id in tweaks::ACTIVATE_ALL {
        step(id);
        items.push(tweak(state, id, None));
    }
    step("gpu_high_perf");
    gpu_games(state, &games, &mut items);
    step("fortnite");
    fortnite_checks(state, &mut items);
    step("game_config");
    items.extend(gameconfig::game_ids().into_iter().filter_map(|gid| game_config(state, gid)));

    // Controle achteraf, met dezelfde meting als de ring. Heeft een latere stap een eerdere ongedaan gemaakt,
    // dan die nu nog een keer aanzetten. Staat iets daarna nog steeds uit, dan zegt het rapport eerlijk dat het
    // mislukt is, in plaats van "aangezet".
    for t in status(state).missing {
        let game = t.game_id.as_deref().and_then(|gid| games.iter().find(|g| g.id == gid));
        let retry = if BIG_WINS.contains(&t.id.as_str()) {
            big_win(state, &t.id)
        } else if tweaks::ACTIVATE_ALL.contains(&t.id.as_str()) || t.id == "gpu_high_perf" {
            tweak(state, &t.id, game)
        } else {
            continue;
        };
        log::warn!("alles activeren: {} stond na de ronde weer uit, opnieuw aangezet", t.id);
        if let Some(it) = items.iter_mut().find(|i| i.id == t.id && i.game_id == t.game_id) {
            match retry.outcome {
                ApplyOutcome::Applied | ApplyOutcome::Already => {
                    it.outcome = ApplyOutcome::Applied;
                    it.reason = None;
                }
                _ => {
                    it.outcome = retry.outcome;
                    it.reason = retry.reason;
                }
            }
        }
    }
    for t in status(state).missing {
        if let Some(it) = items.iter_mut().find(|i| i.id == t.id && i.game_id == t.game_id && i.outcome != ApplyOutcome::Failed) {
            it.outcome = ApplyOutcome::Failed;
            it.reason = Some(Msg::new("verify_failed"));
        }
    }
    progress(app, "done", total, total);

    {
        let done = finished_games(&games, &items);
        let mut s = lock(&state.settings);
        s.all_active = true;
        s.activated_games = done;
    }
    state.save_settings();

    let count = |o: ApplyOutcome| items.iter().filter(|i| i.outcome == o).count() as u32;
    let report = ApplyAllReport {
        applied: count(ApplyOutcome::Applied),
        already: count(ApplyOutcome::Already),
        skipped: count(ApplyOutcome::Skipped),
        failed: count(ApplyOutcome::Failed),
        items,
        restore_point,
        restart: crate::restart::status(state),
    };
    log::info!("alles activeren: {} aangezet, {} stond al goed, {} overgeslagen, {} mislukt", report.applied, report.already, report.skipped, report.failed);
    report
}

/// "Alles activeren" geldt niet meer (alles is teruggezet): nieuwe games krijgen dan ook niets meer vanzelf.
pub fn forget(state: &AppState) {
    let changed = {
        let mut s = lock(&state.settings);
        let changed = s.all_active || !s.activated_games.is_empty();
        s.all_active = false;
        s.activated_games.clear();
        changed
    };
    if changed {
        state.save_settings();
    }
}

/// Na het zoeken naar games of het toevoegen van een eigen game: staat "Alles activeren" aan, dan krijgen
/// nieuwe games ook hun instellingen per game. Een game die draait, komt de volgende keer opnieuw aan de beurt.
pub fn apply_new_games(state: &AppState) {
    let settings = state.settings_clone();
    if !settings.all_active || crate::license::require(state).is_err() {
        return;
    }
    let new: Vec<Game> = state.all_games().into_iter().filter(|g| !settings.activated_games.contains(&g.id)).collect();
    if new.is_empty() {
        return;
    }
    let mut items = vec![];
    gpu_games(state, &new, &mut items);
    for g in &new {
        if g.id == FORTNITE_GAME {
            fortnite_checks(state, &mut items);
        }
        if gameconfig::game_ids().contains(&g.id.as_str()) {
            items.extend(game_config(state, &g.id));
        }
    }
    let done = finished_games(&new, &items);
    log::info!("alles activeren voor {} nieuwe game(s): {} onderdelen", done.len(), items.len());
    lock(&state.settings).activated_games.extend(done);
    state.save_settings();
}

// ---------- Status ----------

/// Staat alles uit "Alles activeren" aan? Wijzigt niets.
pub fn status(state: &AppState) -> ApplyAllStatus {
    // (id, game, stand). Wat niet bij het apparaat past, telt niet mee.
    #[derive(PartialEq)]
    enum Row {
        Off,
        On,
        /// Staat aan, maar werkt pas als de game opnieuw start.
        AfterGame,
    }
    let on_off = |on: bool| if on { Row::On } else { Row::Off };
    // Aan, en ook echt actief? Een tweak die op een herstart van de game wacht, telt apart (grijs, niet groen).
    let of_info = |info: &TweakInfo| match (info.state, info.active.as_deref()) {
        (TweakState::On, Some("after_game_restart")) => Row::AfterGame,
        (TweakState::On, _) => Row::On,
        _ => Row::Off,
    };
    let mut list: Vec<(String, Option<String>, Row)> = vec![];
    for id in BIG_WINS {
        if let Some(w) = bigwins::check_one(state, id) {
            match (w.status, w.action) {
                (CheckStatus::Ok, _) => list.push((id.to_string(), None, Row::On)),
                (CheckStatus::Issue, CheckAction::Fix) => list.push((id.to_string(), None, Row::Off)),
                _ => {}
            }
        }
    }
    for id in tweaks::ACTIVATE_ALL {
        let Ok(d) = tweaks::get(id) else { continue };
        if tweaks::platform_locked(state, d) {
            continue;
        }
        let info = tweaks::info(state, d, None);
        if info.state != TweakState::Unavailable {
            list.push((id.to_string(), None, of_info(&info)));
        }
    }
    if let (true, Ok(d)) = (bigwins::gpu_layout().hybrid, tweaks::get("gpu_high_perf")) {
        for g in state.all_games().iter().filter(|g| g.exe_path.is_some()) {
            list.push(("gpu_high_perf".into(), Some(g.id.clone()), of_info(&tweaks::info(state, d, Some(g)))));
        }
    }
    if fortnite::ini_path().is_file() {
        if let Ok(ov) = fortnite::overview(state) {
            for c in ov.checks.iter().filter(|c| FORTNITE_IDS.contains(&c.id.as_str())) {
                list.push((format!("fortnite_{}", c.id), Some(FORTNITE_GAME.into()), on_off(c.ok)));
            }
        }
    }
    for gid in gameconfig::game_ids() {
        let checks = gameconfig::list(gid);
        if !checks.is_empty() {
            list.push(("game_vsync".into(), Some(gid.to_string()), on_off(checks.iter().all(|c| c.ok))));
        }
    }
    let targets = |want: Row| -> Vec<ApplyTarget> {
        list.iter().filter(|(_, _, r)| *r == want).map(|(id, game_id, _)| ApplyTarget { id: id.clone(), game_id: game_id.clone() }).collect()
    };
    let (missing, present, pending_game) = (targets(Row::Off), targets(Row::On), targets(Row::AfterGame));
    ApplyAllStatus {
        all_active: missing.is_empty() && pending_game.is_empty(),
        enabled: state.settings_clone().all_active,
        active: present.len() as u32,
        total: list.len() as u32,
        missing,
        present,
        pending_game,
    }
}
