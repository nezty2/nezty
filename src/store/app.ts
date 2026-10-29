import { create } from "zustand";
import { resolveLang, translate } from "@/i18n";
import { api, asBackendError, listenEvent } from "@/lib/ipc";
import type * as T from "@/lib/types";
import { EVENTS } from "@/lib/types";

export type PageId = "dashboard" | "games" | "tweaks" | "input" | "controller" | "console" | "crosshair" | "background" | "network" | "startup" | "log" | "settings";

export interface Toast {
  id: number;
  level: "info" | "success" | "warning" | "error";
  title: string;
  message: string;
  detail?: string;
  action?: { label: string; onClick: () => void };
}

interface AppStore {
  page: PageId;
  pageParams: Record<string, string>;
  setPage: (page: PageId, params?: Record<string, string>) => void;

  ready: boolean;
  lang: T.Lang;
  appInfo: T.AppInfo | null;
  settings: T.Settings | null;
  metrics: T.MetricsSnapshot;
  tweaks: T.TweakInfo[];
  tweaksLoading: boolean;
  games: T.Game[];
  gamesLoading: boolean;
  coversVersion: number;
  profiles: T.GameProfile[];
  runningGames: string[];
  boost: T.BoostState;
  boosting: boolean;
  boostStage: string | null;
  lastReport: T.BoostReport | null;
  showResult: boolean;
  bigWins: T.BigWin[] | null;
  journal: T.JournalEntry[];
  watcher: T.WatcherStatus | null;
  restorePoint: T.RestorePointStatus | null;
  toasts: Toast[];

  init: () => Promise<void>;
  tr: (key: string, params?: Record<string, string | number | null | undefined>) => string;
  toast: (t: Omit<Toast, "id">) => void;
  dismissToast: (id: number) => void;
  /** Toont een fout van de backend in gewone taal. */
  handleError: (e: unknown) => void;
  updateSettings: (patch: Partial<T.Settings>) => Promise<boolean>;
  refreshTweaks: () => Promise<void>;
  refreshGames: () => Promise<void>;
  refreshProfiles: () => Promise<void>;
  refreshJournal: () => Promise<void>;
  refreshRestorePoint: () => Promise<void>;
  refreshBigWins: () => Promise<void>;
  setTweak: (id: string, enabled: boolean, option?: string | null, gameId?: string | null) => Promise<T.TweakInfo | null>;
  runBoost: (gameId: string | null, measureFps: boolean) => Promise<T.BoostReport | null>;
  runUnboost: () => Promise<void>;
  setShowResult: (v: boolean) => void;
  saveProfile: (p: T.GameProfile) => Promise<void>;
  relaunchElevated: () => Promise<void>;
  profileFor: (gameId: string) => T.GameProfile;
}

let toastSeq = 1;

export const useApp = create<AppStore>()((set, get) => ({
  page: "dashboard",
  pageParams: {},
  setPage: (page, params = {}) => set({ page, pageParams: params }),

  ready: false,
  lang: resolveLang(null),
  appInfo: null,
  settings: null,
  metrics: { latest: null, history: [] },
  tweaks: [],
  tweaksLoading: false,
  games: [],
  gamesLoading: false,
  coversVersion: 0,
  profiles: [],
  runningGames: [],
  boost: { active: false, report: null },
  boosting: false,
  boostStage: null,
  lastReport: null,
  showResult: false,
  bigWins: null,
  journal: [],
  watcher: null,
  restorePoint: null,
  toasts: [],

  async init() {
    if (get().ready) return;
    try {
      const [appInfo, settings, metrics, boost, restorePoint, watcher] = await Promise.all([
        api.getAppInfo(),
        api.getSettings(),
        api.getMetrics(),
        api.getBoostState(),
        api.getRestorePointStatus(),
        api.getWatcherStatus(),
      ]);
      set({ appInfo, settings, metrics, boost, restorePoint, watcher, ready: true, lang: resolveLang(settings.language, appInfo.uiLanguage), lastReport: boost.report });
    } catch (e) {
      set({ ready: true });
      get().handleError(e);
    }
    void get().refreshTweaks();
    void get().refreshGames();
    void get().refreshJournal();
    void get().refreshBigWins();

    void listenEvent<T.MetricsSample>(EVENTS.metrics, (s) =>
      set((st) => ({ metrics: { latest: s, history: [...st.metrics.history.slice(-119), s] } })),
    );
    void listenEvent<T.BoostState>(EVENTS.boostState, (b) => {
      set((st) => ({ boost: b, lastReport: b.report ?? st.lastReport }));
      void get().refreshTweaks();
      void get().refreshJournal();
    });
    void listenEvent<Record<string, string>>(EVENTS.boostProgress, (p) => set({ boostStage: p.stage ?? null }));
    void listenEvent<T.Notice>(EVENTS.notice, (n) => {
      const tr = get().tr;
      get().toast({ level: n.level, title: tr(`notice.${n.level}`), message: tr(`msg.${n.key}`, n.params) });
    });
    void listenEvent<T.GameEvent>(EVENTS.gameStarted, (e) => {
      set((st) => ({ runningGames: Array.from(new Set([...st.runningGames, e.gameId])) }));
      get().toast({ level: "info", title: get().tr("notice.gameStarted"), message: e.name });
    });
    void listenEvent<T.GameEvent>(EVENTS.gameStopped, (e) => set((st) => ({ runningGames: st.runningGames.filter((g) => g !== e.gameId) })));
    void listenEvent<T.WatcherStatus>(EVENTS.watcherStatus, (w) => set({ watcher: w }));
    // Nieuwe afbeelding in de cache: games opnieuw ophalen (zonder opnieuw te zoeken).
    let coverTimer: ReturnType<typeof setTimeout> | null = null;
    void listenEvent<string>(EVENTS.coversUpdated, () => {
      if (coverTimer) clearTimeout(coverTimer);
      coverTimer = setTimeout(async () => {
        try {
          const games = await api.detectGames();
          set((st) => ({ games, coversVersion: st.coversVersion + 1 }));
        } catch {
          /* niet kritisch */
        }
      }, 600);
    });
    void listenEvent<boolean>(EVENTS.crosshairState, (enabled) => {
      const s = get().settings;
      if (s) set({ settings: { ...s, crosshairEnabled: enabled } });
    });
    // Automatisch herstelpunt vóór de eerste wijziging: status en instelling bijwerken (de melding komt via "notice").
    void listenEvent<T.RestorePointResult>(EVENTS.restorePoint, (r) => {
      const s = get().settings;
      if (r.ok && s) set({ settings: { ...s, restorePointDone: true } });
      void get().refreshRestorePoint();
    });
  },

  tr(key, params) {
    return translate(get().lang, key, params);
  },

  toast(t) {
    const id = toastSeq++;
    set((st) => ({ toasts: [...st.toasts.slice(-3), { ...t, id }] }));
    const ttl = t.level === "error" || t.level === "warning" ? 9000 : t.action ? 8000 : 4500;
    setTimeout(() => get().dismissToast(id), ttl);
  },

  dismissToast(id) {
    set((st) => ({ toasts: st.toasts.filter((t) => t.id !== id) }));
  },

  handleError(e) {
    const err = asBackendError(e);
    const tr = get().tr;
    if (err.code === "needs_admin") {
      get().toast({
        level: "warning",
        title: tr("notice.warning"),
        message: tr("msg.needs_admin"),
        action: { label: tr("common.restartAdmin"), onClick: () => void get().relaunchElevated() },
      });
      return;
    }
    if (err.code === "coded" && err.key) {
      get().toast({ level: "error", title: tr("notice.error"), message: tr(`msg.${err.key}`, err.params) });
      return;
    }
    if (err.code === "not_found") {
      get().toast({ level: "error", title: tr("notice.error"), message: tr("msg.not_found"), detail: err.message });
      return;
    }
    get().toast({ level: "error", title: tr("notice.error"), message: tr("msg.generic"), detail: err.message });
  },

  async updateSettings(patch) {
    const cur = get().settings;
    if (!cur) return false;
    const next = { ...cur, ...patch };
    set({ settings: next });
    if ("language" in patch) set({ lang: resolveLang(next.language, get().appInfo?.uiLanguage) });
    try {
      const saved = await api.saveSettings(next);
      set({ settings: saved });
      return true;
    } catch (e) {
      set({ settings: cur, lang: resolveLang(cur.language, get().appInfo?.uiLanguage) });
      get().handleError(e);
      return false;
    }
  },

  async refreshTweaks() {
    set({ tweaksLoading: true });
    try {
      set({ tweaks: await api.listTweaks() });
    } catch (e) {
      get().handleError(e);
    } finally {
      set({ tweaksLoading: false });
    }
  },

  async refreshGames() {
    set({ gamesLoading: true });
    try {
      const [games, profiles, runningGames] = await Promise.all([api.detectGames(), api.getProfiles(), api.getRunningGames()]);
      set({ games, profiles, runningGames });
    } catch (e) {
      get().handleError(e);
    } finally {
      set({ gamesLoading: false });
    }
  },

  async refreshProfiles() {
    try {
      set({ profiles: await api.getProfiles() });
    } catch (e) {
      get().handleError(e);
    }
  },

  async refreshJournal() {
    try {
      set({ journal: await api.getJournal() });
    } catch (e) {
      get().handleError(e);
    }
  },

  async refreshRestorePoint() {
    try {
      set({ restorePoint: await api.getRestorePointStatus() });
    } catch {
      /* niet kritisch */
    }
  },

  async refreshBigWins() {
    try {
      set({ bigWins: await api.getBigWins() });
    } catch (e) {
      get().handleError(e);
    }
  },

  async setTweak(id, enabled, option = null, gameId = null) {
    try {
      const info = await api.setTweak(id, enabled, option, gameId);
      if (!gameId) set((st) => ({ tweaks: st.tweaks.map((t) => (t.id === id ? info : t)) }));
      void get().refreshJournal();
      return info;
    } catch (e) {
      get().handleError(e);
      // Status opnieuw ophalen, zodat de schakelaar de echte stand laat zien.
      void get().refreshTweaks();
      return null;
    }
  },

  async runBoost(gameId, measureFps) {
    set({ boosting: true, boostStage: null });
    try {
      const report = await api.boost(gameId, measureFps);
      set({ boost: { active: true, report }, lastReport: report, showResult: true });
      void get().refreshTweaks();
      void get().refreshJournal();
      void get().refreshRestorePoint();
      // De backend kan instellingen hebben bijgewerkt (bv. herstelpunt gemaakt).
      void api.getSettings().then((s) => set({ settings: s })).catch(() => undefined);
      return report;
    } catch (e) {
      get().handleError(e);
      return null;
    } finally {
      set({ boosting: false, boostStage: null });
    }
  },

  async runUnboost() {
    set({ boosting: true });
    try {
      const r = await api.unboost();
      set({ boost: { active: false, report: null } });
      get().toast({ level: r.failed.length ? "warning" : "success", title: get().tr("notice.success"), message: get().tr("boost.reverted", { count: r.reverted }) });
      void get().refreshTweaks();
      void get().refreshJournal();
    } catch (e) {
      get().handleError(e);
    } finally {
      set({ boosting: false });
    }
  },

  setShowResult(v) {
    set({ showResult: v });
  },

  async saveProfile(p) {
    set((st) => ({ profiles: st.profiles.some((x) => x.gameId === p.gameId) ? st.profiles.map((x) => (x.gameId === p.gameId ? p : x)) : [...st.profiles, p] }));
    try {
      await api.saveProfile(p);
      void api.getWatcherStatus().then((w) => set({ watcher: w })).catch(() => undefined);
    } catch (e) {
      get().handleError(e);
    }
  },

  async relaunchElevated() {
    try {
      await api.relaunchElevated();
    } catch (e) {
      get().handleError(e);
    }
  },

  profileFor(gameId) {
    const st = get();
    return (
      st.profiles.find((p) => p.gameId === gameId) ?? {
        gameId,
        autoBoost: false,
        revertOnExit: true,
        globalTweaks: { ...(st.settings?.defaultProfileTweaks ?? {}) },
        fullscreenOptOff: true,
        gpuHighPerf: true,
        highPriority: true,
        closeApps: [],
        useGlobalCloseList: true,
      }
    );
  },
}));
