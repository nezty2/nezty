// Typed laag bovenop Tauri `invoke`. Buiten Tauri (gewone browser) valt dit terug op nepdata,
// zodat de UI los van de backend te bekijken en te testen is.
import type * as T from "./types";
import { mockApi, mockListen } from "./mock";

export const isTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in (window as unknown as Record<string, unknown>);

async function call<R>(cmd: string, args?: Record<string, unknown>): Promise<R> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<R>(cmd, args);
}

export interface Api {
  getAppInfo(): Promise<T.AppInfo>;
  getSettings(): Promise<T.Settings>;
  saveSettings(settings: T.Settings): Promise<T.Settings>;
  openPath(path: string): Promise<void>;
  openUrl(url: string): Promise<void>;
  getRestorePointStatus(): Promise<T.RestorePointStatus>;
  createRestorePoint(): Promise<T.RestorePointResult>;
  getMetrics(): Promise<T.MetricsSnapshot>;
  listTweaks(): Promise<T.TweakInfo[]>;
  getGameTweaks(gameId: string): Promise<T.TweakInfo[]>;
  setTweak(tweakId: string, enabled: boolean, option: string | null, gameId?: string | null): Promise<T.TweakInfo>;
  previewCleanup(): Promise<T.CleanupPreview>;
  runCleanup(itemIds: string[]): Promise<T.CleanupResult>;
  getBigWins(): Promise<T.BigWin[]>;
  fixBigWin(id: string): Promise<T.BigWin[]>;
  getJournal(): Promise<T.JournalEntry[]>;
  revertAll(): Promise<T.RevertResult>;
  revertEntry(entryId: string): Promise<T.RevertResult>;
  exportJournal(): Promise<string>;
  detectGames(): Promise<T.Game[]>;
  refreshCovers(): Promise<void>;
  addCustomGame(exePath: string, name: string | null): Promise<T.Game>;
  removeCustomGame(gameId: string): Promise<void>;
  getProfiles(): Promise<T.GameProfile[]>;
  saveProfile(profile: T.GameProfile): Promise<T.GameProfile>;
  launchGame(gameId: string): Promise<void>;
  getRunningGames(): Promise<string[]>;
  boost(gameId: string | null, measureFps: boolean): Promise<T.BoostReport>;
  unboost(): Promise<T.RevertResult>;
  getBoostState(): Promise<T.BoostState>;
  restartClosedApps(): Promise<number>;
  getWatcherStatus(): Promise<T.WatcherStatus>;
  listProcesses(): Promise<T.ProcessInfo[]>;
  killProcess(pid: number): Promise<T.CloseResult>;
  closeApps(names: string[]): Promise<T.CloseResult>;
  getIcons(paths: string[]): Promise<Record<string, string | null>>;
  getStoreBackground(): Promise<boolean>;
  setStoreBackground(allowed: boolean): Promise<void>;
  listStartupItems(): Promise<T.StartupItem[]>;
  setStartupItem(itemId: string, enabled: boolean): Promise<T.StartupItem>;
  getGameRegions(): Promise<T.GameRegion[]>;
  pingHost(host: string, count: number): Promise<T.PingResult>;
  getConnectionOverview(): Promise<T.ConnectionOverview>;
  flushDns(): Promise<void>;
  getFortniteSettings(): Promise<T.FortniteSettings>;
  saveFortniteSettings(settings: T.FortniteSettings): Promise<T.FortniteSettings>;
  restoreFortniteBackup(backupPath: string): Promise<T.FortniteSettings>;
  getFortniteOverview(): Promise<T.FortniteOverview>;
  applyFortniteRecommended(ids: string[]): Promise<T.FortniteOverview>;
  listOverlays(): Promise<T.OverlayInfo[]>;
  verifyInputLive(): Promise<T.LiveCheck[]>;
  measureBandwidth(seconds: number): Promise<T.BandwidthReport>;
  setBackground(path: string | null): Promise<T.Settings>;
  getBackgroundFile(): Promise<string | null>;
  measureSession(gameId: string, seconds: number, pingHost: string | null): Promise<T.Measurement>;
  runBufferbloat(): Promise<T.BufferbloatResult>;
  syncInputLive(): Promise<T.LiveCheck[]>;
  listGameConfig(gameId: string): Promise<T.GameConfigCheck[]>;
  applyGameConfig(gameId: string): Promise<T.GameConfigCheck[]>;
  closeOverlay(id: string): Promise<T.OverlayInfo[]>;
  listHwAccel(): Promise<T.HwAccelInfo[]>;
  setHwAccel(id: string, enabled: boolean): Promise<T.HwAccelInfo[]>;
  setCrosshairEnabled(enabled: boolean): Promise<boolean>;
  getCrosshairConfig(): Promise<T.CrosshairConfig>;
  runCheckup(): Promise<T.TestCheck[]>;
  runTweakTest(): Promise<T.TweakTestStep[]>;
  listControllers(): Promise<T.ControllerInfo[]>;
  measureControllerRate(id: string, seconds?: number): Promise<T.ControllerRate>;
}

const tauriApi: Api = {
  getAppInfo: () => call("get_app_info"),
  getSettings: () => call("get_settings"),
  saveSettings: (settings) => call("save_settings", { settings }),
  openPath: (path) => call("open_path", { path }),
  openUrl: (url) => call("open_url", { url }),
  getRestorePointStatus: () => call("get_restore_point_status"),
  createRestorePoint: () => call("create_restore_point"),
  getMetrics: () => call("get_metrics"),
  listTweaks: () => call("list_tweaks"),
  getGameTweaks: (gameId) => call("get_game_tweaks", { gameId }),
  setTweak: (tweakId, enabled, option, gameId) => call("set_tweak", { tweakId, enabled, option, gameId: gameId ?? null }),
  previewCleanup: () => call("preview_cleanup"),
  runCleanup: (itemIds) => call("run_cleanup", { itemIds }),
  getBigWins: () => call("get_big_wins"),
  fixBigWin: (id) => call("fix_big_win", { id }),
  getJournal: () => call("get_journal"),
  revertAll: () => call("revert_all"),
  revertEntry: (entryId) => call("revert_entry", { entryId }),
  exportJournal: () => call("export_journal"),
  detectGames: () => call("detect_games"),
  refreshCovers: () => call("refresh_covers"),
  addCustomGame: (exePath, name) => call("add_custom_game", { exePath, name }),
  removeCustomGame: (gameId) => call("remove_custom_game", { gameId }),
  getProfiles: () => call("get_profiles"),
  saveProfile: (profile) => call("save_profile", { profile }),
  launchGame: (gameId) => call("launch_game", { gameId }),
  getRunningGames: () => call("get_running_games"),
  boost: (gameId, measureFps) => call("boost", { gameId, measureFps }),
  unboost: () => call("unboost"),
  getBoostState: () => call("get_boost_state"),
  restartClosedApps: () => call("restart_closed_apps"),
  getWatcherStatus: () => call("get_watcher_status"),
  listProcesses: () => call("list_processes"),
  killProcess: (pid) => call("kill_process", { pid }),
  closeApps: (names) => call("close_apps", { names }),
  getIcons: (paths) => call("get_icons", { paths }),
  getStoreBackground: () => call("get_store_background"),
  setStoreBackground: (allowed) => call("set_store_background", { allowed }),
  listStartupItems: () => call("list_startup_items"),
  setStartupItem: (itemId, enabled) => call("set_startup_item", { itemId, enabled }),
  getGameRegions: () => call("get_game_regions"),
  pingHost: (host, count) => call("ping_host", { host, count }),
  getConnectionOverview: () => call("get_connection_overview"),
  flushDns: () => call("flush_dns"),
  getFortniteSettings: () => call("get_fortnite_settings"),
  saveFortniteSettings: (settings) => call("save_fortnite_settings", { settings }),
  restoreFortniteBackup: (backupPath) => call("restore_fortnite_backup", { backupPath }),
  getFortniteOverview: () => call("get_fortnite_overview"),
  applyFortniteRecommended: (ids) => call("apply_fortnite_recommended", { ids }),
  listOverlays: () => call("list_overlays"),
  verifyInputLive: () => call("verify_input_live"),
  measureBandwidth: (seconds) => call("measure_bandwidth", { seconds }),
  setBackground: (path) => call("set_background", { path }),
  getBackgroundFile: () => call("get_background_file"),
  measureSession: (gameId, seconds, pingHost) => call("measure_session", { gameId, seconds, pingHost }),
  runBufferbloat: () => call("run_bufferbloat"),
  syncInputLive: () => call("sync_input_live"),
  listGameConfig: (gameId) => call("list_game_config", { gameId }),
  applyGameConfig: (gameId) => call("apply_game_config", { gameId }),
  closeOverlay: (id) => call("close_overlay", { id }),
  listHwAccel: () => call("list_hw_accel"),
  setHwAccel: (id, enabled) => call("set_hw_accel", { id, enabled }),
  setCrosshairEnabled: (enabled) => call("set_crosshair_enabled", { enabled }),
  getCrosshairConfig: () => call("get_crosshair_config"),
  runCheckup: () => call("run_checkup"),
  runTweakTest: () => call("run_tweak_test"),
  listControllers: () => call("list_controllers"),
  measureControllerRate: (id, seconds) => call("measure_controller_rate", { id, seconds: seconds ?? null }),
};

export const api: Api = isTauri ? tauriApi : mockApi;

/** Luister naar een backend-event. Geeft een unsubscribe-functie terug. */
export async function listenEvent<P>(name: string, handler: (payload: P) => void): Promise<() => void> {
  if (!isTauri) return mockListen<P>(name, handler);
  const { listen } = await import("@tauri-apps/api/event");
  return listen<P>(name, (e) => handler(e.payload));
}

/** Bestandskiezer voor een .exe. */
export async function pickExe(title: string): Promise<string | null> {
  if (!isTauri) return "C:\\Games\\MyGame\\MyGame.exe";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const result = await open({ multiple: false, directory: false, title, filters: [{ name: "exe", extensions: ["exe"] }] });
  return typeof result === "string" ? result : null;
}

/** Kies een afbeelding voor de achtergrond. */
export async function pickImage(title: string): Promise<string | null> {
  if (!isTauri) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const result = await open({ multiple: false, directory: false, title, filters: [{ name: "Afbeelding", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }] });
  return typeof result === "string" ? result : null;
}

/** Lokaal bestand (bv. afbeelding in de cache) als URL voor <img>. */
export async function fileUrl(path: string): Promise<string> {
  if (!isTauri) return path;
  const { convertFileSrc } = await import("@tauri-apps/api/core");
  return convertFileSrc(path);
}

/** Starten met Windows (geplande taak met hoogste rechten, zie autostart.rs). */
export const autostart = {
  async isEnabled(): Promise<boolean> {
    if (!isTauri) return false;
    return call<boolean>("get_autostart");
  },
  async set(enabled: boolean): Promise<void> {
    if (!isTauri) return;
    await call<void>("set_autostart", { enabled });
  },
};

/** Zet een fout van de backend om naar de vorm { code, key, params, message }. */
export function asBackendError(e: unknown): T.BackendError {
  if (e && typeof e === "object" && "code" in e) {
    const o = e as Partial<T.BackendError>;
    return { code: (o.code as T.BackendError["code"]) ?? "error", key: o.key ?? null, params: o.params ?? {}, message: o.message ?? "" };
  }
  return { code: "error", key: null, params: {}, message: typeof e === "string" ? e : String(e) };
}
