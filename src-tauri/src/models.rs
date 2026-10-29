//! Datamodellen die 1-op-1 overeenkomen met `src/lib/types.ts`. Alle zichtbare tekst zit in de
//! frontend (nl, en, fr). De backend stuurt alleen id's, codes en waarden.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Een vertaalbare melding: sleutel plus invulwaarden.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Msg {
    pub key: String,
    #[serde(default)]
    pub params: BTreeMap<String, String>,
}

impl Msg {
    pub fn new(key: &str) -> Self {
        Msg { key: key.to_string(), params: BTreeMap::new() }
    }
    pub fn with(mut self, name: &str, value: impl ToString) -> Self {
        self.params.insert(name.to_string(), value.to_string());
        self
    }
}

// ---------- Tweaks ----------

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    Low,
    Medium,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Gain {
    Big,
    Measurable,
    Small,
    Negligible,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TweakCategory {
    System,
    Network,
    Input,
    Cleanup,
    Game,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TweakScope {
    Global,
    PerGame,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TweakKind {
    Toggle,
    Action,
}

/// Wanneer een tweak effect heeft.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Timing {
    Instant,
    GameRestart,
    PcRestart,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TweakState {
    On,
    Off,
    Unknown,
    Unavailable,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TweakInfo {
    pub id: String,
    pub category: TweakCategory,
    pub scope: TweakScope,
    pub kind: TweakKind,
    pub risk: Risk,
    pub gain: Gain,
    pub timing: Timing,
    pub experimental: bool,
    pub requires_admin: bool,
    pub state: TweakState,
    pub applied_by_app: bool,
    pub options: Option<Vec<String>>,
    pub current_option: Option<String>,
    pub detail: Option<Msg>,
}

// ---------- App en instellingen ----------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub is_elevated: bool,
    pub data_dir: String,
    pub os_version: String,
    pub windows11: bool,
    pub cpu_name: String,
    pub gpu_name: String,
    pub total_memory_mb: u64,
    pub ui_language: String,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Accent {
    #[default]
    Violet,
    Cyan,
    Emerald,
    Rose,
    Pink,
    Amber,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RestorePointPolicy {
    #[default]
    Auto,
    Never,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct CrosshairConfig {
    /// cross | dot | circle | t | cross_dot
    pub shape: String,
    pub color: String,
    pub size: f32,
    pub thickness: f32,
    pub gap: f32,
    pub outline: bool,
    pub outline_color: String,
    pub outline_thickness: f32,
    pub opacity: f32,
    pub dot: bool,
}

impl Default for CrosshairConfig {
    fn default() -> Self {
        CrosshairConfig {
            shape: "cross".into(),
            color: "#00ff7f".into(),
            size: 8.0,
            thickness: 2.0,
            gap: 4.0,
            outline: true,
            outline_color: "#000000".into(),
            outline_thickness: 1.0,
            opacity: 1.0,
            dot: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CrosshairPreset {
    pub name: String,
    pub config: CrosshairConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// "nl" | "en" | "fr" of null (volgt Windows).
    pub language: Option<String>,
    pub accent: Accent,
    pub reduce_motion: bool,
    pub auto_boost_enabled: bool,
    pub restore_point_policy: RestorePointPolicy,
    pub restore_point_done: bool,
    pub dashboard_ping_host: String,
    pub close_during_gaming: Vec<String>,
    pub custom_ping_targets: Vec<PingTarget>,
    pub custom_games: Vec<Game>,
    pub start_minimized: bool,
    pub first_run_done: bool,
    pub default_profile_tweaks: HashMap<String, bool>,
    pub steam_grid_db_key: String,
    pub crosshair: CrosshairConfig,
    pub crosshair_presets: Vec<CrosshairPreset>,
    pub crosshair_enabled: bool,
    pub crosshair_hotkey: String,
    /// Bij het starten zoeken naar een nieuwe versie (alleen als er een updatebron is ingesteld).
    pub auto_update_check: bool,
    /// Game-afbeeldingen online ophalen (Steam, Epic, SteamGridDB). Uit = alleen wat de launchers lokaal hebben.
    pub online_covers: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let mut default_profile_tweaks = HashMap::new();
        for id in ["game_mode", "game_bar_dvr_off", "power_plan", "mouse_precision_off"] {
            default_profile_tweaks.insert(id.to_string(), true);
        }
        Settings {
            language: None,
            accent: Accent::Violet,
            reduce_motion: false,
            auto_boost_enabled: true,
            restore_point_policy: RestorePointPolicy::Auto,
            restore_point_done: false,
            dashboard_ping_host: "1.1.1.1".to_string(),
            close_during_gaming: vec![],
            custom_ping_targets: vec![],
            custom_games: vec![],
            start_minimized: false,
            first_run_done: false,
            default_profile_tweaks,
            steam_grid_db_key: String::new(),
            crosshair: CrosshairConfig::default(),
            crosshair_presets: vec![],
            crosshair_enabled: false,
            crosshair_hotkey: "ctrl+alt+c".to_string(),
            auto_update_check: true,
            online_covers: true,
        }
    }
}

// ---------- Metingen ----------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MetricsSample {
    pub ts: i64,
    pub cpu_pct: f32,
    pub ram_used_mb: u64,
    pub ram_total_mb: u64,
    pub ram_free_mb: u64,
    pub gpu_pct: Option<f32>,
    pub gpu_temp_c: Option<f32>,
    pub cpu_temp_c: Option<f32>,
    pub gpu_mem_used_mb: Option<u64>,
    pub gpu_mem_total_mb: Option<u64>,
    pub process_count: u32,
    pub ping_ms: Option<f32>,
    pub ping_host: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MetricsSnapshot {
    pub latest: Option<MetricsSample>,
    pub history: Vec<MetricsSample>,
}

// ---------- Games en profielen ----------

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Launcher {
    Epic,
    Steam,
    Battlenet,
    Riot,
    Ea,
    Xbox,
    Minecraft,
    Custom,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: String,
    pub name: String,
    pub launcher: Launcher,
    pub install_path: Option<String>,
    pub exe_path: Option<String>,
    pub process_names: Vec<String>,
    pub launch_uri: Option<String>,
    pub known: bool,
    pub icon_key: Option<String>,
    /// Steam-appid als die bekend is (ook voor games via andere launchers), voor afbeeldingen.
    #[serde(default)]
    pub steam_app_id: Option<u32>,
    /// Pad naar de afbeelding in de cache (als die er is).
    #[serde(default)]
    pub cover: Option<String>,
    /// Eigen pictogram in de cache (bv. het Minecraft-grasblok), voor als er geen afbeelding is.
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct GameProfile {
    pub game_id: String,
    pub auto_boost: bool,
    pub revert_on_exit: bool,
    pub global_tweaks: HashMap<String, bool>,
    pub fullscreen_opt_off: bool,
    pub gpu_high_perf: bool,
    pub high_priority: bool,
    pub close_apps: Vec<String>,
    pub use_global_close_list: bool,
}

impl Default for GameProfile {
    fn default() -> Self {
        GameProfile {
            game_id: String::new(),
            auto_boost: false,
            revert_on_exit: true,
            global_tweaks: HashMap::new(),
            fullscreen_opt_off: true,
            gpu_high_perf: true,
            high_priority: true,
            close_apps: vec![],
            use_global_close_list: true,
        }
    }
}

impl GameProfile {
    pub fn new(game_id: &str, default_tweaks: &HashMap<String, bool>) -> Self {
        GameProfile { game_id: game_id.to_string(), global_tweaks: default_tweaks.clone(), ..Default::default() }
    }
}

// ---------- Boost ----------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BoostSnapshot {
    pub ts: i64,
    pub free_ram_mb: u64,
    pub process_count: u32,
    pub cpu_pct: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppliedTweak {
    pub id: String,
    pub timing: Timing,
    pub option: Option<String>,
    pub count: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SkippedTweak {
    pub id: String,
    pub reason: Msg,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClosedProcess {
    pub name: String,
    pub exe_path: Option<String>,
    #[serde(default)]
    pub memory_mb: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FpsResult {
    pub avg_fps: f32,
    pub low1_fps: f32,
    pub frames: u32,
    pub seconds: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BoostReport {
    pub session_id: String,
    pub game_id: Option<String>,
    pub game_name: Option<String>,
    pub started_at: i64,
    pub before: BoostSnapshot,
    pub after: BoostSnapshot,
    pub applied: Vec<AppliedTweak>,
    pub already_ok: Vec<String>,
    pub skipped: Vec<SkippedTweak>,
    pub closed_processes: Vec<ClosedProcess>,
    pub restore_point: Option<RestorePointResult>,
    pub game_running: bool,
    pub warnings: Vec<Msg>,
    pub fps_before: Option<FpsResult>,
    pub fps_after: Option<FpsResult>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BoostState {
    pub active: bool,
    pub report: Option<BoostReport>,
}

// ---------- Logboek ----------

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JournalKind {
    Registry,
    PowerPlan,
    PowerSetting,
    Display,
    File,
    Process,
    Setting,
    Cleanup,
    Startup,
    StoreApp,
    Adapter,
    Priority,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RevertFailure {
    pub id: String,
    pub target: String,
    pub error: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RevertResult {
    pub reverted: u32,
    pub skipped: u32,
    pub failed: Vec<RevertFailure>,
}

// ---------- Processen ----------

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProcessCategory {
    Browser,
    Launcher,
    Cloud,
    Updater,
    Communication,
    Media,
    Rgb,
    Game,
    System,
    Security,
    Anticheat,
    Other,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    /// Vriendelijke naam uit de versie-informatie van het programma (bv. "Google Chrome").
    pub title: String,
    pub exe_path: Option<String>,
    pub memory_mb: f32,
    pub cpu_pct: f32,
    pub category: ProcessCategory,
    /// Hoort bij "Sluit alles wat niet nodig is".
    pub unneeded: bool,
    pub instances: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CloseResult {
    pub closed: Vec<ClosedProcess>,
    pub failed: Vec<String>,
    pub freed_mb: f32,
}

// ---------- Opstarten ----------

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StartupSource {
    HkcuRun,
    HklmRun,
    HklmRun32,
    UserFolder,
    CommonFolder,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StartupItem {
    pub id: String,
    pub name: String,
    pub command: String,
    pub exe_path: Option<String>,
    pub source: StartupSource,
    pub enabled: bool,
    pub requires_admin: bool,
}

// ---------- Netwerk ----------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PingTarget {
    pub id: String,
    pub label: String,
    pub host: String,
    pub group: String,
    pub note: Option<String>,
    pub custom: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GameRegion {
    /// Id van de regio (bv. "eu-central").
    pub id: String,
    /// Regio zoals games hem noemen: "eu" | "na_east" | "na_central" | "na_west" | "brazil" | "middle_east" | "asia" | "oceania".
    pub region: String,
    /// Stad van het datacenter (eigennaam, niet vertaald).
    pub city: String,
    pub host: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PingResult {
    pub host: String,
    pub ip: Option<String>,
    pub sent: u32,
    pub received: u32,
    pub min_ms: Option<f32>,
    pub avg_ms: Option<f32>,
    pub max_ms: Option<f32>,
    pub jitter_ms: Option<f32>,
    pub loss_pct: f32,
    pub samples: Vec<Option<f32>>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AdapterKind {
    Wifi,
    Ethernet,
    Other,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NetworkAdapter {
    pub name: String,
    pub description: String,
    pub kind: AdapterKind,
    pub status: String,
    pub link_speed_mbps: Option<f64>,
    pub ipv4: Option<String>,
    pub gateway: Option<String>,
    pub dns: Vec<String>,
    pub mac: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionOverview {
    pub adapters: Vec<NetworkAdapter>,
    pub tcp_global: BTreeMap<String, String>,
}

// ---------- Opruimen ----------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CleanupItem {
    /// temp | wintemp | nv-dx | ... (vertaald in de frontend)
    pub id: String,
    pub path: String,
    pub size_bytes: u64,
    pub file_count: u64,
    pub shader_cache: bool,
    pub needs_admin: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPreview {
    pub items: Vec<CleanupItem>,
    pub total_bytes: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResult {
    pub freed_bytes: u64,
    pub deleted_files: u64,
    pub skipped_files: u64,
    pub admin_skipped: Vec<String>,
}

// ---------- Fortnite ----------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FortniteSettings {
    pub ini_path: String,
    pub exists: bool,
    pub read_only: bool,
    pub frame_rate_limit: Option<f32>,
    pub vsync: Option<bool>,
    pub fullscreen_mode: Option<u32>,
    pub res_x: Option<u32>,
    pub res_y: Option<u32>,
    pub backups: Vec<String>,
}

// ---------- Grootste winst ----------

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Ok,
    Issue,
    Info,
    Unavailable,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckAction {
    None,
    Fix,
    OpenPage,
    Link,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BigWin {
    /// refresh_rate | gpu_choice | power_source | ram_speed | gpu_driver | heavy_apps
    pub id: String,
    pub status: CheckStatus,
    pub gain: Gain,
    /// Variant van de tekst (bv. "battery", "saver", "plan" bij power_source).
    pub variant: String,
    pub params: BTreeMap<String, String>,
    pub action: CheckAction,
    pub link: Option<String>,
}

// ---------- Herstelpunt, meldingen, events ----------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RestorePointResult {
    pub ok: bool,
    /// created | recent_exists | needs_admin | protection_off | failed
    pub key: String,
    pub detail: Option<String>,
    pub ts: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct RestorePointStatus {
    pub protection_enabled: Option<bool>,
    pub last_result: Option<RestorePointResult>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub level: String,
    pub key: String,
    pub params: BTreeMap<String, String>,
}

impl Notice {
    pub fn new(level: &str, msg: Msg) -> Self {
        Notice { level: level.into(), key: msg.key, params: msg.params }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GameEvent {
    pub game_id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WatcherStatus {
    pub running: bool,
    pub watching: u32,
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
