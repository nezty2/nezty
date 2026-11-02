//! nezty backend: Tauri-commands, opstart en achtergrondthreads.

mod activate;
mod autostart;
mod bigwins;
mod background;
mod bandwidth;
mod boost;
mod bufferbloat;
mod controller;
mod covers;
mod device;
mod crosshair;
mod display;
mod error;
mod fortnite;
mod fps;
mod gameconfig;
mod gamenet;
mod games;
mod hwaccel;
mod ini;
mod jsonfile;
mod license;
mod icons;
mod journal;
// Frametijden analyseren: wordt nu alleen door de zelftest gebruikt.
#[cfg(feature = "selftest")]
mod measure;
mod metrics;
mod models;
mod netinfo;
mod overlays;
mod ping;
mod powershell;
mod processes;
mod registry;
mod restart;
mod restore;
#[cfg(feature = "selftest")]
mod selftest;
mod startup;
mod state;
mod timer;
mod tray;
mod tweaks;
mod updater;
mod watcher;
mod winapi;

use error::{AppError, AppResult};
use journal::JournalEntry;
use models::*;
use state::{lock, AppState};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

type St<'a> = State<'a, Arc<AppState>>;

/// Voert blokkerend werk uit buiten de UI-thread.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> AppResult<T> + Send + 'static) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::msg(format!("background task failed: {e}")))?
}

fn find_game(st: &AppState, id: &str) -> AppResult<Game> {
    st.all_games().into_iter().find(|g| g.id == id).ok_or_else(|| AppError::NotFound("game".into()))
}

/// Taal van Windows ("nl", "en", "fr" of een andere code).
/// Taal voor teksten in de backend (systeemvak): de gekozen taal, anders die van Windows.
fn ui_lang(settings: &Settings) -> String {
    let lang = settings.language.clone().unwrap_or_else(windows_ui_language);
    if lang == "nl" || lang == "fr" { lang } else { "en".into() }
}

fn windows_ui_language() -> String {
    let id = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() } & 0x3ff;
    match id {
        0x13 => "nl".into(),
        0x0c => "fr".into(),
        0x09 => "en".into(),
        other => format!("{other:x}"),
    }
}

// ---------- App ----------

#[tauri::command]
async fn get_app_info(app: AppHandle, state: St<'_>) -> AppResult<AppInfo> {
    let st = state.inner().clone();
    let version = app.package_info().version.to_string();
    blocking(move || {
        let (cpu_name, gpu_name) = metrics::hardware_names();
        let build: u32 = sysinfo::System::kernel_version().and_then(|k| k.trim().parse().ok()).unwrap_or(0);
        let os_version = format!(
            "{} (build {})",
            sysinfo::System::long_os_version().unwrap_or_else(|| "Windows".into()),
            if build > 0 { build.to_string() } else { "?".into() }
        );
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();
        Ok(AppInfo {
            version,
            is_elevated: st.elevated,
            data_dir: st.data_dir.to_string_lossy().to_string(),
            os_version,
            windows11: build >= 22000,
            cpu_name,
            gpu_name,
            total_memory_mb: sys.total_memory() / 1_048_576,
            ui_language: windows_ui_language(),
        })
    })
    .await
}

#[tauri::command]
async fn get_autostart() -> bool {
    blocking(|| Ok(autostart::is_enabled())).await.unwrap_or(false)
}

#[tauri::command]
async fn set_autostart(enabled: bool) -> AppResult<()> {
    blocking(move || autostart::set(enabled)).await
}

#[tauri::command]
fn get_settings(state: St<'_>) -> Settings {
    state.settings_clone()
}

#[tauri::command]
fn save_settings(app: AppHandle, state: St<'_>, mut settings: Settings) -> AppResult<Settings> {
    let old = state.settings_clone();
    // Deze velden houdt alleen de backend bij: een oudere kopie uit de frontend mag ze niet terugzetten.
    settings.migrated_v050 = old.migrated_v050;
    settings.all_active = old.all_active;
    settings.activated_games = old.activated_games.clone();
    settings.license_no_token_since = old.license_no_token_since;
    *lock(&state.settings) = settings.clone();
    state.save_settings();
    if old.language != settings.language {
        tray::set_language(&app, &ui_lang(&settings));
    }
    if old.crosshair != settings.crosshair {
        crosshair::push_config(&app, &state);
    }
    if old.crosshair_hotkey != settings.crosshair_hotkey {
        if let Err(e) = crosshair::register_hotkey(&app, &settings.crosshair_hotkey) {
            // Oude sneltoets terugzetten en de fout melden.
            lock(&state.settings).crosshair_hotkey = old.crosshair_hotkey.clone();
            state.save_settings();
            let _ = crosshair::register_hotkey(&app, &old.crosshair_hotkey);
            return Err(e);
        }
    }
    if old.steam_grid_db_key != settings.steam_grid_db_key {
        covers::clear_misses(&state.covers_dir());
        covers::fetch_missing(app.clone(), state.inner().clone());
    }
    Ok(state.settings_clone())
}

#[tauri::command]
fn open_path(app: AppHandle, path: String) -> AppResult<()> {
    app.opener().open_path(path, None::<&str>).map_err(|e| AppError::msg(e.to_string()))
}

#[tauri::command]
fn open_url(app: AppHandle, url: String) -> AppResult<()> {
    // Alleen https, Windows-instellingen en de instellingenpagina's van Discord en Steam (zie overlays.rs).
    let allowed = ["https://", "ms-settings:", "discord://-/settings/", "steam://settings/"];
    if !allowed.iter().any(|p| url.starts_with(p)) {
        return Err(AppError::msg("link not allowed"));
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| AppError::msg(e.to_string()))
}

#[tauri::command]
fn get_restore_point_status(state: St<'_>) -> RestorePointStatus {
    let mut s = lock(&state.restore_point).clone();
    s.protection_enabled = restore::protection_enabled();
    s
}

#[tauri::command]
async fn create_restore_point(state: St<'_>) -> AppResult<RestorePointResult> {
    let st = state.inner().clone();
    blocking(move || {
        let r = restore::create(st.elevated);
        if r.ok {
            lock(&st.settings).restore_point_done = true;
            st.save_settings();
        }
        lock(&st.restore_point).last_result = Some(r.clone());
        Ok(r)
    })
    .await
}

// ---------- Metingen ----------

#[tauri::command]
fn get_metrics(state: St<'_>) -> MetricsSnapshot {
    let m = lock(&state.metrics);
    MetricsSnapshot { latest: m.history.back().cloned(), history: m.history.iter().cloned().collect() }
}

// ---------- Tweaks ----------

#[tauri::command]
async fn list_tweaks(state: St<'_>) -> AppResult<Vec<TweakInfo>> {
    let st = state.inner().clone();
    blocking(move || Ok(tweaks::list(&st))).await
}

#[tauri::command]
async fn get_game_tweaks(state: St<'_>, game_id: String) -> AppResult<Vec<TweakInfo>> {
    let st = state.inner().clone();
    blocking(move || {
        let game = find_game(&st, &game_id)?;
        Ok(tweaks::CATALOG.iter().filter(|d| d.scope == TweakScope::PerGame).map(|d| tweaks::info(&st, d, Some(&game))).collect())
    })
    .await
}

/// Vóór de eerste wijziging aan Windows: automatisch een herstelpunt (beleid "Automatisch"),
/// met een melding dat het even duurt en daarna de uitkomst. Geeft de uitkomst als er een gemaakt is.
fn restore_point_first(app: &AppHandle, st: &AppState) -> Option<RestorePointResult> {
    use tauri::Emitter;
    let started = restore::ensure_before_change(st, || {
        let _ = app.emit("notice", Notice::new("info", Msg::new("restore_creating")));
    });
    if let Some(r) = &started {
        let level = if r.ok { "success" } else { "warning" };
        let _ = app.emit("notice", Notice::new(level, Msg::new(&format!("restore_{}", r.key))));
        let _ = app.emit("restore-point", r);
    }
    started
}

#[tauri::command]
async fn set_tweak(app: AppHandle, state: St<'_>, tweak_id: String, enabled: bool, option: Option<String>, game_id: Option<String>) -> AppResult<TweakInfo> {
    if enabled {
        license::require(&state)?;
    }
    let st = state.inner().clone();
    blocking(move || {
        let game = match &game_id {
            Some(id) => Some(find_game(&st, id)?),
            None => None,
        };
        let d = tweaks::get(&tweak_id)?;
        restore_point_first(&app, &st);
        if enabled {
            // Andere optie gekozen (bv. Ultieme prestaties): eerst terug, dan opnieuw toepassen.
            if d.id == "power_plan" && lock(&st.journal).has_active(d.id, None) {
                tweaks::revert_via_journal(&st, d.id, None)?;
            }
            tweaks::apply(&st, &tweak_id, option.as_deref(), None, game.as_ref())?;
        } else {
            tweaks::disable(&st, &tweak_id, game.as_ref())?;
        }
        tweaks::verify(&st, &tweak_id, enabled, game.as_ref())?;
        Ok(tweaks::info(&st, d, game.as_ref()))
    })
    .await
}

/// "Alles activeren": alles wat op dit apparaat echt helpt in één keer, met voortgang ("apply-progress").
#[tauri::command]
async fn apply_all(app: AppHandle, state: St<'_>) -> AppResult<ApplyAllReport> {
    license::require(&state)?;
    let st = state.inner().clone();
    blocking(move || Ok(activate::run(&app, &st))).await
}

/// Staat alles uit "Alles activeren" aan?
#[tauri::command]
async fn apply_all_status(state: St<'_>) -> AppResult<ApplyAllStatus> {
    let st = state.inner().clone();
    blocking(move || Ok(activate::status(&st))).await
}

/// Moet de pc herstarten voordat alles werkt?
#[tauri::command]
async fn get_restart_status(state: St<'_>) -> AppResult<RestartStatus> {
    let st = state.inner().clone();
    blocking(move || Ok(restart::status(&st))).await
}

/// Start Windows opnieuw op (de gebruiker heeft dat zelf gevraagd).
#[tauri::command]
async fn restart_pc() -> AppResult<()> {
    blocking(restart::restart_pc).await
}

#[tauri::command]
async fn preview_cleanup(state: St<'_>) -> AppResult<CleanupPreview> {
    let st = state.inner().clone();
    blocking(move || Ok(tweaks::cleanup::preview(&st))).await
}

#[tauri::command]
async fn run_cleanup(state: St<'_>, item_ids: Vec<String>) -> AppResult<CleanupResult> {
    license::require(&state)?;
    let st = state.inner().clone();
    blocking(move || tweaks::cleanup::run(&st, &item_ids)).await
}

#[tauri::command]
async fn get_big_wins(state: St<'_>) -> AppResult<Vec<BigWin>> {
    let st = state.inner().clone();
    blocking(move || Ok(bigwins::all(&st))).await
}

#[tauri::command]
async fn fix_big_win(app: AppHandle, state: St<'_>, id: String) -> AppResult<Vec<BigWin>> {
    license::require(&state)?;
    let st = state.inner().clone();
    blocking(move || {
        restore_point_first(&app, &st);
        bigwins::fix(&st, &id)?;
        Ok(bigwins::all(&st))
    })
    .await
}

// ---------- Logboek ----------

#[tauri::command]
fn get_journal(state: St<'_>) -> Vec<JournalEntry> {
    let j = lock(&state.journal);
    let mut v = j.entries.clone();
    v.sort_by(|a, b| b.ts.cmp(&a.ts));
    v
}

#[tauri::command]
async fn revert_all(app: AppHandle, state: St<'_>) -> AppResult<RevertResult> {
    let st = state.inner().clone();
    blocking(move || {
        let r = lock(&st.journal).revert_all();
        *lock(&st.boost) = None;
        activate::forget(&st);
        // Alles terug naar standaard: ook de geheugenbewaker en het automatisch wisselen van verbinding uit,
        // anders zouden die later weer iets veranderen.
        {
            let mut s = lock(&st.settings);
            s.mem_guard = false;
            s.multinet_auto = false;
        }
        st.save_settings();
        boost::emit_state(&app, &st);
        Ok(r)
    })
    .await
}

#[tauri::command]
async fn revert_entry(state: St<'_>, entry_id: String) -> AppResult<RevertResult> {
    let st = state.inner().clone();
    blocking(move || Ok(lock(&st.journal).revert_ids(&[entry_id]))).await
}

#[tauri::command]
async fn export_journal(app: AppHandle, state: St<'_>) -> AppResult<String> {
    let st = state.inner().clone();
    let dir = app.path().desktop_dir().unwrap_or_else(|_| st.data_dir.clone());
    blocking(move || {
        let entries = lock(&st.journal).entries.clone();
        let mut out = String::from("nezty log\n=========\n\n");
        for e in entries.iter().rev() {
            let when = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(e.ts)
                .map(|d| d.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_default();
            out.push_str(&format!(
                "[{when}] {} ({}) | {}\n  old: {}\n  new: {}\n  status: {}{}\n\n",
                e.tweak_name,
                e.tweak_id,
                e.target,
                e.old_value.as_deref().unwrap_or("-"),
                e.new_value.as_deref().unwrap_or("-"),
                if e.reverted { "reverted" } else if e.revertable { "active" } else { "not revertable" },
                e.note.as_deref().map(|n| format!(" ({n})")).unwrap_or_default()
            ));
        }
        let path = dir.join(format!("nezty-log-{}.txt", chrono::Local::now().format("%Y%m%d-%H%M%S")));
        std::fs::write(&path, out)?;
        Ok(path.to_string_lossy().to_string())
    })
    .await
}

// ---------- Games ----------

#[tauri::command]
async fn detect_games(app: AppHandle, state: St<'_>) -> AppResult<Vec<Game>> {
    let st = state.inner().clone();
    let games = blocking({
        let st = st.clone();
        move || {
            let found = games::detect_all();
            *lock(&st.games) = found;
            activate::apply_new_games(&st);
            Ok(st.all_games())
        }
    })
    .await?;
    covers::fetch_missing(app, st);
    Ok(games)
}

#[tauri::command]
fn refresh_covers(app: AppHandle, state: St<'_>) {
    let dir = state.covers_dir();
    covers::clear_misses(&dir);
    covers::fetch_missing(app, state.inner().clone());
}

#[tauri::command]
async fn add_custom_game(app: AppHandle, state: St<'_>, exe_path: String, name: Option<String>) -> AppResult<Game> {
    let st = state.inner().clone();
    let g = blocking({
        let st = st.clone();
        move || {
            let g = games::custom_game(&exe_path, name.as_deref())?;
            lock(&st.settings).custom_games.push(g.clone());
            st.save_settings();
            activate::apply_new_games(&st);
            Ok(g)
        }
    })
    .await?;
    covers::fetch_missing(app, st);
    Ok(g)
}

#[tauri::command]
fn remove_custom_game(state: St<'_>, game_id: String) {
    lock(&state.settings).custom_games.retain(|g| g.id != game_id);
    lock(&state.profiles).retain(|p| p.game_id != game_id);
    state.save_settings();
    state.save_profiles();
}

#[tauri::command]
fn get_profiles(state: St<'_>) -> Vec<GameProfile> {
    let mut profiles = lock(&state.profiles).clone();
    let defaults = lock(&state.settings).default_profile_tweaks.clone();
    for g in state.all_games() {
        if !profiles.iter().any(|p| p.game_id == g.id) {
            profiles.push(GameProfile::new(&g.id, &defaults));
        }
    }
    profiles
}

#[tauri::command]
fn save_profile(state: St<'_>, profile: GameProfile) -> GameProfile {
    state.upsert_profile(profile.clone());
    profile
}

#[tauri::command]
async fn launch_game(state: St<'_>, game_id: String) -> AppResult<()> {
    let st = state.inner().clone();
    blocking(move || games::launch(&find_game(&st, &game_id)?)).await
}

#[tauri::command]
fn get_running_games(state: St<'_>) -> Vec<String> {
    lock(&state.running_games).iter().cloned().collect()
}

// ---------- Boost ----------

#[tauri::command]
async fn boost(app: AppHandle, state: St<'_>, game_id: Option<String>, measure_fps: Option<bool>) -> AppResult<BoostReport> {
    let st = state.inner().clone();
    blocking(move || boost::run_boost(&app, &st, game_id.as_deref(), measure_fps.unwrap_or(false))).await
}

#[tauri::command]
async fn unboost(app: AppHandle, state: St<'_>) -> AppResult<RevertResult> {
    let st = state.inner().clone();
    blocking(move || boost::unboost(&app, &st)).await
}

#[tauri::command]
fn get_boost_state(state: St<'_>) -> BoostState {
    boost::current_state(&state)
}

#[tauri::command]
async fn restart_closed_apps(state: St<'_>) -> AppResult<u32> {
    let st = state.inner().clone();
    blocking(move || boost::restart_closed_apps(&st)).await
}

#[tauri::command]
fn get_watcher_status(state: St<'_>) -> WatcherStatus {
    watcher::status(&state)
}

#[tauri::command]
async fn measure_fps(app: AppHandle, state: St<'_>, game_id: String) -> AppResult<FpsResult> {
    license::require(&state)?;
    let st = state.inner().clone();
    blocking(move || {
        let g = find_game(&st, &game_id)?;
        fps::measure(&app, &st, &g, fps::DEFAULT_SECONDS)
    })
    .await
}

// ---------- Processen ----------

#[tauri::command]
async fn list_processes(state: St<'_>) -> AppResult<Vec<ProcessInfo>> {
    let st = state.inner().clone();
    blocking(move || Ok(processes::list(&st))).await
}

#[tauri::command]
async fn kill_process(state: St<'_>, pid: u32) -> AppResult<CloseResult> {
    license::require(&state)?;
    let st = state.inner().clone();
    blocking(move || {
        let closed = processes::kill_pid(&st, pid)?;
        let freed_mb = closed.iter().map(|c| c.memory_mb).sum();
        *lock(&st.last_closed) = closed.clone();
        Ok(CloseResult { closed, failed: vec![], freed_mb })
    })
    .await
}

#[tauri::command]
async fn close_apps(state: St<'_>, names: Vec<String>) -> AppResult<CloseResult> {
    license::require(&state)?;
    let st = state.inner().clone();
    blocking(move || Ok(processes::close_many(&st, &names, None, None))).await
}

#[tauri::command]
async fn get_icons(state: St<'_>, paths: Vec<String>) -> AppResult<HashMap<String, Option<String>>> {
    let st = state.inner().clone();
    blocking(move || {
        let mut out = HashMap::new();
        for p in paths {
            let cached = lock(&st.icons).get(&p).cloned();
            let icon = match cached {
                Some(v) => v,
                None => {
                    let v = icons::exe_icon_data_url(&p, 48);
                    lock(&st.icons).insert(p.clone(), v.clone());
                    v
                }
            };
            out.insert(p, icon);
        }
        Ok(out)
    })
    .await
}

#[tauri::command]
async fn get_store_background() -> AppResult<bool> {
    blocking(processes::store_background_allowed).await
}

#[tauri::command]
async fn set_store_background(app: AppHandle, state: St<'_>, allowed: bool) -> AppResult<()> {
    if !allowed {
        license::require(&state)?;
    }
    let st = state.inner().clone();
    blocking(move || {
        restore_point_first(&app, &st);
        processes::set_store_background(&st, allowed)
    })
    .await
}

// ---------- Opstarten ----------

#[tauri::command]
async fn list_startup_items() -> AppResult<Vec<StartupItem>> {
    blocking(startup::list).await
}

#[tauri::command]
async fn set_startup_item(app: AppHandle, state: St<'_>, item_id: String, enabled: bool) -> AppResult<StartupItem> {
    if !enabled {
        license::require(&state)?;
    }
    let st = state.inner().clone();
    blocking(move || {
        restore_point_first(&app, &st);
        startup::set_enabled(&st, &item_id, enabled)
    })
    .await
}

// ---------- Netwerk ----------

#[tauri::command]
fn get_game_regions() -> Vec<GameRegion> {
    netinfo::game_regions()
}

#[tauri::command]
async fn ping_host(host: String, count: u32) -> AppResult<PingResult> {
    blocking(move || Ok(ping::ping_series(&host, count.clamp(1, 20), 1500, 150))).await
}

// ---------- Game-netwerk ----------

#[tauri::command]
async fn gamenet_regions(state: St<'_>) -> AppResult<Vec<NetRegion>> {
    let st = state.inner().clone();
    blocking(move || Ok(gamenet::geo::regions(&st))).await
}

#[tauri::command]
async fn gamenet_server(state: St<'_>, game_id: String) -> AppResult<GameServer> {
    let st = state.inner().clone();
    blocking(move || {
        let game = find_game(&st, &game_id)?;
        Ok(gamenet::detect_server(&st, &game))
    })
    .await
}

#[tauri::command]
async fn gamenet_route(host: String) -> AppResult<RouteReport> {
    blocking(move || Ok(gamenet::trace::route(&host, 8))).await
}

#[tauri::command]
async fn gamenet_monitor_start(app: AppHandle, host: String) -> AppResult<()> {
    blocking(move || gamenet::monitor::start(app, host)).await
}

#[tauri::command]
fn gamenet_monitor_stop(host: String) {
    gamenet::monitor::stop_host(&host);
}

#[tauri::command]
async fn warp_info(state: St<'_>) -> AppResult<WarpInfo> {
    let st = state.inner().clone();
    blocking(move || Ok(gamenet::warp::info(&st))).await
}

#[tauri::command]
async fn warp_test(state: St<'_>, host: String) -> AppResult<WarpTest> {
    license::require(&state)?;
    let st = state.inner().clone();
    blocking(move || gamenet::warp::test(&st, &host)).await
}

#[tauri::command]
async fn warp_stop(state: St<'_>) -> AppResult<WarpInfo> {
    let st = state.inner().clone();
    blocking(move || {
        gamenet::warp::stop_ours(&st)?;
        Ok(gamenet::warp::info(&st))
    })
    .await
}

#[tauri::command]
async fn multinet_status(state: St<'_>, measure: bool) -> AppResult<MultiNetStatus> {
    let st = state.inner().clone();
    blocking(move || Ok(gamenet::multinet::status(&st, measure))).await
}

#[tauri::command]
async fn multinet_prefer(app: AppHandle, state: St<'_>, luid: String) -> AppResult<MultiNetStatus> {
    license::require(&state)?;
    let luid: u64 = luid.parse().map_err(|_| AppError::coded("connection_gone"))?;
    let st = state.inner().clone();
    blocking(move || {
        restore_point_first(&app, &st);
        gamenet::multinet::set_preferred(&st, luid)?;
        Ok(gamenet::multinet::status(&st, false))
    })
    .await
}

#[tauri::command]
async fn multinet_clear(state: St<'_>) -> AppResult<MultiNetStatus> {
    let st = state.inner().clone();
    blocking(move || {
        gamenet::multinet::clear_preferred(&st)?;
        Ok(gamenet::multinet::status(&st, false))
    })
    .await
}

#[tauri::command]
async fn memguard_status(state: St<'_>) -> AppResult<MemGuardStatus> {
    let st = state.inner().clone();
    blocking(move || Ok(gamenet::memguard::status(&st))).await
}

#[tauri::command]
async fn memguard_purge(state: St<'_>) -> AppResult<PurgeResult> {
    license::require(&state)?;
    blocking(gamenet::memguard::purge).await
}

#[tauri::command]
async fn get_connection_overview() -> AppResult<ConnectionOverview> {
    blocking(netinfo::connection_overview).await
}

#[tauri::command]
async fn flush_dns() -> AppResult<()> {
    blocking(netinfo::flush_dns).await
}

// ---------- Fortnite ----------

#[tauri::command]
async fn get_fortnite_settings() -> AppResult<FortniteSettings> {
    blocking(fortnite::read).await
}

#[tauri::command]
async fn save_fortnite_settings(state: St<'_>, settings: FortniteSettings) -> AppResult<FortniteSettings> {
    license::require(&state)?;
    let st = state.inner().clone();
    blocking(move || fortnite::save(&st, &settings)).await
}

#[tauri::command]
async fn set_background(state: St<'_>, path: Option<String>) -> AppResult<Settings> {
    let st = state.inner().clone();
    blocking(move || background::set(&st, path.as_deref())).await
}

#[tauri::command]
fn license_status(state: St<'_>) -> license::LicenseStatus {
    license::status(&state)
}

#[tauri::command]
async fn license_config(state: St<'_>) -> AppResult<license::PublicConfig> {
    let st = state.inner().clone();
    blocking(move || Ok(license::config(&st))).await
}

#[tauri::command]
async fn license_register(app: AppHandle, state: St<'_>, username: String, password: String, key: String, email: Option<String>, remember: bool) -> AppResult<license::LicenseStatus> {
    let st = state.inner().clone();
    let r = blocking(move || license::register(&st, &username, &password, &key, email.as_deref(), remember)).await?;
    let _ = app.emit("license-state", r.clone());
    Ok(r)
}

#[tauri::command]
async fn license_login(app: AppHandle, state: St<'_>, username: String, password: String, remember: bool) -> AppResult<license::LicenseStatus> {
    let st = state.inner().clone();
    let r = blocking(move || license::login(&st, &username, &password, remember)).await?;
    let _ = app.emit("license-state", r.clone());
    Ok(r)
}

#[tauri::command]
async fn license_demo(app: AppHandle, state: St<'_>) -> AppResult<license::LicenseStatus> {
    let st = state.inner().clone();
    let r = blocking(move || license::demo(&st)).await?;
    let _ = app.emit("license-state", r.clone());
    Ok(r)
}

#[tauri::command]
async fn license_logout(app: AppHandle, state: St<'_>) -> AppResult<license::LicenseStatus> {
    let st = state.inner().clone();
    let app2 = app.clone();
    let r = blocking(move || {
        let r = license::logout(&st);
        license::enforce(&app2, &st);
        Ok(r)
    })
    .await?;
    let _ = app.emit("license-state", r.clone());
    Ok(r)
}

#[tauri::command]
async fn license_refresh(app: AppHandle, state: St<'_>) -> AppResult<license::LicenseStatus> {
    let st = state.inner().clone();
    let r = blocking(move || license::refresh(&st)).await?;
    let _ = app.emit("license-state", r.clone());
    Ok(r)
}

#[tauri::command]
fn get_device_info(state: St<'_>) -> DeviceInfo {
    device::info(&state)
}

#[tauri::command]
fn get_background_file(state: St<'_>) -> Option<String> {
    background::current(&state).map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
async fn measure_bandwidth(state: St<'_>, seconds: Option<f32>) -> AppResult<BandwidthReport> {
    license::require(&state)?;
    let st = state.inner().clone();
    blocking(move || Ok(bandwidth::measure(&st, seconds.unwrap_or(3.0)))).await
}

#[tauri::command]
async fn run_bufferbloat(app: AppHandle, state: St<'_>) -> AppResult<BufferbloatResult> {
    license::require(&state)?;
    blocking(move || bufferbloat::run(&app)).await
}

#[tauri::command]
async fn verify_input_live() -> AppResult<Vec<LiveCheck>> {
    blocking(|| Ok(tweaks::input::verify_live())).await
}

#[tauri::command]
async fn sync_input_live(state: St<'_>) -> AppResult<Vec<LiveCheck>> {
    license::require(&state)?;
    blocking(|| Ok(tweaks::input::sync_live())).await
}

#[tauri::command]
async fn list_game_config(game_id: String) -> AppResult<Vec<GameConfigCheck>> {
    blocking(move || Ok(gameconfig::list(&game_id))).await
}

#[tauri::command]
async fn apply_game_config(app: AppHandle, state: St<'_>, game_id: String) -> AppResult<Vec<GameConfigCheck>> {
    license::require(&state)?;
    let st = state.inner().clone();
    blocking(move || {
        restore_point_first(&app, &st);
        gameconfig::apply(&st, &game_id)
    })
    .await
}

#[tauri::command]
async fn get_fortnite_overview(state: St<'_>) -> AppResult<FortniteOverview> {
    let st = state.inner().clone();
    blocking(move || fortnite::overview(&st)).await
}

#[tauri::command]
async fn apply_fortnite_recommended(app: AppHandle, state: St<'_>, ids: Vec<String>) -> AppResult<FortniteOverview> {
    license::require(&state)?;
    let st = state.inner().clone();
    blocking(move || {
        restore_point_first(&app, &st);
        fortnite::apply_recommended(&st, &ids)
    })
    .await
}

#[tauri::command]
async fn list_overlays(state: St<'_>) -> AppResult<Vec<OverlayInfo>> {
    let st = state.inner().clone();
    blocking(move || Ok(overlays::list(&st))).await
}

#[tauri::command]
async fn close_overlay(state: St<'_>, id: String) -> AppResult<Vec<OverlayInfo>> {
    license::require(&state)?;
    let st = state.inner().clone();
    blocking(move || overlays::close(&st, &id)).await
}

#[tauri::command]
async fn list_hw_accel(state: St<'_>) -> AppResult<Vec<HwAccelInfo>> {
    let st = state.inner().clone();
    blocking(move || Ok(hwaccel::list(&st))).await
}

#[tauri::command]
async fn set_hw_accel(app: AppHandle, state: St<'_>, id: String, enabled: bool) -> AppResult<Vec<HwAccelInfo>> {
    license::require(&state)?;
    let st = state.inner().clone();
    blocking(move || {
        restore_point_first(&app, &st);
        hwaccel::set(&st, &id, enabled)
    })
    .await
}

#[tauri::command]
async fn restore_fortnite_backup(state: St<'_>, backup_path: String) -> AppResult<FortniteSettings> {
    let st = state.inner().clone();
    blocking(move || fortnite::restore_backup(&st, &backup_path)).await
}

// ---------- Crosshair ----------

/// Async: een venster maken vanuit een synchrone opdracht kan op Windows vastlopen.
#[tauri::command]
async fn set_crosshair_enabled(app: AppHandle, state: St<'_>, enabled: bool) -> AppResult<bool> {
    if enabled {
        license::require(&state)?;
    }
    crosshair::set_enabled(&app, &state, enabled)?;
    Ok(enabled)
}

// ---------- Updates ----------

/// Is er een updatebron ingebouwd? Zonder bron zoekt nezty niet naar updates.
#[tauri::command]
fn update_configured(app: AppHandle) -> bool {
    updater::configured(&app)
}

/// Zoekt naar een nieuwere versie (latest.json van de updatebron).
#[tauri::command]
async fn check_update(app: AppHandle) -> AppResult<Option<updater::UpdateInfo>> {
    blocking(move || updater::check(&app)).await
}

/// Downloadt, controleert de handtekening en start de installer in updatemodus. nezty sluit daarna.
#[tauri::command]
async fn install_update(app: AppHandle, state: St<'_>) -> AppResult<()> {
    let data_dir = state.data_dir.clone();
    blocking(move || updater::install(&app, &data_dir)).await
}

// ---------- Controller ----------

#[tauri::command]
async fn list_controllers() -> AppResult<Vec<controller::ControllerInfo>> {
    blocking(|| Ok(controller::list())).await
}

/// Meet een paar seconden hoe vaak de controller nieuwe standen doorgeeft (alleen uitlezen).
#[tauri::command]
async fn measure_controller_rate(state: St<'_>, id: String, seconds: Option<f32>) -> AppResult<controller::ControllerRate> {
    license::require(&state)?;
    blocking(move || Ok(controller::measure(&id, seconds.unwrap_or(4.0)))).await
}

#[tauri::command]
fn get_crosshair_config(state: St<'_>) -> CrosshairConfig {
    lock(&state.settings).crosshair.clone()
}

// ---------- Opstart ----------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// De gegevensmap zonder draaiende Tauri-app: dezelfde als `app.path().app_data_dir()`
/// (%APPDATA%\com.nzty.desktop), of NZTY_DATA_DIR om te testen.
fn standalone_data_dir() -> std::path::PathBuf {
    if let Some(dir) = std::env::var_os("NZTY_DATA_DIR").filter(|d| !d.is_empty()) {
        return std::path::PathBuf::from(dir);
    }
    let appdata = std::env::var_os("APPDATA").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    appdata.join("com.nzty.desktop")
}

/// Andere nezty.exe-processen dan dit (het gewone venster of een tweede stille controle): telt ze en sluit ze
/// als `kill` aan staat.
fn other_instances(kill: bool) -> usize {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
    let own = std::process::id();
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let others: Vec<_> = sys.processes().iter().filter(|(pid, p)| pid.as_u32() != own && p.name().to_string_lossy().eq_ignore_ascii_case("nezty.exe")).map(|(_, p)| p).collect();
    if kill {
        for p in &others {
            p.kill();
        }
    }
    others.len()
}

fn another_instance_running() -> bool {
    other_instances(false) > 0
}

/// Sluit een draaiende nezty, zodat het logboek maar door één proces wordt teruggezet. Geeft het aantal.
fn close_other_instances() -> usize {
    let n = other_instances(true);
    if n > 0 {
        std::thread::sleep(std::time::Duration::from_millis(800));
    }
    n
}

/// Uitkomst van een stille modus in %TEMP%\nezty-<naam>.json, om na te kijken als iets niet klopt.
fn write_standalone_log(name: &str, result: &serde_json::Value) {
    let path = std::env::temp_dir().join(format!("nezty-{name}.json"));
    let _ = std::fs::write(path, serde_json::to_string_pretty(result).unwrap_or_default());
}

/// `nezty.exe --restore-all`: elke gelogde wijziging terug naar hoe het stond, wifi-gamestand uit, geplande taken en
/// autostart weg, token uit Credential Manager. Zet niets terug wat nezty niet zelf heeft veranderd.
fn restore_all_standalone() -> serde_json::Value {
    let closed = close_other_instances();
    let state = AppState::new(standalone_data_dir(), winapi::is_elevated());
    license::init(&state);
    let pending = lock(&state.journal).has_any_active();
    let reverted = license::reset_changes(&state);
    let left = lock(&state.journal).has_any_active();
    autostart::remove_all();
    license::forget(&state);
    log::info!("alles teruggezet voor verwijderen: {} wijzigingen, nog actief in logboek: {}", reverted.reverted, left);
    serde_json::json!({
        "elevated": state.elevated,
        "closedInstances": closed,
        "pendingChanges": pending,
        "reverted": reverted.reverted,
        "failed": reverted.failed,
        "stillActiveInLog": left,
        "tasksLeft": [autostart::is_enabled(), autostart::task_exists(autostart::LICENSE_TASK)],
    })
}

/// `--selftest*`: schrijft de uitkomst als JSON naar het opgegeven pad. Geeft true als zo'n modus gevraagd is.
#[cfg(feature = "selftest")]
fn run_selftest(args: &[String]) -> bool {
    if let Some(i) = args.iter().position(|a| a == "--selftest-license") {
        if let Some(out) = args.get(i + 1) {
            let mut v = license::selftest_cred();
            v["enforce"] = license::selftest_enforce();
            if let Some(key) = args.get(i + 2) {
                v["server"] = license::selftest_server(key);
            }
            let _ = std::fs::write(out, serde_json::to_string_pretty(&v).unwrap_or_default());
        }
        return true;
    }
    if let Some(i) = args.iter().position(|a| a == "--selftest-wifi") {
        if let Some(out) = args.get(i + 1) {
            let before = tweaks::wifi_gaming::adapter_count();
            let started = tweaks::wifi_gaming::start().map_err(|e| e.to_string());
            let active = tweaks::wifi_gaming::is_active();
            std::thread::sleep(std::time::Duration::from_secs(2));
            tweaks::wifi_gaming::stop();
            let v = serde_json::json!({ "adapters": before, "start": started, "activeDuring": active, "activeAfterStop": tweaks::wifi_gaming::is_active(), "hybridCpu": tweaks::laptop::cpu_is_hybrid() });
            let _ = std::fs::write(out, serde_json::to_string_pretty(&v).unwrap_or_default());
        }
        return true;
    }
    if let Some(i) = args.iter().position(|a| a == "--selftest-device") {
        if let Some(out) = args.get(i + 1) {
            let st = AppState::new(std::env::temp_dir().join("nezty-selftest-device"), false);
            let info = device::info(&st);
            let displays: Vec<serde_json::Value> = display::list().iter().map(|d| serde_json::json!({ "monitor": d.monitor, "adapter": d.adapter, "internal": d.internal, "hz": d.current_hz, "max": d.max_hz })).collect();
            let tweaks: Vec<serde_json::Value> = tweaks::list(&st).iter().filter(|t| t.platform != Platform::All).map(|t| serde_json::json!({ "id": t.id, "platform": t.platform, "locked": t.platform_locked, "state": t.state, "detail": t.detail })).collect();
            let v = serde_json::json!({ "device": info, "battery": metrics::battery(), "displays": displays, "tweaks": tweaks, "bigwins": bigwins::all(&st) });
            let _ = std::fs::write(out, serde_json::to_string_pretty(&v).unwrap_or_default());
        }
        return true;
    }
    if let Some(i) = args.iter().position(|a| a == "--selftest-update") {
        // --selftest-update <out.json> <pubkey> <endpoint>...
        if let (Some(out), Some(pubkey)) = (args.get(i + 1), args.get(i + 2)) {
            let v = updater::selftest(&args[i + 3..], pubkey);
            let _ = std::fs::write(out, serde_json::to_string_pretty(&v).unwrap_or_default());
        }
        return true;
    }
    if let Some(i) = args.iter().position(|a| a == "--selftest-gamenet") {
        // --selftest-gamenet <out.json> [host voor de route]
        if let Some(out) = args.get(i + 1) {
            let st = AppState::new(std::env::temp_dir().join("nezty-selftest-gamenet"), false);
            let host = args.get(i + 2).cloned().unwrap_or_else(|| "1.1.1.1".into());
            let regions = gamenet::geo::regions(&st);
            let located: Vec<serde_json::Value> = ["155.133.248.36", "3.120.181.40", "34.90.0.1", "8.8.8.8"]
                .iter()
                .map(|ip| serde_json::json!({ "ip": ip, "location": ip.parse().ok().and_then(|a| gamenet::geo::locate(&st, a)) }))
                .collect();
            let v = serde_json::json!({
                "regions": regions.len(),
                "valveEu": regions.iter().filter(|r| r.provider == "valve" && r.group == "eu").map(|r| format!("{} {}", r.city, r.host)).collect::<Vec<_>>(),
                "located": located,
                "route": gamenet::trace::route(&host, 5),
                "multinet": gamenet::multinet::status(&st, true),
                "memory": gamenet::memguard::stats(),
                "memguard": gamenet::memguard::status(&st),
                "warp": gamenet::warp::info(&st),
                "qosAllNetworks": gamenet::qos::nla_status().map(|s| serde_json::to_value(s.state).unwrap_or_default()).map_err(|e| e.to_string()),
                "etw": gamenet::etw::ensure_running().map_err(|e| e.to_string()),
            });
            gamenet::shutdown();
            let _ = std::fs::write(out, serde_json::to_string_pretty(&v).unwrap_or_default());
        }
        return true;
    }
    if let Some(i) = args.iter().position(|a| a == "--selftest-network") {
        if let Some(out) = args.get(i + 1) {
            selftest::run_network(out);
        }
        return true;
    }
    if let Some(i) = args.iter().position(|a| a == "--selftest-roundtrip") {
        if let Some(out) = args.get(i + 1) {
            selftest::run_roundtrip(out);
        }
        return true;
    }
    if let Some(i) = args.iter().position(|a| a == "--selftest" || a == "--selftest-apply") {
        if let Some(out) = args.get(i + 1) {
            selftest::run(out, args[i] == "--selftest-apply");
        }
        return true;
    }
    false
}

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args: Vec<String> = std::env::args().collect();
    // Zelftests voor ontwikkelaars: alleen in een build met `--features selftest`, nooit in een gewone release.
    #[cfg(feature = "selftest")]
    if run_selftest(&args) {
        return;
    }
    // Stille licentiecontrole bij het inloggen in Windows (geplande taak). Geen venster, geen systeemvak.
    // Draait nezty al, dan doet die de controle zelf en is er niets te doen.
    if args.iter().any(|a| a == "--license-check") {
        if another_instance_running() {
            return;
        }
        let state = AppState::new(standalone_data_dir(), winapi::is_elevated());
        let result = license::check_headless(&state);
        write_standalone_log("license-check", &result);
        return;
    }
    // Alles terugzetten en opruimen, vóór het verwijderen van de app (de uninstaller start dit en wacht erop).
    if args.iter().any(|a| a == "--restore-all") {
        let result = restore_all_standalone();
        write_standalone_log("restore-all", &result);
        return;
    }
    // --minimized komt alleen van starten met Windows (geplande taak).
    let autostart_launch = args.iter().any(|a| a == "--minimized");
    // Draait nezty al, dan geeft deze tweede start zijn voorgrondrecht door aan het bestaande venster.
    winapi::allow_foreground_any();

    let mut builder = tauri::Builder::default();
    builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_main(app)));
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(crosshair::plugin())
        .setup(move |app| {
            // NZTY_DATA_DIR wijst een andere gegevensmap aan: om te testen zonder echte instellingen of logboek te raken.
            let data_dir = match std::env::var_os("NZTY_DATA_DIR").filter(|d| !d.is_empty()) {
                Some(dir) => std::path::PathBuf::from(dir),
                None => app.path().app_data_dir()?,
            };
            let state = Arc::new(AppState::new(data_dir, winapi::is_elevated()));
            // Covers mogen altijd uit de eigen covermap laden, ook als de gegevensmap anders is.
            if let Err(e) = app.asset_protocol_scope().allow_directory(state.covers_dir(), false) {
                log::warn!("covermap niet toegestaan: {e}");
            }
            let _ = std::fs::create_dir_all(state.background_dir());
            if let Err(e) = app.asset_protocol_scope().allow_directory(state.background_dir(), false) {
                log::warn!("achtergrondmap niet toegestaan: {e}");
            }
            app.manage(state.clone());
            // Logboek was (deels) onleesbaar: melden zodra de interface luistert. Er is een kopie bewaard.
            if let Some(backup) = lock(&state.journal).backup.clone() {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    use tauri::Emitter;
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    let file = backup.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                    let _ = handle.emit("notice", Notice::new("warning", Msg::new("journal_backup").with("file", file)));
                });
            }
            lock(&state.restore_point).protection_enabled = restore::protection_enabled();
            {
                let data_dir = state.data_dir.clone();
                std::thread::spawn(move || {
                    autostart::migrate_old();
                    // Eén geplande taak bij het inloggen, van deze versie (met vertraging).
                    autostart::sync_tasks();
                    // Een gedownloade update van de vorige keer is geïnstalleerd: het bestand mag weg.
                    updater::cleanup(&data_dir);
                });
            }

            let settings = state.settings_clone();
            if let Err(e) = tray::create(app.handle(), &ui_lang(&settings)) {
                log::warn!("systeemvak: {e}");
            }
            // Alleen bij starten met Windows (en als dat in Instellingen aan staat) blijft het venster in het
            // systeemvak. Elke andere start toont het venster meteen op de voorgrond.
            if !(autostart_launch && settings.start_minimized) {
                tray::show_main(app.handle());
            }

            if let Err(e) = crosshair::register_hotkey(app.handle(), &settings.crosshair_hotkey) {
                log::warn!("sneltoets crosshair: {e}");
            }
            crosshair::start_keeper(app.handle().clone(), state.clone());
            if settings.crosshair_enabled {
                let _ = crosshair::set_enabled(app.handle(), &state, true);
            }

            license::init(&state);
            // Eerste start van 0.5.0: weggehaalde tweaks die nog aan staan exact terugzetten (eenmalig).
            {
                let st = state.clone();
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    let started = std::time::Instant::now();
                    let Some(r) = tweaks::migrate_removed(&st) else { return };
                    log::info!("opruiming 0.5.0: {} wijziging(en) teruggezet", r.reverted);
                    if r.reverted > 0 {
                        // Melden zodra de interface luistert.
                        if let Some(wait) = std::time::Duration::from_secs(5).checked_sub(started.elapsed()) {
                            std::thread::sleep(wait);
                        }
                        let _ = handle.emit("notice", Notice::new("info", Msg::new("migrated_v050").with("count", r.reverted)));
                    }
                });
            }
            license::start(app.handle().clone(), state.clone());
            metrics::start(app.handle().clone(), state.clone());
            gamenet::memguard::start(app.handle().clone(), state.clone());
            gamenet::multinet::start(app.handle().clone(), state.clone());
            // Een ETW-sessie die na een crash is blijven staan, opruimen.
            if state.elevated {
                gamenet::etw::stop_by_name();
            }
            // Wifi-gamestand hoort bij een open sessie van nezty: na een herstart van de app opnieuw aanzetten.
            {
                let st = state.clone();
                std::thread::spawn(move || tweaks::wifi_gaming::resume_if_active(&st));
            }
            watcher::start(app.handle().clone(), state.clone());
            {
                let st = state.clone();
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    let found = games::detect_all();
                    *lock(&st.games) = found;
                    // GPU-indeling alvast bepalen (gebruikt door tweaks en controles).
                    let _ = bigwins::gpu_layout();
                    activate::apply_new_games(&st);
                    covers::fetch_missing(handle, st);
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Als het hoofdvenster sluit, sluit ook de crosshair-overlay en stopt de app.
            if window.label() == "main" {
                // Minimaliseren is gewoon minimaliseren: het venster blijft in de taakbalk. Verbergen in het
                // systeemvak gebeurt alleen bij starten met Windows.
                if let tauri::WindowEvent::CloseRequested { .. } = event {
                    window.app_handle().exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            get_autostart,
            set_autostart,
            get_settings,
            save_settings,
            open_path,
            open_url,
            get_restore_point_status,
            create_restore_point,
            get_metrics,
            list_tweaks,
            get_game_tweaks,
            set_tweak,
            apply_all,
            apply_all_status,
            get_restart_status,
            restart_pc,
            preview_cleanup,
            run_cleanup,
            get_big_wins,
            fix_big_win,
            get_journal,
            revert_all,
            revert_entry,
            export_journal,
            detect_games,
            refresh_covers,
            add_custom_game,
            remove_custom_game,
            get_profiles,
            save_profile,
            launch_game,
            get_running_games,
            boost,
            unboost,
            get_boost_state,
            restart_closed_apps,
            get_watcher_status,
            measure_fps,
            list_processes,
            kill_process,
            close_apps,
            get_icons,
            get_store_background,
            set_store_background,
            list_startup_items,
            set_startup_item,
            get_game_regions,
            get_device_info,
            license_status,
            license_config,
            license_register,
            license_login,
            license_demo,
            license_logout,
            license_refresh,
            ping_host,
            get_connection_overview,
            flush_dns,
            get_fortnite_settings,
            save_fortnite_settings,
            restore_fortnite_backup,
            get_fortnite_overview,
            verify_input_live,
            measure_bandwidth,
            set_background,
            get_background_file,
            run_bufferbloat,
            sync_input_live,
            list_game_config,
            apply_game_config,
            apply_fortnite_recommended,
            list_overlays,
            close_overlay,
            list_hw_accel,
            set_hw_accel,
            set_crosshair_enabled,
            get_crosshair_config,
            list_controllers,
            update_configured,
            check_update,
            install_update,
            measure_controller_rate,
            gamenet_regions,
            gamenet_server,
            gamenet_route,
            gamenet_monitor_start,
            gamenet_monitor_stop,
            warp_info,
            warp_test,
            warp_stop,
            multinet_status,
            multinet_prefer,
            multinet_clear,
            memguard_status,
            memguard_purge,
        ])
        .build(tauri::generate_context!())
        .expect("nezty could not start")
        .run(|_, event| {
            // Altijd opruimen bij afsluiten, ook via het systeemvak of na een update: anders blijft de ETW-sessie
            // tot de volgende herstart van Windows bestaan.
            if let tauri::RunEvent::Exit = event {
                gamenet::shutdown();
            }
        });
}
