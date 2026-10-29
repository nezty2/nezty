//! De tweak-catalogus en de dispatch naar de implementaties. Elke tweak is omkeerbaar via het logboek.
//! Namen en uitleg staan in de frontend (nl, en, fr); hier staan alleen id's en eigenschappen.

pub mod cleanup;
pub mod input;
pub mod network_extra;
pub mod network;
pub mod per_game;
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
    }
}

use Gain::*;
use Timing::*;
use TweakCategory as C;
use TweakScope as S;

pub const CATALOG: &[TweakDef] = &[
    // Systeem
    def("game_mode", "Windows Game Mode", C::System, S::Global, Small, GameRestart, false),
    def("game_bar_dvr_off", "Xbox Game Bar and Game DVR off", C::System, S::Global, Measurable, GameRestart, false),
    TweakDef { options: &["high", "ultimate"], ..def("power_plan", "High performance power plan", C::System, S::Global, Small, Instant, false) },
    TweakDef { risk: Risk::Medium, ..def("hags", "Hardware-accelerated GPU scheduling", C::System, S::Global, Small, PcRestart, true) },
    def("visual_effects", "Visual effects for best performance", C::System, S::Global, Negligible, Instant, false),
    def("windowed_opt", "Optimizations for windowed games", C::System, S::Global, Measurable, GameRestart, false),
    TweakDef { experimental: true, ..def("timer_resolution", "Higher timer resolution while gaming", C::System, S::Global, Negligible, PcRestart, true) },
    def("notifications_off", "Notification banners off", C::System, S::Global, Negligible, PcRestart, false),
    // Muis en toetsenbord
    def("mouse_precision_off", "Enhance pointer precision off", C::Input, S::Global, Measurable, Instant, false),
    def("keyboard_fast_repeat", "Fastest keyboard repeat", C::Input, S::Global, Small, Instant, false),
    def("accessibility_hotkeys_off", "Sticky/Filter/Toggle keys shortcuts off", C::Input, S::Global, Small, Instant, false),
    def("usb_suspend_off", "USB selective suspend off", C::Input, S::Global, Small, Instant, false),
    // Netwerk
    def("adapter_power_saving_off", "Network adapter power saving off", C::Network, S::Global, Small, Instant, true),
    TweakDef { experimental: true, ..def("nagle_off", "Nagle's algorithm off", C::Network, S::Global, Negligible, PcRestart, true) },
    def("wifi_power_max", "Wi-Fi power saving off", C::Network, S::Global, Small, Instant, false),
    def("delivery_optimization_off", "Delivery Optimization uploads off", C::Network, S::Global, Small, Instant, true),
    TweakDef { experimental: true, ..def("interrupt_moderation_off", "Interrupt moderation off", C::Network, S::Global, Negligible, Instant, true) },
    TweakDef { experimental: true, ..def("network_throttling_off", "Network throttling off", C::Network, S::Global, Negligible, PcRestart, true) },
    // Per game
    def("fullscreen_opt_off", "Fullscreen optimizations off", C::Game, S::PerGame, Small, GameRestart, false),
    def("gpu_high_perf", "GPU preference high performance", C::Game, S::PerGame, Small, GameRestart, false),
    def("high_priority", "High process priority", C::Game, S::PerGame, Small, Instant, false),
    // Opruimen
    TweakDef { kind: TweakKind::Action, ..def("cleanup_temp", "Clean temp files and shader cache", C::Cleanup, S::Global, Negligible, Instant, false) },
];

pub fn get(id: &str) -> AppResult<&'static TweakDef> {
    CATALOG.iter().find(|t| t.id == id).ok_or_else(|| AppError::NotFound(format!("tweak {id}")))
}

/// Live status van een tweak.
pub struct Status {
    pub state: TweakState,
    pub detail: Option<Msg>,
    pub current_option: Option<String>,
}

impl Status {
    pub fn simple(state: TweakState) -> Self {
        Status { state, detail: None, current_option: None }
    }
    pub fn with_detail(state: TweakState, detail: Msg) -> Self {
        Status { state, detail: Some(detail), current_option: None }
    }
}

pub fn status(state: &AppState, id: &str, game: Option<&Game>) -> Status {
    let r = match id {
        "game_mode" => system::game_mode_status(),
        "game_bar_dvr_off" => system::game_bar_status(),
        "power_plan" => system::power_plan_status(),
        "hags" => system::hags_status(),
        "visual_effects" => system::visual_effects_status(),
        "windowed_opt" => system::windowed_opt_status(),
        "notifications_off" => system::notifications_status(state),
        "timer_resolution" => system::timer_status(),
        "interrupt_moderation_off" => network::interrupt_status(state.elevated),
        "wifi_power_max" => network_extra::wifi_status(network_extra::has_wifi()),
        "delivery_optimization_off" => network_extra::delivery_status(),
        "mouse_precision_off" => input::mouse_status(),
        "keyboard_fast_repeat" => input::keyboard_status(),
        "accessibility_hotkeys_off" => input::accessibility_status(),
        "usb_suspend_off" => input::usb_suspend_status(),
        "adapter_power_saving_off" => network::adapter_power_status(state.elevated),
        "nagle_off" => network::nagle_status(),
        "network_throttling_off" => network::throttling_status(),
        "fullscreen_opt_off" => per_game::fullscreen_status(game),
        "gpu_high_perf" => per_game::gpu_pref_status(game),
        "high_priority" => per_game::priority_status(state, game),
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
/// (of de game) na het aanzetten opnieuw gestart is. Dat lezen we af aan de opstarttijd van Windows en van de game.
fn active_state(state: &AppState, d: &TweakDef, game: Option<&Game>, applied_ts: Option<i64>) -> &'static str {
    let Some(ts) = applied_ts else { return "now" };
    let applied_secs = (ts / 1000) as u64;
    match d.timing {
        Timing::Instant => "now",
        Timing::PcRestart => {
            if sysinfo::System::boot_time() > applied_secs {
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

pub fn info(state: &AppState, d: &TweakDef, game: Option<&Game>) -> TweakInfo {
    let st = status(state, d.id, game);
    let game_id = game.map(|g| g.id.as_str());
    let applied_ts = lock(&state.journal).latest_active_ts(d.id, game_id);
    let applied_by_app = applied_ts.is_some();
    let active = (st.state == TweakState::On).then(|| active_state(state, d, game, applied_ts).to_string());
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
        options: if d.options.is_empty() { None } else { Some(d.options.iter().map(|s| s.to_string()).collect()) },
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
    match id {
        "game_mode" => system::game_mode_apply(state, session),
        "game_bar_dvr_off" => system::game_bar_apply(state, session),
        "power_plan" => system::power_plan_apply(state, option.unwrap_or("high"), session),
        "hags" => system::hags_apply(state, session),
        "visual_effects" => system::visual_effects_apply(state, session),
        "windowed_opt" => system::windowed_opt_apply(state, session),
        "notifications_off" => system::notifications_apply(state, session),
        "timer_resolution" => system::timer_apply(state, session),
        "interrupt_moderation_off" => network::interrupt_apply(state, session),
        "wifi_power_max" => network_extra::wifi_apply(state, session),
        "delivery_optimization_off" => network_extra::delivery_apply(state, session),
        "mouse_precision_off" => input::mouse_apply(state, session),
        "keyboard_fast_repeat" => input::keyboard_apply(state, session),
        "accessibility_hotkeys_off" => input::accessibility_apply(state, session),
        "usb_suspend_off" => input::usb_suspend_apply(state, session),
        "adapter_power_saving_off" => network::adapter_power_apply(state, session),
        "nagle_off" => network::nagle_apply(state, session),
        "network_throttling_off" => network::throttling_apply(state, session),
        "fullscreen_opt_off" => per_game::fullscreen_apply(state, game, session),
        "gpu_high_perf" => per_game::gpu_pref_apply(state, game, session),
        "high_priority" => per_game::priority_apply(state, game, session).map(|_| ()),
        _ => Err(AppError::NotFound(format!("tweak {id}"))),
    }
}

/// Zet een tweak uit: eerst via het logboek (exact terug), anders naar de Windows-standaard.
pub fn disable(state: &AppState, id: &str, game: Option<&Game>) -> AppResult<()> {
    let d = get(id)?;
    admin_check(state, d)?;
    match id {
        "game_mode" => system::game_mode_disable(state),
        "game_bar_dvr_off" => system::game_bar_disable(state),
        "power_plan" => system::power_plan_disable(state),
        "hags" => system::hags_disable(state),
        "visual_effects" => system::visual_effects_disable(state),
        "windowed_opt" => system::windowed_opt_disable(state),
        "notifications_off" => system::notifications_disable(state),
        "timer_resolution" => system::timer_disable(state),
        "interrupt_moderation_off" => network::interrupt_disable(state),
        "wifi_power_max" => network_extra::wifi_disable(state),
        "delivery_optimization_off" => network_extra::delivery_disable(state),
        "mouse_precision_off" => input::mouse_disable(state),
        "keyboard_fast_repeat" => input::keyboard_disable(state),
        "accessibility_hotkeys_off" => input::accessibility_disable(state),
        "usb_suspend_off" => input::usb_suspend_disable(state),
        "adapter_power_saving_off" => network::adapter_power_disable(state),
        "nagle_off" => network::nagle_disable(state),
        "network_throttling_off" => network::throttling_disable(state),
        "fullscreen_opt_off" => per_game::fullscreen_disable(state, game),
        "gpu_high_perf" => per_game::gpu_pref_disable(state, game),
        "high_priority" => per_game::priority_disable(state, game),
        _ => Err(AppError::NotFound(format!("tweak {id}"))),
    }
}

/// Controleert na een wijziging of Windows hem echt heeft overgenomen.
pub fn verify(state: &AppState, id: &str, enabled: bool, game: Option<&Game>) -> AppResult<()> {
    // Prioriteit werkt pas als de game draait; dat is geen fout.
    if id == "high_priority" {
        return Ok(());
    }
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
