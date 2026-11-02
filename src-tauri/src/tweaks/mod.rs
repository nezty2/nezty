//! De tweak-catalogus en de dispatch naar de implementaties. Elke tweak is omkeerbaar via het logboek.
//! Namen en uitleg staan in de frontend (nl, en, fr); hier staan alleen id's en eigenschappen.

pub mod cleanup;
pub mod input;
pub mod network_extra;
pub mod per_game;
pub mod input_extra;
pub mod laptop;
pub mod wifi_gaming;
pub mod regtweak;
pub mod system;

use crate::error::{AppError, AppResult};
use crate::models::*;
use crate::state::{lock, AppState};

pub struct TweakDef {
    pub id: &'static str,
    /// Engelse naam, alleen voor het logboek als terugval.
    pub name: &'static str,
    pub category: TweakCategory,
    pub scope: TweakScope,
    pub kind: TweakKind,
    pub risk: Risk,
    pub gain: Gain,
    pub timing: Timing,
    pub experimental: bool,
    pub requires_admin: bool,
    pub options: &'static [&'static str],
    pub platform: Platform,
}

const fn def(
    id: &'static str,
    name: &'static str,
    category: TweakCategory,
    scope: TweakScope,
    gain: Gain,
    timing: Timing,
    requires_admin: bool,
) -> TweakDef {
    TweakDef {
        id,
        name,
        category,
        scope,
        kind: TweakKind::Toggle,
        risk: Risk::Low,
        gain,
        timing,
        experimental: false,
        requires_admin,
        options: &[],
        platform: Platform::All,
    }
}

use Gain::*;
use Timing::*;
use TweakCategory as C;
use TweakScope as S;

pub const CATALOG: &[TweakDef] = &[
    // Systeem
    def("game_mode", "Windows Game Mode", C::System, S::Global, Small, Instant, false),
    def("game_bar_dvr_off", "Xbox Game Bar and Game DVR off", C::System, S::Global, Small, GameRestart, false),
    TweakDef { options: &["high", "ultimate"], ..def("power_plan", "High performance power plan", C::System, S::Global, Small, Instant, false) },
    TweakDef { risk: Risk::Medium, ..def("hags", "Hardware-accelerated GPU scheduling", C::System, S::Global, Small, PcRestart, true) },
    def("windowed_opt", "Optimizations for windowed games", C::System, S::Global, Small, GameRestart, false),
    // Terug sinds 0.6.1: 0,5 ms timer zolang er een game draait (zoals in 0.3.16).
    def("timer_resolution", "Higher timer resolution while gaming", C::System, S::Global, Measurable, PcRestart, true),
    def("notifications_off", "Notification banners off", C::System, S::Global, Negligible, Instant, false),
    // Muis en toetsenbord
    def("mouse_precision_off", "Enhance pointer precision off", C::Input, S::Global, Small, Instant, false),
    def("accessibility_hotkeys_off", "Sticky/Filter/Toggle keys shortcuts off", C::Input, S::Global, Small, Instant, false),
    TweakDef { platform: Platform::PcOnly, ..def("usb_suspend_off", "USB selective suspend off", C::Input, S::Global, Small, Instant, false) },
    def("lang_hotkey_off", "Language switch shortcuts off", C::Input, S::Global, Small, Instant, false),
    // Netwerk
    def("wifi_power_max", "Wi-Fi power saving off", C::Network, S::Global, Small, Instant, false),
    def("wifi_gaming_mode", "Wi-Fi background scan off while nezty runs", C::Network, S::Global, Measurable, Instant, false),
    // Windows past QoS-regels op een pc zonder domein pas toe met deze instelling (nodig voor game_qos).
    def("qos_all_networks", "QoS policies on every network", C::Network, S::Global, Negligible, PcRestart, true),
    // Alleen laptop
    TweakDef { platform: Platform::LaptopOnly, ..def("power_mode_best", "Power mode best performance", C::Laptop, S::Global, Measurable, Instant, true) },
    TweakDef { platform: Platform::LaptopOnly, ..def("battery_saver_off", "Battery saver off while gaming", C::Laptop, S::Global, Measurable, Instant, false) },
    TweakDef { platform: Platform::LaptopOnly, ..def("cpu_full_speed", "Processor at full speed", C::Laptop, S::Global, Measurable, Instant, false) },
    TweakDef { platform: Platform::LaptopOnly, ..def("cpu_epp_performance", "Processor reacts at once (EPP)", C::Laptop, S::Global, Measurable, Instant, false) },
    TweakDef { platform: Platform::LaptopOnly, ..def("cpu_prefer_pcores", "Games on the performance cores", C::Laptop, S::Global, Measurable, Instant, false) },
    TweakDef { platform: Platform::LaptopOnly, ..def("usb_suspend_ac_off", "USB selective suspend off on the charger", C::Laptop, S::Global, Small, Instant, false) },
    TweakDef { platform: Platform::LaptopOnly, ..def("touchpad_off_with_mouse", "Touchpad off while a mouse is connected", C::Laptop, S::Global, Small, Instant, false) },
    // Per game
    def("gpu_high_perf", "GPU preference high performance", C::Game, S::PerGame, Small, GameRestart, false),
    def("game_qos", "Game traffic priority (DSCP 46)", C::Network, S::PerGame, Small, GameRestart, true),
    // Opruimen
    TweakDef { kind: TweakKind::Action, ..def("cleanup_temp", "Clean temp files and shader cache", C::Cleanup, S::Global, Negligible, Instant, false) },
];

/// Tweaks die in 0.5.0 uit de app zijn gehaald (geen meetbaar effect, of meer risico dan winst).
/// Oude wijzigingen ervan blijven via het logboek exact terug te zetten. timer_resolution is in 0.6.1 teruggekomen.
pub const REMOVED: &[&str] = &[
    "visual_effects",
    "keyboard_fast_repeat",
    "input_device_power_off",
    "delivery_optimization_off",
    "adapter_power_saving_off",
    "interrupt_moderation_off",
    "nagle_off",
    "network_throttling_off",
    "pcie_aspm_off",
    "fullscreen_opt_off",
    "high_priority",
];

/// De algemene tweaks van "Alles activeren", in deze volgorde. Energieplan en energiemodus als eerste: alles wat
/// in een energieplan staat (USB, processor, wifi) komt dan in het nieuwe plan terecht, niet in het oude. Wat niet
/// bij het apparaat past, slaat nezty over. Meldingen uit zit er bewust niet in: dat blijft een losse keuze onder
/// Meer opties.
pub const ACTIVATE_ALL: &[&str] = &[
    "power_plan",
    "power_mode_best",
    "game_mode",
    "game_bar_dvr_off",
    "windowed_opt",
    "timer_resolution",
    "mouse_precision_off",
    "accessibility_hotkeys_off",
    "usb_suspend_off",
    "lang_hotkey_off",
    "cpu_full_speed",
    "wifi_power_max",
    "wifi_gaming_mode",
];

pub fn get(id: &str) -> AppResult<&'static TweakDef> {
    CATALOG.iter().find(|t| t.id == id).ok_or_else(|| AppError::NotFound(format!("tweak {id}")))
}

/// Live status van een tweak.
pub struct Status {
    pub state: TweakState,
    pub detail: Option<Msg>,
    pub current_option: Option<String>,
    /// Werkt het nu echt, volgens Windows zelf? None: niet uit te lezen.
    pub live: Option<bool>,
}

impl Status {
    pub fn simple(state: TweakState) -> Self {
        Status { state, detail: None, current_option: None, live: None }
    }
    pub fn with_detail(state: TweakState, detail: Msg) -> Self {
        Status { state, detail: Some(detail), current_option: None, live: None }
    }
}

pub fn status(state: &AppState, id: &str, game: Option<&Game>) -> Status {
    let r = match id {
        "game_mode" => system::game_mode_status(),
        "game_bar_dvr_off" => system::game_bar_status(),
        "power_plan" => system::power_plan_status(state),
        "hags" => system::hags_status(),
        "windowed_opt" => system::windowed_opt_status(),
        "timer_resolution" => system::timer_status(),
        "notifications_off" => system::notifications_status(state),
        "wifi_power_max" => network_extra::wifi_status(network_extra::has_wifi()),
        "mouse_precision_off" => input::mouse_status(),
        "accessibility_hotkeys_off" => input::accessibility_status(),
        "usb_suspend_off" => input::usb_suspend_status(),
        "lang_hotkey_off" => input_extra::lang_hotkey_status(),
        "power_mode_best" => laptop::power_mode_status(),
        "battery_saver_off" => laptop::battery_saver_status(),
        "cpu_full_speed" => laptop::cpu_full_speed_status(),
        "cpu_epp_performance" => laptop::cpu_epp_status(),
        "cpu_prefer_pcores" => laptop::cpu_pcores_status(),
        "usb_suspend_ac_off" => laptop::usb_ac_status(),
        "touchpad_off_with_mouse" => laptop::touchpad_status(),
        "wifi_gaming_mode" => wifi_gaming::status(),
        "gpu_high_perf" => per_game::gpu_pref_status(game),
        "qos_all_networks" => crate::gamenet::qos::nla_status(),
        "game_qos" => crate::gamenet::qos::game_status(state, game),
        _ => Ok(Status::simple(TweakState::Unknown)),
    };
    match r {
        Ok(s) => s,
        Err(e) => Status::with_detail(TweakState::Unknown, Msg::new("status_error").with("detail", e.to_string())),
    }
}

/// Winst kan afhangen van de hardware: de GPU-voorkeur levert alleen veel op met twee videokaarten.
fn effective_gain(d: &TweakDef) -> Gain {
    if d.id == "gpu_high_perf" {
        return if crate::bigwins::gpu_layout().hybrid { Gain::Big } else { Gain::Negligible };
    }
    d.gain
}

/// Is een aangezette tweak ook echt al actief? Een tweak die een herstart nodig heeft, is pas actief als de pc
/// (of de game) na het aanzetten opnieuw gestart is. Voor de pc zie restart.rs, voor de game de starttijd ervan.
fn active_state(state: &AppState, d: &TweakDef, game: Option<&Game>, applied_ts: Option<i64>, st: &Status) -> &'static str {
    let Some(ts) = applied_ts else { return "now" };
    let applied_secs = (ts / 1000) as u64;
    match d.timing {
        Timing::Instant => "now",
        Timing::PcRestart => {
            // Zegt Windows zelf dat het al werkt, dan is er geen herstart meer nodig.
            if st.live == Some(true) || !crate::restart::pending(state, d.id) {
                "now"
            } else {
                "after_pc_restart"
            }
        }
        Timing::GameRestart => {
            // Draait er een game (deze game, of bij een algemene tweak welke dan ook) die al liep toen de tweak aanging?
            let games: Vec<Game> = match game {
                Some(g) => vec![g.clone()],
                None => {
                    let running = lock(&state.running_games).clone();
                    state.all_games().into_iter().filter(|g| running.contains(&g.id)).collect()
                }
            };
            let pids: Vec<u32> = games.iter().flat_map(|g| per_game::running_pids(state, g)).collect();
            let sys = lock(&state.procs);
            let stale = pids.iter().any(|pid| sys.process(sysinfo::Pid::from_u32(*pid)).is_some_and(|p| p.start_time() < applied_secs));
            if stale {
                "after_game_restart"
            } else {
                "now"
            }
        }
    }
}

/// Past deze tweak niet bij het apparaat (pc of laptop)?
pub fn platform_locked(state: &AppState, d: &TweakDef) -> bool {
    match d.platform {
        Platform::All => false,
        Platform::PcOnly => crate::device::is_laptop(state),
        Platform::LaptopOnly => !crate::device::is_laptop(state),
    }
}

/// Tweaks die Game Boost op een laptop standaard meeneemt als de gebruiker er niets voor gekozen heeft.
/// Allemaal tijdelijk: na het gamen zet de boost alles terug, zodat de laptop daarna weer koel en zuinig is.
pub const LAPTOP_BOOST_DEFAULTS: &[&str] = &[
    "power_mode_best",
    "battery_saver_off",
    "cpu_full_speed",
    "cpu_epp_performance",
    "cpu_prefer_pcores",
    "usb_suspend_ac_off",
    "wifi_gaming_mode",
];

pub fn info(state: &AppState, d: &TweakDef, game: Option<&Game>) -> TweakInfo {
    let st = status(state, d.id, game);
    let game_id = game.map(|g| g.id.as_str());
    let applied_ts = lock(&state.journal).latest_active_ts(d.id, game_id);
    let applied_by_app = applied_ts.is_some();
    let active = (st.state == TweakState::On).then(|| active_state(state, d, game, applied_ts, &st).to_string());
    TweakInfo {
        id: d.id.to_string(),
        category: d.category,
        scope: d.scope,
        kind: d.kind,
        risk: d.risk,
        gain: effective_gain(d),
        timing: d.timing,
        experimental: d.experimental,
        requires_admin: d.requires_admin,
        state: st.state,
        applied_by_app,
        active,
        platform: d.platform,
        platform_locked: platform_locked(state, d),
        // Het Ultimate-plan hoort niet op een laptop: die optie verdwijnt daar (tenzij hij nu aan staat).
        options: if d.options.is_empty() {
            None
        } else {
            let laptop = crate::device::is_laptop(state);
            let cur = st.current_option.clone();
            Some(d.options.iter().filter(|o| !(laptop && **o == "ultimate" && cur.as_deref() != Some("ultimate"))).map(|s| s.to_string()).collect())
        },
        current_option: st.current_option,
        detail: st.detail,
    }
}

pub fn list(state: &AppState) -> Vec<TweakInfo> {
    CATALOG
        .iter()
        .map(|d| {
            if d.scope == TweakScope::PerGame {
                let mut i = info(state, d, None);
                i.state = TweakState::Unknown;
                i.detail = Some(Msg::new("per_game_hint"));
                i
            } else if d.kind == TweakKind::Action {
                let mut i = info(state, d, None);
                i.state = TweakState::Unknown;
                i
            } else {
                info(state, d, None)
            }
        })
        .collect()
}

fn admin_check(state: &AppState, d: &TweakDef) -> AppResult<()> {
    if d.requires_admin && !state.elevated {
        return Err(AppError::NeedsAdmin(d.name.to_string()));
    }
    Ok(())
}

/// Past een tweak toe. `session` koppelt de wijziging aan een boost-sessie.
pub fn apply(state: &AppState, id: &str, option: Option<&str>, session: Option<&str>, game: Option<&Game>) -> AppResult<()> {
    let d = get(id)?;
    admin_check(state, d)?;
    if platform_locked(state, d) || (id == "power_plan" && option == Some("ultimate") && crate::device::is_laptop(state)) {
        return Err(AppError::coded("platform_locked"));
    }
    let journal_len = lock(&state.journal).entries.len();
    let result = apply_inner(state, id, option, session, game);
    // Werkt pas na een herstart: onthouden met een vluchtige sleutel, zodat nezty de herstart zeker herkent.
    if d.timing == Timing::PcRestart {
        crate::restart::mark_new_entries(state, id, journal_len);
    }
    result
}

fn apply_inner(state: &AppState, id: &str, option: Option<&str>, session: Option<&str>, game: Option<&Game>) -> AppResult<()> {
    match id {
        "game_mode" => system::game_mode_apply(state, session),
        "game_bar_dvr_off" => system::game_bar_apply(state, session),
        "power_plan" => system::power_plan_apply(state, option.unwrap_or("high"), session),
        "hags" => system::hags_apply(state, session),
        "windowed_opt" => system::windowed_opt_apply(state, session),
        "timer_resolution" => system::timer_apply(state, session),
        "notifications_off" => system::notifications_apply(state, session),
        "wifi_power_max" => network_extra::wifi_apply(state, session),
        "mouse_precision_off" => input::mouse_apply(state, session),
        "accessibility_hotkeys_off" => input::accessibility_apply(state, session),
        "usb_suspend_off" => input::usb_suspend_apply(state, session),
        "lang_hotkey_off" => input_extra::lang_hotkey_apply(state, session),
        "power_mode_best" => laptop::power_mode_apply(state, session),
        "battery_saver_off" => laptop::battery_saver_apply(state, session),
        "cpu_full_speed" => laptop::cpu_full_speed_apply(state, session),
        "cpu_epp_performance" => laptop::cpu_epp_apply(state, session),
        "cpu_prefer_pcores" => laptop::cpu_pcores_apply(state, session),
        "usb_suspend_ac_off" => laptop::usb_ac_apply(state, session),
        "touchpad_off_with_mouse" => laptop::touchpad_apply(state, session),
        "wifi_gaming_mode" => wifi_gaming::apply(state, session),
        "gpu_high_perf" => per_game::gpu_pref_apply(state, game, session),
        "qos_all_networks" => crate::gamenet::qos::nla_apply(state, session),
        "game_qos" => crate::gamenet::qos::game_apply(state, game, session),
        _ => Err(AppError::NotFound(format!("tweak {id}"))),
    }
}

/// Zet een tweak uit: eerst via het logboek (exact terug), anders naar de Windows-standaard.
pub fn disable(state: &AppState, id: &str, game: Option<&Game>) -> AppResult<()> {
    let d = get(id)?;
    admin_check(state, d)?;
    let result = disable_inner(state, id, game);
    if result.is_ok() && d.timing == Timing::PcRestart {
        crate::restart::clear_marker(id);
    }
    result
}

fn disable_inner(state: &AppState, id: &str, game: Option<&Game>) -> AppResult<()> {
    match id {
        "game_mode" => system::game_mode_disable(state),
        "game_bar_dvr_off" => system::game_bar_disable(state),
        "power_plan" => system::power_plan_disable(state),
        "hags" => system::hags_disable(state),
        "windowed_opt" => system::windowed_opt_disable(state),
        "timer_resolution" => system::timer_disable(state),
        "notifications_off" => system::notifications_disable(state),
        "wifi_power_max" => network_extra::wifi_disable(state),
        "mouse_precision_off" => input::mouse_disable(state),
        "accessibility_hotkeys_off" => input::accessibility_disable(state),
        "usb_suspend_off" => input::usb_suspend_disable(state),
        "lang_hotkey_off" => input_extra::lang_hotkey_disable(state),
        "power_mode_best" => laptop::power_mode_disable(state),
        "battery_saver_off" => laptop::battery_saver_disable(state),
        "cpu_full_speed" => laptop::cpu_full_speed_disable(state),
        "cpu_epp_performance" => laptop::cpu_epp_disable(state),
        "cpu_prefer_pcores" => laptop::cpu_pcores_disable(state),
        "usb_suspend_ac_off" => laptop::usb_ac_disable(state),
        "touchpad_off_with_mouse" => laptop::touchpad_disable(state),
        "wifi_gaming_mode" => wifi_gaming::disable(state),
        "gpu_high_perf" => per_game::gpu_pref_disable(state, game),
        "qos_all_networks" => crate::gamenet::qos::nla_disable(state),
        "game_qos" => crate::gamenet::qos::game_disable(state, game),
        _ => Err(AppError::NotFound(format!("tweak {id}"))),
    }
}

/// Controleert na een wijziging of Windows hem echt heeft overgenomen.
pub fn verify(state: &AppState, id: &str, enabled: bool, game: Option<&Game>) -> AppResult<()> {
    let st = status(state, id, game);
    if matches!(st.state, TweakState::Unknown | TweakState::Unavailable) {
        return Ok(());
    }
    let expected = if enabled { TweakState::On } else { TweakState::Off };
    if st.state != expected {
        return Err(AppError::coded("verify_failed"));
    }
    Ok(())
}

/// Zet eerdere wijzigingen van deze tweak terug via het logboek. Geeft `true` als er iets was.
pub fn revert_via_journal(state: &AppState, id: &str, game_id: Option<&str>) -> AppResult<bool> {
    let ids = lock(&state.journal).active_ids(id, game_id);
    if ids.is_empty() {
        return Ok(false);
    }
    let result = lock(&state.journal).revert_ids(&ids);
    if let Some(f) = result.failed.first() {
        return Err(AppError::coded("revert_failed").with("target", &f.target).with("detail", &f.error));
    }
    Ok(true)
}

/// Eenmalig bij de eerste start van 0.5.0: wijzigingen van weggehaalde tweaks die nog aan staan, exact
/// terugzetten via het logboek. Zo blijft er niets veranderd dat de app niet meer laat zien.
/// Geeft het aantal teruggezette wijzigingen, of None als dit al eerder gedaan is.
pub fn migrate_removed(state: &AppState) -> Option<RevertResult> {
    if state.settings_clone().migrated_v050 {
        return None;
    }
    let (ids, priority): (Vec<String>, Vec<String>) = {
        let j = lock(&state.journal);
        let active: Vec<&crate::journal::JournalEntry> = j.entries.iter().filter(|e| REMOVED.contains(&e.tweak_id.as_str()) && !e.reverted && e.revertable).collect();
        let (prio, rest): (Vec<_>, Vec<_>) = active.into_iter().partition(|e| matches!(e.payload, crate::journal::RevertPayload::Priority { .. }));
        (rest.iter().map(|e| e.id.clone()).collect(), prio.iter().map(|e| e.id.clone()).collect())
    };
    // Procesprioriteit geldt alleen zolang dat proces draait. Terugzetten op pid kan nu een ander proces raken.
    lock(&state.journal).mark_reverted(&priority);
    let mut result = lock(&state.journal).revert_ids(&ids);
    result.reverted += priority.len() as u32;
    // Mislukt er iets zonder beheerdersrechten, dan de volgende keer opnieuw proberen.
    if result.failed.is_empty() || state.elevated {
        lock(&state.settings).migrated_v050 = true;
        state.save_settings();
    }
    if !result.failed.is_empty() {
        log::warn!("opruiming 0.5.0: {} wijziging(en) niet teruggezet, ze blijven in het logboek", result.failed.len());
    }
    Some(result)
}
