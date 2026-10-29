// Gedeeld contract tussen de React-frontend en de Rust-backend.
// Rust-structs gebruiken #[serde(rename_all = "camelCase")] zodat de velden 1-op-1 kloppen.
// De backend stuurt geen zichtbare tekst: alleen id's, codes en waarden. Teksten staan in src/i18n.

export type Lang = "nl" | "en" | "fr";

export interface Msg {
  key: string;
  params: Record<string, string>;
}

export type Risk = "low" | "medium";
export type Gain = "big" | "measurable" | "small" | "negligible";
export type TweakCategory = "system" | "network" | "input" | "cleanup" | "game" | "laptop";
export type Platform = "all" | "pc_only" | "laptop_only";
export type DeviceKind = "pc" | "laptop";
export type DeviceSetting = "auto" | "pc" | "laptop";

export interface DeviceInfo {
  detected: DeviceKind;
  effective: DeviceKind;
  setting: DeviceSetting;
  chassis: number[];
  hasBattery: boolean;
  manufacturer: string;
  model: string;
  vendorApp: string | null;
  hybrid: boolean;
  displayGpu: string | null;
  fastGpu: string | null;
}
export type TweakScope = "global" | "per_game";
export type TweakKind = "toggle" | "action";
export type Timing = "instant" | "game_restart" | "pc_restart";
export type TweakState = "on" | "off" | "unknown" | "unavailable";

export interface TweakInfo {
  id: string;
  category: TweakCategory;
  scope: TweakScope;
  kind: TweakKind;
  risk: Risk;
  gain: Gain;
  timing: Timing;
  experimental: boolean;
  requiresAdmin: boolean;
  state: TweakState;
  appliedByApp: boolean;
  platform: Platform;
  platformLocked: boolean;
  active: "now" | "after_pc_restart" | "after_game_restart" | null;
  options: string[] | null;
  currentOption: string | null;
  detail: Msg | null;
}

export interface AppInfo {
  version: string;
  isElevated: boolean;
  dataDir: string;
  osVersion: string;
  windows11: boolean;
  cpuName: string;
  gpuName: string;
  totalMemoryMb: number;
  uiLanguage: string;
}

export type Accent = "violet" | "cyan" | "emerald" | "rose" | "pink" | "amber";

export type CrosshairShapeId = "cross" | "cross_dot" | "dot" | "circle" | "circle_dot" | "t" | "x" | "square";

export interface CrosshairConfig {
  shape: CrosshairShapeId;
  color: string;
  size: number;
  thickness: number;
  gap: number;
  outline: boolean;
  outlineColor: string;
  outlineThickness: number;
  opacity: number;
  dot: boolean;
  dotSize: number;
  rotation: number;
  offsetX: number;
  offsetY: number;
  showTop: boolean;
  showRight: boolean;
  showBottom: boolean;
  showLeft: boolean;
}

export interface FortniteCheck {
  id: string;
  current: string | null;
  recommended: string;
  ok: boolean;
}

export interface FortniteOverview {
  iniExists: boolean;
  running: boolean;
  gpuVendor: string;
  gpuName: string | null;
  maxHz: number;
  renderMode: "performance" | "dx12" | "dx11" | "default";
  advice: "igpu" | "nvidia" | "amd" | "other";
  checks: FortniteCheck[];
}

export interface FpsDetail {
  avgFps: number;
  low1Fps: number;
  frames: number;
  seconds: number;
  medianMs: number;
  stutters: number;
  stuttersPerMin: number;
  gpuBusy: number | null;
  cpuBusy: number | null;
  graph: number[];
}

export interface Measurement {
  gameId: string;
  gameName: string;
  takenAt: number;
  boostActive: boolean;
  pingHost: string | null;
  fps: FpsDetail;
  ping: PingResult | null;
}

export interface BandwidthUser {
  name: string;
  label: string | null;
  downKbps: number;
  upKbps: number;
}

export interface BandwidthReport {
  seconds: number;
  found: number;
  connections: number;
  totalDownKbps: number;
  totalUpKbps: number;
  users: BandwidthUser[];
  elevated: boolean;
}

export interface BufferbloatResult {
  idleMs: number;
  downloadMs: number | null;
  uploadMs: number | null;
  downloadMbps: number | null;
  uploadMbps: number | null;
  increaseMs: number;
  grade: string;
}

export interface GameConfigCheck {
  gameId: string;
  id: string;
  file: string;
  current: string | null;
  recommended: string;
  ok: boolean;
}

export interface LiveCheck {
  id: string;
  saved: boolean;
  live: boolean;
}

export interface OverlayInfo {
  id: string;
  active: boolean;
  fix: "tweak" | "close" | "manual";
}

export interface HwAccelInfo {
  id: string;
  name: string;
  enabled: boolean;
  running: boolean;
}

export interface CrosshairPreset {
  name: string;
  config: CrosshairConfig;
}

export interface Settings {
  language: Lang | null;
  accent: Accent;
  accentCustom: string | null;
  theme: "midnight" | "oled" | "slate" | "tinted";
  backgroundImage: string | null;
  backgroundDim: number;
  backgroundBlur: number;
  reduceMotion: boolean;
  autoBoostEnabled: boolean;
  restorePointPolicy: "auto" | "never";
  restorePointDone: boolean;
  dashboardPingHost: string;
  closeDuringGaming: string[];
  customPingTargets: unknown[];
  customGames: Game[];
  startMinimized: boolean;
  minimizeToTray: boolean;
  firstRunDone: boolean;
  defaultProfileTweaks: Record<string, boolean>;
  steamGridDbKey: string;
  crosshair: CrosshairConfig;
  crosshairPresets: CrosshairPreset[];
  crosshairEnabled: boolean;
  crosshairHotkey: string;
  autoUpdateCheck: boolean;
  onlineCovers: boolean;
  deviceType: DeviceSetting;
  boostRequiresCharger: boolean;
}

export interface MetricsSample {
  ts: number;
  cpuPct: number;
  ramUsedMb: number;
  ramTotalMb: number;
  ramFreeMb: number;
  gpuPct: number | null;
  gpuTempC: number | null;
  cpuTempC: number | null;
  gpuMemUsedMb: number | null;
  gpuMemTotalMb: number | null;
  processCount: number;
  pingMs: number | null;
  pingHost: string;
  batteryPct: number | null;
  onAc: boolean | null;
  thermalThrottle: boolean | null;
}

export interface MetricsSnapshot {
  latest: MetricsSample | null;
  history: MetricsSample[];
}

export type Launcher = "epic" | "steam" | "battlenet" | "riot" | "ea" | "xbox" | "minecraft" | "custom";

export interface Game {
  id: string;
  name: string;
  launcher: Launcher;
  installPath: string | null;
  exePath: string | null;
  processNames: string[];
  launchUri: string | null;
  known: boolean;
  iconKey: string | null;
  steamAppId: number | null;
  cover: string | null;
  /** Eigen pictogram (bv. het Minecraft-grasblok) voor als er geen afbeelding is. */
  icon?: string | null;
}

export interface GameProfile {
  gameId: string;
  autoBoost: boolean;
  revertOnExit: boolean;
  globalTweaks: Record<string, boolean>;
  fullscreenOptOff: boolean;
  gpuHighPerf: boolean;
  highPriority: boolean;
  closeApps: string[];
  useGlobalCloseList: boolean;
}

export interface BoostSnapshot {
  ts: number;
  freeRamMb: number;
  processCount: number;
  cpuPct: number;
  cpuTempC?: number | null;
  gpuTempC?: number | null;
  throttling?: boolean | null;
  onAc?: boolean | null;
}

export interface AppliedTweak {
  id: string;
  timing: Timing;
  option: string | null;
  count: number | null;
}

export interface ClosedProcess {
  name: string;
  exePath: string | null;
  memoryMb: number;
}

export type ControllerModel = "xbox" | "dualsense" | "dualsense_edge" | "dualshock4" | "ps_adapter";

export interface ControllerInfo {
  /** "xinput:<plek>" of "hid:<pad>". */
  id: string;
  kind: "xbox" | "playstation";
  model: ControllerModel;
  slot: number | null;
  /** true = kabel, false = draadloos, null = onbekend. */
  wired: boolean | null;
  /** Xbox: niveau. */
  battery: "empty" | "low" | "medium" | "full" | "unknown" | null;
  /** PlayStation: procenten (bij benadering). */
  batteryPercent: number | null;
  charging: boolean | null;
}

export interface ControllerRate {
  id: string;
  hz: number | null;
  changes: number;
  seconds: number;
  connected: boolean;
}

export interface FpsResult {
  avgFps: number;
  low1Fps: number;
  frames: number;
  seconds: number;
}

export interface BoostReport {
  sessionId: string;
  gameId: string | null;
  gameName: string | null;
  startedAt: number;
  before: BoostSnapshot;
  after: BoostSnapshot;
  applied: AppliedTweak[];
  alreadyOk: string[];
  skipped: { id: string; reason: Msg }[];
  closedProcesses: ClosedProcess[];
  restorePoint: RestorePointResult | null;
  gameRunning: boolean;
  warnings: Msg[];
  fpsBefore: FpsResult | null;
  fpsAfter: FpsResult | null;
}

export interface BoostState {
  active: boolean;
  report: BoostReport | null;
}

export type JournalKind =
  | "registry"
  | "power_plan"
  | "power_setting"
  | "display"
  | "file"
  | "process"
  | "setting"
  | "cleanup"
  | "startup"
  | "store_app"
  | "adapter"
  | "priority";

export interface JournalEntry {
  id: string;
  ts: number;
  tweakId: string;
  tweakName: string;
  kind: JournalKind;
  target: string;
  oldValue: string | null;
  newValue: string | null;
  reverted: boolean;
  revertable: boolean;
  sessionId: string | null;
  gameId: string | null;
  note: string | null;
  detail: Msg | null;
}

export interface RevertResult {
  reverted: number;
  skipped: number;
  failed: { id: string; target: string; error: string }[];
}

export type ProcessCategory =
  | "browser"
  | "launcher"
  | "cloud"
  | "updater"
  | "communication"
  | "media"
  | "rgb"
  | "game"
  | "system"
  | "security"
  | "anticheat"
  | "vendor"
  | "other";

export interface ProcessInfo {
  pid: number;
  name: string;
  title: string;
  exePath: string | null;
  memoryMb: number;
  cpuPct: number;
  category: ProcessCategory;
  unneeded: boolean;
  instances: number;
}

export interface CloseResult {
  closed: ClosedProcess[];
  failed: string[];
  freedMb: number;
}

export type StartupSource = "hkcu_run" | "hklm_run" | "hklm_run32" | "user_folder" | "common_folder";

export interface StartupItem {
  id: string;
  name: string;
  command: string;
  exePath: string | null;
  source: StartupSource;
  enabled: boolean;
  requiresAdmin: boolean;
}

export type RegionId = "eu" | "na_east" | "na_central" | "na_west" | "brazil" | "middle_east" | "asia" | "oceania";

export interface GameRegion {
  id: string;
  region: RegionId;
  city: string;
  host: string;
}

export interface PingResult {
  host: string;
  ip: string | null;
  sent: number;
  received: number;
  minMs: number | null;
  avgMs: number | null;
  maxMs: number | null;
  jitterMs: number | null;
  lossPct: number;
  samples: (number | null)[];
}

export interface NetworkAdapter {
  name: string;
  description: string;
  kind: "wifi" | "ethernet" | "other";
  status: string;
  linkSpeedMbps: number | null;
  ipv4: string | null;
  gateway: string | null;
  dns: string[];
  mac: string | null;
}

export interface ConnectionOverview {
  adapters: NetworkAdapter[];
  tcpGlobal: Record<string, string>;
}

export interface CleanupItem {
  id: string;
  path: string;
  sizeBytes: number;
  fileCount: number;
  shaderCache: boolean;
  needsAdmin: boolean;
}

export interface CleanupPreview {
  items: CleanupItem[];
  totalBytes: number;
}

export interface CleanupResult {
  freedBytes: number;
  deletedFiles: number;
  skippedFiles: number;
  adminSkipped: string[];
}

export interface FortniteSettings {
  iniPath: string;
  exists: boolean;
  readOnly: boolean;
  frameRateLimit: number | null;
  vsync: boolean | null;
  fullscreenMode: number | null;
  resX: number | null;
  resY: number | null;
  backups: string[];
}

export type CheckStatus = "ok" | "issue" | "info" | "unavailable";
export type CheckAction = "none" | "fix" | "open_page" | "link";

export interface BigWin {
  id: "refresh_rate" | "gpu_choice" | "power_source" | "ram_speed" | "gpu_driver" | "heavy_apps" | "laptop_heat" | "laptop_gpu_mode" | "laptop_wifi" | "laptop_display" | "vendor_apps" | "gpu_battery_limit";
  status: CheckStatus;
  gain: Gain;
  variant: string;
  params: Record<string, string>;
  action: CheckAction;
  link: string | null;
}

export interface RestorePointResult {
  ok: boolean;
  key: "created" | "recent_exists" | "needs_admin" | "protection_off" | "failed";
  detail: string | null;
  ts: number;
}

export interface RestorePointStatus {
  protectionEnabled: boolean | null;
  lastResult: RestorePointResult | null;
}

export interface Notice {
  level: "info" | "success" | "warning" | "error";
  key: string;
  params: Record<string, string>;
}

export interface GameEvent {
  gameId: string;
  name: string;
}

export interface WatcherStatus {
  running: boolean;
  watching: number;
}

/** Foutvorm die de backend stuurt. */
export interface BackendError {
  code: "needs_admin" | "coded" | "not_found" | "error";
  key: string | null;
  params: Record<string, string>;
  message: string;
}

/** Namen van backend-events (tauri `listen`). */
export const EVENTS = {
  metrics: "metrics",
  boostState: "boost-state",
  boostProgress: "boost-progress",
  gameStarted: "game-started",
  gameStopped: "game-stopped",
  watcherStatus: "watcher-status",
  notice: "notice",
  coversUpdated: "covers-updated",
  crosshairConfig: "crosshair-config",
  crosshairState: "crosshair-state",
  restorePoint: "restore-point",
} as const;
