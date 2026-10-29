// Nepdata voor gebruik in een gewone browser (zonder Tauri). Alleen om de UI te bekijken en te testen.
// Doet niets met het echte systeem.
import type * as T from "./types";
import type { Api } from "./ipc";

type Handler = (payload: unknown) => void;
const listeners = new Map<string, Set<Handler>>();

function emit(name: string, payload: unknown) {
  listeners.get(name)?.forEach((h) => h(payload));
}

export function mockListen<P>(name: string, handler: (p: P) => void): () => void {
  const set = listeners.get(name) ?? new Set<Handler>();
  set.add(handler as Handler);
  listeners.set(name, set);
  return () => {
    set.delete(handler as Handler);
  };
}

const delay = (ms = 120) => new Promise((r) => setTimeout(r, ms));
const now = () => Date.now();
const msg = (key: string, params: Record<string, string> = {}): T.Msg => ({ key, params });

let fortniteOverview: T.FortniteOverview = {
  iniExists: true,
  running: false,
  gpuVendor: "nvidia",
  gpuName: "NVIDIA GeForce RTX 5070 Ti",
  maxHz: 144,
  renderMode: "dx12",
  advice: "nvidia",
  checks: [
    { id: "vsync", current: "False", recommended: "False", ok: true },
    { id: "fps_limit", current: "240.000000", recommended: "0", ok: false },
    { id: "reflex", current: "1", recommended: "2", ok: false },
    { id: "render_mode", current: "dx12", recommended: "performance", ok: false },
  ],
};
let liveChecks: T.LiveCheck[] = [
  { id: "mouse_precision_off", saved: true, live: true },
  { id: "keyboard_fast_repeat", saved: true, live: false },
  { id: "accessibility_hotkeys_off", saved: false, live: false },
];
let gameConfig: T.GameConfigCheck[] = [
  { gameId: "steam:cs2", id: "vsync", file: "C:\\Program Files (x86)\\Steam\\userdata\\123\\730\\local\\cfg\\cs2_video.txt", current: "1", recommended: "0", ok: false },
];
let overlays: T.OverlayInfo[] = [
  { id: "game_bar", active: true, fix: "tweak" },
  { id: "discord", active: true, fix: "manual" },
  { id: "nvidia", active: true, fix: "manual" },
  { id: "medal", active: true, fix: "close" },
];
let hwAccel: T.HwAccelInfo[] = [
  { id: "discord", name: "Discord", enabled: true, running: true },
  { id: "chrome", name: "Google Chrome", enabled: true, running: false },
];

const defaultCrosshair: T.CrosshairConfig = {
  shape: "cross", color: "#00ff7f", size: 8, thickness: 2, gap: 4, outline: true, outlineColor: "#000000", outlineThickness: 1, opacity: 1, dot: false,
  dotSize: 3, rotation: 0, offsetX: 0, offsetY: 0, showTop: true, showRight: true, showBottom: true, showLeft: true,
};

let settings: T.Settings = {
  language: null,
  accent: "violet",
  accentCustom: null,
  theme: "midnight",
  backgroundImage: null,
  backgroundDim: 55,
  backgroundBlur: 6,
  reduceMotion: false,
  autoBoostEnabled: true,
  restorePointPolicy: "auto",
  restorePointDone: false,
  dashboardPingHost: "1.1.1.1",
  closeDuringGaming: ["OneDrive.exe"],
  customPingTargets: [],
  customGames: [],
  minimizeToTray: false,
  startMinimized: false,
  firstRunDone: true,
  defaultProfileTweaks: { game_mode: true, game_bar_dvr_off: true, power_plan: true, mouse_precision_off: true },
  steamGridDbKey: "",
  crosshair: defaultCrosshair,
  crosshairPresets: [],
  crosshairEnabled: false,
  crosshairHotkey: "ctrl+alt+c",
  autoUpdateCheck: true,
  onlineCovers: true,
};

function tw(p: Partial<T.TweakInfo> & { id: string; category: T.TweakCategory }): T.TweakInfo {
  return {
    scope: "global", kind: "toggle", risk: "low", gain: "small", timing: "instant", experimental: false, requiresAdmin: false,
    state: "off", appliedByApp: false, active: null, options: null, currentOption: null, detail: null, ...p,
  };
}

let tweaks: T.TweakInfo[] = [
  tw({ id: "game_mode", category: "system", timing: "game_restart", state: "on", active: "now" }),
  tw({ id: "game_bar_dvr_off", category: "system", gain: "measurable", timing: "game_restart", state: "off" }),
  tw({ id: "power_plan", category: "system", options: ["high", "ultimate"], currentOption: "high", detail: msg("power_current", { name: "Balanced" }) }),
  tw({ id: "wifi_power_max", category: "network", state: "unavailable", detail: msg("wifi_none") }),
  tw({ id: "delivery_optimization_off", category: "network", requiresAdmin: true }),
  tw({ id: "timer_resolution", category: "system", gain: "negligible", timing: "pc_restart", experimental: true, requiresAdmin: true }),
  tw({ id: "interrupt_moderation_off", category: "network", gain: "negligible", experimental: true, requiresAdmin: true, state: "on", active: "now" }),
  tw({ id: "notifications_off", category: "system", gain: "negligible", timing: "pc_restart" }),
  tw({ id: "hags", category: "system", risk: "medium", timing: "pc_restart", requiresAdmin: true, state: "on", active: "after_pc_restart" }),
  tw({ id: "visual_effects", category: "system", gain: "negligible" }),
  tw({ id: "mouse_precision_off", category: "input", gain: "measurable", state: "on", active: "now" }),
  tw({ id: "keyboard_fast_repeat", category: "input" }),
  tw({ id: "accessibility_hotkeys_off", category: "input", state: "on", active: "now" }),
  tw({ id: "input_device_power_off", category: "input", requiresAdmin: true, state: "off", detail: { key: "device_power_count", params: { n: "3" } } }),
  tw({ id: "lang_hotkey_off", category: "input", state: "off" }),
  tw({ id: "usb_suspend_off", category: "input" }),
  tw({ id: "adapter_power_saving_off", category: "network", requiresAdmin: true, state: "unknown", detail: msg("adapter_needs_admin") }),
  tw({ id: "nagle_off", category: "network", gain: "negligible", timing: "pc_restart", experimental: true, requiresAdmin: true, detail: msg("nagle_ifaces", { count: "1" }) }),
  tw({ id: "network_throttling_off", category: "network", gain: "negligible", timing: "pc_restart", experimental: true, requiresAdmin: true }),
  tw({ id: "fullscreen_opt_off", category: "game", scope: "per_game", timing: "game_restart", state: "unknown", detail: msg("per_game_hint") }),
  tw({ id: "gpu_high_perf", category: "game", scope: "per_game", gain: "big", timing: "game_restart", state: "unknown", detail: msg("per_game_hint") }),
  tw({ id: "high_priority", category: "game", scope: "per_game", state: "unknown", detail: msg("per_game_hint") }),
  tw({ id: "cleanup_temp", category: "cleanup", kind: "action", gain: "negligible", state: "unknown" }),
];

const svgCover = (a: string, b: string, text: string) =>
  `data:image/svg+xml;utf8,${encodeURIComponent(`<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 460 215'><defs><linearGradient id='g' x1='0' y1='0' x2='1' y2='1'><stop offset='0' stop-color='${a}'/><stop offset='1' stop-color='${b}'/></linearGradient></defs><rect width='460' height='215' fill='url(#g)'/><text x='24' y='190' font-family='Segoe UI' font-weight='800' font-size='44' fill='white'>${text}</text></svg>`)}`;

function g(id: string, name: string, launcher: T.Launcher, exe: string | null, procs: string[], steamAppId: number | null, cover: string | null): T.Game {
  return { id, name, launcher, installPath: exe ? exe.replace(/[\\/][^\\/]+$/, "") : null, exePath: exe, processNames: procs, launchUri: null, known: true, iconKey: null, steamAppId, cover };
}

let games: T.Game[] = [
  g("epic:fortnite", "Fortnite", "epic", "C:\\Program Files\\Epic Games\\Fortnite\\FortniteGame\\Binaries\\Win64\\FortniteClient-Win64-Shipping.exe", ["FortniteClient-Win64-Shipping.exe"], null, svgCover("#3b82f6", "#a855f7", "FORTNITE")),
  g("steam:cs2", "Counter-Strike 2", "steam", "D:\\Steam\\steamapps\\common\\Counter-Strike Global Offensive\\game\\bin\\win64\\cs2.exe", ["cs2.exe"], 730, svgCover("#f59e0b", "#7c2d12", "CS2")),
  g("steam:apex", "Apex Legends", "steam", "D:\\Steam\\steamapps\\common\\Apex Legends\\r5apex_dx12.exe", ["r5apex_dx12.exe"], 1172470, null),
  g("riot:valorant", "VALORANT", "riot", "C:\\Riot Games\\VALORANT\\live\\ShooterGame\\Binaries\\Win64\\VALORANT-Win64-Shipping.exe", ["VALORANT-Win64-Shipping.exe"], null, null),
  g("steam:cod", "Call of Duty", "steam", "D:\\Steam\\steamapps\\common\\Call of Duty HQ\\cod.exe", ["cod.exe"], 1938090, null),
  g("minecraft:java", "Minecraft (Java Edition)", "minecraft", null, ["javaw.exe"], null, null),
];

let profiles: T.GameProfile[] = games.map((x) => ({
  gameId: x.id, autoBoost: x.id === "epic:fortnite", revertOnExit: true, globalTweaks: { ...settings.defaultProfileTweaks },
  fullscreenOptOff: true, gpuHighPerf: true, highPriority: true, closeApps: [], useGlobalCloseList: true,
}));

let journal: T.JournalEntry[] = [
  { id: "j1", ts: now() - 3600_000, tweakId: "mouse_precision_off", tweakName: "Enhance pointer precision off", kind: "registry", target: "HKCU\\Control Panel\\Mouse\\MouseSpeed", oldValue: "\"1\"", newValue: "\"0\"", reverted: false, revertable: true, sessionId: null, gameId: null, note: null, detail: null },
  { id: "j2", ts: now() - 7200_000, tweakId: "close_apps", tweakName: "Close background apps", kind: "process", target: "OneDrive.exe", oldValue: "@running", newValue: "@closed", reverted: false, revertable: false, sessionId: "s0", gameId: "epic:fortnite", note: "not_revertable", detail: null },
  { id: "j3", ts: now() - 90_000_000, tweakId: "game_bar_dvr_off", tweakName: "Xbox Game Bar and Game DVR off", kind: "registry", target: "HKCU\\...\\GameDVR\\AppCaptureEnabled", oldValue: "1 (0x1)", newValue: "0 (0x0)", reverted: true, revertable: true, sessionId: "s0", gameId: "epic:fortnite", note: null, detail: null },
];

let boostState: T.BoostState = { active: false, report: null };
let running: string[] = ["epic:fortnite"];
let history: T.MetricsSample[] = [];
let cpu = 18;
let gpu = 62;
let ping = 9;

function sample(): T.MetricsSample {
  cpu = Math.min(100, Math.max(2, cpu + (Math.random() - 0.5) * 10));
  gpu = Math.min(100, Math.max(0, gpu + (Math.random() - 0.5) * 14));
  ping = Math.max(4, ping + (Math.random() - 0.5) * 3);
  const total = 31861;
  const used = 14800 + Math.round(Math.sin(now() / 20000) * 600);
  return {
    ts: now(), cpuPct: Math.round(cpu * 10) / 10, ramUsedMb: used, ramTotalMb: total, ramFreeMb: total - used,
    gpuPct: Math.round(gpu), gpuTempC: 44 + Math.round(gpu / 6), cpuTempC: null, gpuMemUsedMb: 5300, gpuMemTotalMb: 16303,
    processCount: 212 - (boostState.active ? 3 : 0), pingMs: Math.round(ping * 10) / 10, pingHost: settings.dashboardPingHost,
  };
}
for (let i = 0; i < 60; i++) history.push(sample());
setInterval(() => {
  const s = sample();
  history = [...history.slice(-119), s];
  emit("metrics", s);
}, 1000);

const processes: T.ProcessInfo[] = [
  { pid: 1337, name: "chrome.exe", title: "Google Chrome", exePath: "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe", memoryMb: 2121, cpuPct: 3.2, category: "browser", unneeded: true, instances: 24 },
  { pid: 2201, name: "OneDrive.exe", title: "Microsoft OneDrive", exePath: "C:\\Users\\you\\AppData\\Local\\Microsoft\\OneDrive\\OneDrive.exe", memoryMb: 96, cpuPct: 0.1, category: "cloud", unneeded: true, instances: 1 },
  { pid: 3500, name: "GoogleUpdate.exe", title: "Google Installer", exePath: "C:\\Program Files (x86)\\Google\\Update\\GoogleUpdate.exe", memoryMb: 12, cpuPct: 0, category: "updater", unneeded: true, instances: 1 },
  { pid: 4000, name: "FortniteClient-Win64-Shipping.exe", title: "Fortnite", exePath: "C:\\Program Files\\Epic Games\\Fortnite\\FortniteGame\\Binaries\\Win64\\FortniteClient-Win64-Shipping.exe", memoryMb: 2275, cpuPct: 21.4, category: "game", unneeded: false, instances: 1 },
  { pid: 2460, name: "Discord.exe", title: "Discord", exePath: "C:\\Users\\you\\AppData\\Local\\Discord\\app-1.0.9266\\Discord.exe", memoryMb: 1904, cpuPct: 1.1, category: "communication", unneeded: false, instances: 6 },
  { pid: 3011, name: "Spotify.exe", title: "Spotify", exePath: "C:\\Users\\you\\AppData\\Roaming\\Spotify\\Spotify.exe", memoryMb: 310, cpuPct: 0.8, category: "media", unneeded: false, instances: 5 },
  { pid: 3900, name: "SteelSeriesGG.exe", title: "SteelSeries GG", exePath: "C:\\Program Files\\SteelSeries\\GG\\SteelSeriesGG.exe", memoryMb: 160, cpuPct: 0.4, category: "rgb", unneeded: false, instances: 3 },
];

let storeBg = true;

let startup: T.StartupItem[] = [
  { id: "hkcu_run:Discord", name: "Discord", command: "C:\\Users\\you\\AppData\\Local\\Discord\\Update.exe --processStart Discord.exe", exePath: null, source: "hkcu_run", enabled: true, requiresAdmin: false },
  { id: "hkcu_run:OneDrive", name: "OneDrive", command: "\"C:\\Users\\you\\AppData\\Local\\Microsoft\\OneDrive\\OneDrive.exe\" /background", exePath: null, source: "hkcu_run", enabled: true, requiresAdmin: false },
  { id: "hklm_run:SecurityHealth", name: "Windows Security notification icon", command: "%windir%\\system32\\SecurityHealthSystray.exe", exePath: null, source: "hklm_run", enabled: true, requiresAdmin: true },
  { id: "hkcu_run:Steam", name: "Steam", command: "\"C:\\Program Files (x86)\\Steam\\steam.exe\" -silent", exePath: null, source: "hkcu_run", enabled: false, requiresAdmin: false },
];

const regions: T.GameRegion[] = [
  { id: "eu-central", region: "eu", city: "Frankfurt", host: "ec2.eu-central-1.amazonaws.com:443" },
  { id: "eu-west-2", region: "eu", city: "London", host: "ec2.eu-west-2.amazonaws.com:443" },
  { id: "eu-west-1", region: "eu", city: "Dublin", host: "ec2.eu-west-1.amazonaws.com:443" },
  { id: "us-east-1", region: "na_east", city: "Virginia", host: "ec2.us-east-1.amazonaws.com:443" },
  { id: "us-east-2", region: "na_central", city: "Ohio", host: "ec2.us-east-2.amazonaws.com:443" },
  { id: "sa-east-1", region: "brazil", city: "São Paulo", host: "ec2.sa-east-1.amazonaws.com:443" },
];

let bigWins: T.BigWin[] = [
  { id: "refresh_rate", status: "issue", gain: "big", variant: "one", params: { monitor: "AOC 24G2W1G4", current: "60", max: "144", count: "1" }, action: "fix", link: null },
  { id: "gpu_choice", status: "issue", gain: "big", variant: "missing", params: { gpu: "NVIDIA GeForce RTX 5070 Ti", count: "5", names: "Apex Legends, Call of Duty, Counter-Strike 2" }, action: "fix", link: null },
  { id: "power_source", status: "ok", gain: "big", variant: "desktop", params: { plan: "High performance" }, action: "none", link: null },
  { id: "ram_speed", status: "ok", gain: "big", variant: "ok", params: { configured: "6000", rated: "6000", type: "DDR5" }, action: "none", link: null },
  { id: "gpu_driver", status: "ok", gain: "measurable", variant: "ok", params: { gpu: "NVIDIA GeForce RTX 5070 Ti", version: "621.01", date: "2026-10-19", days: "9", vendor: "nvidia" }, action: "link", link: "https://www.nvidia.com/en-us/drivers/" },
  { id: "heavy_apps", status: "info", gain: "measurable", variant: "plenty_ram", params: { names: "Google Chrome, Discord", mb: "4025", count: "2" }, action: "open_page", link: null },
];

export const mockApi: Api = {
  async getAppInfo() {
    await delay();
    return { version: "0.3.0", isElevated: true, dataDir: "C:\\Users\\you\\AppData\\Roaming\\com.nzty.desktop", osVersion: "Windows 11 (build 26300)", windows11: true, cpuName: "AMD Ryzen 7 9800X3D 8-Core Processor", gpuName: "NVIDIA GeForce RTX 5070 Ti", totalMemoryMb: 31861, uiLanguage: "nl" };
  },
  async getSettings() { await delay(); return settings; },
  async saveSettings(s) { await delay(); settings = s; return settings; },
  async openPath() { await delay(); },
  async openUrl(url) { if (url.startsWith("https://")) window.open(url, "_blank"); },
  async getRestorePointStatus() { await delay(); return { protectionEnabled: true, lastResult: null }; },
  async createRestorePoint() { await delay(900); settings = { ...settings, restorePointDone: true }; return { ok: true, key: "created", detail: null, ts: now() }; },
  async getMetrics() { return { latest: history[history.length - 1] ?? null, history }; },
  async listTweaks() { await delay(200); return tweaks; },
  async getGameTweaks(gameId) {
    await delay(200);
    const game = games.find((x) => x.id === gameId);
    return tweaks.filter((t) => t.scope === "per_game").map((t) => ({
      ...t,
      state: !game?.exePath ? "unavailable" : t.id === "high_priority" ? (running.includes(gameId) ? "on" : "off") : t.id === "fullscreen_opt_off" ? "on" : "off",
      detail: !game?.exePath ? msg("no_exe", { game: game?.name ?? "" }) : t.id === "high_priority" && !running.includes(gameId) ? msg("game_not_running") : null,
    }));
  },
  async setTweak(id, enabled, option, gameId) {
    await delay(350);
    if (id === "usb_suspend_off" && enabled && Math.random() < 0.15) throw { code: "coded", key: "verify_failed", params: {}, message: "verify_failed" };
    const t = tweaks.find((x) => x.id === id)!;
    if (gameId) return { ...t, state: enabled ? "on" : "off", appliedByApp: enabled };
    tweaks = tweaks.map((x) => (x.id === id ? { ...x, state: enabled ? "on" : "off", appliedByApp: enabled, currentOption: option ?? x.currentOption } : x));
    journal = [{ id: `j${now()}`, ts: now(), tweakId: id, tweakName: id, kind: "registry", target: `HKCU\\...\\${id}`, oldValue: enabled ? "0" : "1", newValue: enabled ? "1" : "0", reverted: false, revertable: true, sessionId: null, gameId: null, note: enabled ? null : "manual_off", detail: null }, ...journal];
    return tweaks.find((x) => x.id === id)!;
  },
  async previewCleanup() {
    await delay(600);
    const items: T.CleanupItem[] = [
      { id: "temp", path: "C:\\Users\\you\\AppData\\Local\\Temp", sizeBytes: 418_000_000, fileCount: 4613, shaderCache: false, needsAdmin: false },
      { id: "wintemp", path: "C:\\Windows\\Temp", sizeBytes: 4_100_000, fileCount: 18, shaderCache: false, needsAdmin: false },
      { id: "nv-dx", path: "C:\\Users\\you\\AppData\\Local\\NVIDIA\\DXCache", sizeBytes: 3_860_000_000, fileCount: 225, shaderCache: true, needsAdmin: false },
      { id: "d3d", path: "C:\\Users\\you\\AppData\\Local\\D3DSCache", sizeBytes: 903_000_000, fileCount: 79, shaderCache: true, needsAdmin: false },
    ];
    return { items, totalBytes: items.reduce((a, b) => a + b.sizeBytes, 0) };
  },
  async runCleanup(ids) { await delay(900); return { freedBytes: ids.length * 200_000_000, deletedFiles: ids.length * 300, skippedFiles: 12, adminSkipped: [] }; },
  async getBigWins() { await delay(500); return bigWins; },
  async fixBigWin(id) {
    await delay(800);
    bigWins = bigWins.map((b) => (b.id === id ? { ...b, status: "ok", variant: id === "refresh_rate" ? "ok" : "set", params: { ...b.params, current: b.params.max ?? b.params.current }, action: "none" } : b));
    return bigWins;
  },
  async getJournal() { await delay(); return journal; },
  async revertAll() {
    await delay(700);
    const n = journal.filter((j) => !j.reverted && j.revertable).length;
    journal = journal.map((j) => ({ ...j, reverted: j.revertable ? true : j.reverted }));
    tweaks = tweaks.map((t) => (t.appliedByApp ? { ...t, appliedByApp: false, state: "off" } : t));
    boostState = { active: false, report: null };
    emit("boost-state", boostState);
    return { reverted: n, skipped: 1, failed: [] };
  },
  async revertEntry(id) { await delay(300); journal = journal.map((j) => (j.id === id ? { ...j, reverted: true } : j)); return { reverted: 1, skipped: 0, failed: [] }; },
  async exportJournal() { await delay(); return "C:\\Users\\you\\Desktop\\nezty-log.txt"; },
  async detectGames() { await delay(600); return [...games, ...settings.customGames]; },
  async refreshCovers() { await delay(); },
  async addCustomGame(exePath, name) {
    await delay();
    const file = exePath.split(/[\\/]/).pop() ?? "game.exe";
    const x: T.Game = { id: `custom:${now()}`, name: name || file.replace(/\.exe$/i, ""), launcher: "custom", installPath: null, exePath, processNames: [file], launchUri: null, known: false, iconKey: null, steamAppId: null, cover: null };
    settings = { ...settings, customGames: [...settings.customGames, x] };
    return x;
  },
  async removeCustomGame(id) { await delay(); settings = { ...settings, customGames: settings.customGames.filter((x) => x.id !== id) }; },
  async getProfiles() { await delay(); return profiles; },
  async saveProfile(p) { await delay(); profiles = profiles.some((x) => x.gameId === p.gameId) ? profiles.map((x) => (x.gameId === p.gameId ? p : x)) : [...profiles, p]; return p; },
  async launchGame() { await delay(); },
  async getRunningGames() { return running; },
  async boost(gameId, measureFps) {
    const gameRunning = !!gameId && running.includes(gameId);
    if (measureFps && gameRunning) { emit("boost-progress", { stage: "measure_before" }); await delay(1500); }
    emit("boost-progress", { stage: "applying" });
    await delay(1200);
    if (measureFps && gameRunning) { emit("boost-progress", { stage: "measure_after" }); await delay(1500); }
    const game = games.find((x) => x.id === gameId) ?? null;
    const report: T.BoostReport = {
      sessionId: `s${now()}`, gameId, gameName: game?.name ?? null, startedAt: now(),
      before: { ts: now() - 2000, freeRamMb: 16900, processCount: 212, cpuPct: 24 },
      after: { ts: now(), freeRamMb: 19050, processCount: 186, cpuPct: 19 },
      applied: [
        { id: "game_bar_dvr_off", timing: "game_restart", option: null, count: null },
        { id: "power_plan", timing: "instant", option: "high", count: null },
        { id: "gpu_high_perf", timing: "game_restart", option: null, count: null },
        { id: "high_priority", timing: "instant", option: null, count: 1 },
      ],
      alreadyOk: ["game_mode", "mouse_precision_off", "fullscreen_opt_off"],
      skipped: [{ id: "hags", reason: msg("pc_restart_only") }],
      closedProcesses: [{ name: "OneDrive.exe", exePath: null, memoryMb: 96 }, { name: "chrome.exe", exePath: null, memoryMb: 2050 }],
      restorePoint: settings.restorePointDone ? null : { ok: true, key: "created", detail: null, ts: now() },
      gameRunning,
      warnings: measureFps && !gameRunning ? [msg("fps_game_not_running")] : [],
      fpsBefore: measureFps && gameRunning ? { avgFps: 231.4, low1Fps: 142.7, frames: 2314, seconds: 10 } : null,
      fpsAfter: measureFps && gameRunning ? { avgFps: 238.9, low1Fps: 161.2, frames: 2389, seconds: 10 } : null,
    };
    settings = { ...settings, restorePointDone: true };
    boostState = { active: true, report };
    emit("boost-progress", { stage: "done" });
    emit("boost-state", boostState);
    return report;
  },
  async unboost() { await delay(800); boostState = { active: false, report: null }; emit("boost-state", boostState); return { reverted: 4, skipped: 2, failed: [] }; },
  async getBoostState() { return boostState; },
  async restartClosedApps() { await delay(500); return 2; },
  async getWatcherStatus() { return { running: true, watching: profiles.filter((p) => p.autoBoost).length }; },
  async listProcesses() { await delay(300); return processes.map((p) => ({ ...p, cpuPct: Math.max(0, Math.round((p.cpuPct + (Math.random() - 0.5)) * 10) / 10) })); },
  async killProcess(pid) {
    await delay(300);
    const i = processes.findIndex((p) => p.pid === pid);
    const p = processes[i];
    if (p) processes.splice(i, 1);
    return { closed: p ? [{ name: p.name, exePath: p.exePath, memoryMb: p.memoryMb }] : [], failed: [], freedMb: p?.memoryMb ?? 0 };
  },
  async closeApps(names) {
    await delay(500);
    const closed: T.ClosedProcess[] = [];
    for (const n of names) {
      const i = processes.findIndex((p) => p.name.toLowerCase() === n.toLowerCase());
      if (i >= 0) { closed.push({ name: processes[i].name, exePath: processes[i].exePath, memoryMb: processes[i].memoryMb }); processes.splice(i, 1); }
    }
    return { closed, failed: [], freedMb: closed.reduce((a, b) => a + b.memoryMb, 0) };
  },
  async getIcons(paths) { await delay(100); return Object.fromEntries(paths.map((p) => [p, null])); },
  async getStoreBackground() { await delay(); return storeBg; },
  async setStoreBackground(allowed) { await delay(); storeBg = allowed; },
  async listStartupItems() { await delay(300); return startup; },
  async setStartupItem(id, enabled) { await delay(250); startup = startup.map((s) => (s.id === id ? { ...s, enabled } : s)); return startup.find((s) => s.id === id)!; },
  async getGameRegions() { await delay(); return regions; },
  async pingHost(host, count) {
    await delay(300 + Math.random() * 500);
    const base = host.includes("eu-central") ? 9 : host.includes("eu-west-2") ? 13 : host.includes("eu-west-1") ? 21 : host.includes("us-east-1") ? 86 : host.includes("us-east-2") ? 98 : host.includes("sa-east") ? 190 : host.includes("1.1.1.1") ? 4 : 6;
    const samples = Array.from({ length: count }, () => Math.round((base + Math.random() * 3) * 10) / 10);
    const avg = samples.reduce((a, b) => a + b, 0) / samples.length;
    return { host, ip: "203.0.113.42", sent: count, received: count, minMs: Math.min(...samples), avgMs: Math.round(avg * 10) / 10, maxMs: Math.max(...samples), jitterMs: 0.9, lossPct: 0, samples };
  },
  async getConnectionOverview() {
    await delay(500);
    return {
      adapters: [{ name: "Ethernet", description: "Realtek Gaming 2.5GbE Family Controller", kind: "ethernet", status: "Up", linkSpeedMbps: 1000, ipv4: "192.168.2.4", gateway: "192.168.2.254", dns: ["1.1.1.1", "1.0.0.1"], mac: "9C-6B-00-xx-xx-xx" }],
      tcpGlobal: {},
    };
  },
  async flushDns() { await delay(300); },
  async getFortniteSettings() {
    await delay();
    return { iniPath: "C:\\Users\\you\\AppData\\Local\\FortniteGame\\Saved\\Config\\WindowsClient\\GameUserSettings.ini", exists: true, readOnly: false, frameRateLimit: 240, vsync: false, fullscreenMode: 0, resX: 1920, resY: 1080, backups: ["GameUserSettings.ini.nzty-backup-20261027-203011"] };
  },
  async saveFortniteSettings(s) { await delay(500); return { ...s, backups: [`GameUserSettings.ini.nzty-backup-${now()}`, ...s.backups] }; },
  async restoreFortniteBackup() { await delay(500); return mockApi.getFortniteSettings(); },
  async setCrosshairEnabled(enabled) { await delay(); settings = { ...settings, crosshairEnabled: enabled }; return enabled; },
  async getCrosshairConfig() { return settings.crosshair; },
  async getFortniteOverview() {
    await delay(300);
    return fortniteOverview;
  },
  async applyFortniteRecommended(ids) {
    await delay(600);
    fortniteOverview = { ...fortniteOverview, renderMode: ids.includes("render_mode") ? "performance" : fortniteOverview.renderMode, checks: fortniteOverview.checks.map((c) => (ids.includes(c.id) ? { ...c, ok: true, current: c.recommended } : c)) };
    return fortniteOverview;
  },
  async setBackground(path) {
    await delay(200);
    settings = { ...settings, backgroundImage: path ? "background.png" : null };
    return settings;
  },
  async getBackgroundFile() {
    return null;
  },
  async measureBandwidth() {
    await delay(3000);
    return {
      seconds: 3,
      found: 26,
      connections: 26,
      totalDownKbps: 41200,
      totalUpKbps: 3100,
      elevated: true,
      users: [
        { name: "svchost.exe", label: "delivery_optimization", downKbps: 180, upKbps: 2900 },
        { name: "steamwebhelper.exe", label: "steam", downKbps: 38000, upKbps: 120 },
        { name: "OneDrive.exe", label: "onedrive", downKbps: 2400, upKbps: 60 },
      ],
    };
  },
  async runBufferbloat() {
    await delay(2500);
    return { idleMs: 9, downloadMs: 61, uploadMs: 23, downloadMbps: 480, uploadMbps: 46, increaseMs: 52, grade: "B" };
  },
  async verifyInputLive() {
    await delay(200);
    return liveChecks;
  },
  async syncInputLive() {
    await delay(400);
    liveChecks = liveChecks.map((c) => ({ ...c, live: c.saved ? true : c.live }));
    return liveChecks;
  },
  async listGameConfig(gameId) {
    await delay(200);
    return gameConfig.filter((c) => c.gameId === gameId);
  },
  async applyGameConfig(gameId) {
    await delay(400);
    gameConfig = gameConfig.map((c) => (c.gameId === gameId ? { ...c, ok: true, current: c.recommended } : c));
    return gameConfig.filter((c) => c.gameId === gameId);
  },
  async listOverlays() {
    await delay(200);
    return overlays;
  },
  async closeOverlay(id) {
    await delay(400);
    overlays = overlays.filter((o) => o.id !== id);
    return overlays;
  },
  async listHwAccel() {
    await delay(200);
    return hwAccel;
  },
  async setHwAccel(id, enabled) {
    await delay(400);
    hwAccel = hwAccel.map((h) => (h.id === id ? { ...h, enabled } : h));
    return hwAccel;
  },
  async listControllers() { await delay(); return [{ id: "xinput:0", kind: "xbox", model: "xbox", slot: 0, wired: false, battery: "medium", batteryPercent: null, charging: null }, { id: "hid:mock-dualsense", kind: "playstation", model: "dualsense", slot: null, wired: true, battery: null, batteryPercent: 80, charging: true }]; },
  async measureControllerRate(id, seconds) { await delay((seconds ?? 4) * 1000); const ps = id.startsWith("hid:"); return { id, hz: ps ? 250 : 125, changes: ps ? 1000 : 480, seconds: seconds ?? 4, connected: true }; },
};
