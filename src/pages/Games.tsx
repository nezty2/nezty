import { FolderOpen, Gamepad2, Lightbulb, Play, Plus, RefreshCw, Save, Trash2, Undo2, Zap } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { GameConfigCard } from "@/components/GameConfigCard";
import { GameCover } from "@/components/GameCover";
import { OverlaysCard } from "@/components/OverlaysCard";
import { GainBadge, StateBadge, TimingBadge } from "@/components/TweakCard";
import { useAppIcons } from "@/components/useAppIcons";
import { Badge, Button, Chip, EmptyState, Modal, PageHeader, Row, SearchInput, SegmentedControl, Skeleton, Spinner, TextInput, Toggle, cn } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import { api, pickExe } from "@/lib/ipc";
import { prettyPath } from "@/lib/privacy";
import { formatMb } from "@/lib/format";
import type { FortniteSettings, Game, GameProfile, ProcessInfo, TweakInfo } from "@/lib/types";
import { useApp } from "@/store/app";

/* ---------- Boost-instellingen per game ---------- */

function ProfileTab({ game, profile, onChange }: { game: Game; profile: GameProfile; onChange: (p: GameProfile) => void }) {
  const { t, td, lang } = useT();
  const tweaks = useApp((s) => s.tweaks);
  const settings = useApp((s) => s.settings);
  const [procs, setProcs] = useState<ProcessInfo[] | null>(null);
  const [newApp, setNewApp] = useState("");
  const boostable = tweaks.filter((tw) => tw.scope === "global" && tw.kind === "toggle" && tw.timing !== "pc_restart");
  const icons = useAppIcons((procs ?? []).map((p) => p.exePath));

  useEffect(() => {
    api.listProcesses().then(setProcs).catch(() => setProcs([]));
  }, []);

  const set = (patch: Partial<GameProfile>) => onChange({ ...profile, ...patch });
  const addApp = (name: string) => {
    const n = name.trim();
    if (!n || profile.closeApps.some((a) => a.toLowerCase() === n.toLowerCase())) return;
    set({ closeApps: [...profile.closeApps, n] });
    setNewApp("");
  };
  const suggestions = (procs ?? [])
    .filter((p) => p.category !== "game")
    .filter((p) => !profile.closeApps.some((a) => a.toLowerCase() === p.name.toLowerCase()))
    .filter((p) => !(settings?.closeDuringGaming ?? []).some((a) => a.toLowerCase() === p.name.toLowerCase()))
    .slice(0, 24);

  return (
    <div className="space-y-6">
      <div className="card-inset divide-y divide-line px-4">
        <Row title={t("profile.autoBoost")} desc={t("profile.autoBoostDesc", { game: game.name })}>
          <Toggle checked={profile.autoBoost} onChange={(v) => set({ autoBoost: v })} label={t("profile.autoBoost")} />
        </Row>
        <Row title={t("profile.revertOnExit")} desc={t("profile.revertOnExitDesc")}>
          <Toggle checked={profile.revertOnExit} onChange={(v) => set({ revertOnExit: v })} label={t("profile.revertOnExit")} />
        </Row>
      </div>

      <section>
        <h3 className="mb-2 text-sm font-semibold">{t("profile.perGame")}</h3>
        <div className="card-inset divide-y divide-line px-4">
          {(
            [
              ["fullscreenOptOff", "fullscreen_opt_off"],
              ["gpuHighPerf", "gpu_high_perf"],
              ["highPriority", "high_priority"],
            ] as const
          ).map(([key, id]) => (
            <Row key={id} title={td(`tweak.${id}.name`)} desc={td(`tweak.${id}.what`)}>
              <Toggle checked={profile[key]} onChange={(v) => set({ [key]: v } as Partial<GameProfile>)} label={td(`tweak.${id}.name`)} />
            </Row>
          ))}
        </div>
      </section>

      <section>
        <h3 className="mb-1 text-sm font-semibold">{t("profile.systemTweaks")}</h3>
        <p className="mb-2 text-[12px] text-ink-3">{t("profile.pcRestartNote")}</p>
        <div className="card-inset divide-y divide-line px-4">
          {boostable.map((tw) => (
            <Row
              key={tw.id}
              title={
                <span className="flex flex-wrap items-center gap-2">
                  {td(`tweak.${tw.id}.name`)}
                  <TimingBadge timing={tw.timing} />
                </span>
              }
              desc={td(`tweak.${tw.id}.what`)}
            >
              <Toggle checked={!!profile.globalTweaks[tw.id]} onChange={(v) => set({ globalTweaks: { ...profile.globalTweaks, [tw.id]: v } })} label={td(`tweak.${tw.id}.name`)} />
            </Row>
          ))}
        </div>
      </section>

      <section>
        <div className="mb-2 flex flex-wrap items-center justify-between gap-2">
          <h3 className="text-sm font-semibold">{t("profile.closeApps")}</h3>
          <label className="flex items-center gap-2 text-[13px] text-ink-2">
            {t("profile.useGlobal", { count: settings?.closeDuringGaming.length ?? 0 })}
            <Toggle checked={profile.useGlobalCloseList} onChange={(v) => set({ useGlobalCloseList: v })} label={t("profile.useGlobal", { count: 0 })} />
          </label>
        </div>
        <div className="flex flex-wrap gap-1.5">
          {profile.closeApps.map((a) => (
            <span key={a} className="inline-flex items-center gap-1 rounded-lg border border-line bg-panel-2 py-1 pl-3 pr-1 text-[13px]">
              {a}
              <button onClick={() => set({ closeApps: profile.closeApps.filter((x) => x !== a) })} className="rounded p-0.5 text-ink-3 hover:text-ink" aria-label={t("common.remove")}>
                <Trash2 className="h-3 w-3" />
              </button>
            </span>
          ))}
          {profile.closeApps.length === 0 && <span className="text-[13px] text-ink-3">{t("profile.noneYet")}</span>}
        </div>
        <div className="mt-3 flex gap-2">
          <TextInput value={newApp} onChange={setNewApp} placeholder={t("profile.placeholder")} className="max-w-xs" />
          <Button icon={Plus} onClick={() => addApp(newApp)} disabled={!newApp.trim()}>
            {t("common.add")}
          </Button>
        </div>
        {procs === null ? (
          <div className="mt-3">
            <Spinner className="h-4 w-4" />
          </div>
        ) : (
          suggestions.length > 0 && (
            <div className="mt-3">
              <div className="mb-1.5 text-[12px] text-ink-3">{t("profile.openNow")}</div>
              <div className="flex flex-wrap gap-1.5">
                {suggestions.map((p) => (
                  <Chip key={p.pid} onClick={() => addApp(p.name)}>
                    {p.exePath && icons[p.exePath] && <img src={icons[p.exePath]!} alt="" className="h-4 w-4" />}
                    {p.title} <span className="text-ink-3">{formatMb(lang, p.memoryMb)}</span>
                  </Chip>
                ))}
              </div>
            </div>
          )
        )}
      </section>
    </div>
  );
}

/* ---------- Nu instellen (los van een boost) ---------- */

export function NowTab({ game, extras = true }: { game: Game; extras?: boolean }) {
  const { t, td } = useT();
  const [list, setList] = useState<TweakInfo[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const setTweak = useApp((s) => s.setTweak);
  const handleError = useApp((s) => s.handleError);
  const load = useCallback(() => {
    api.getGameTweaks(game.id).then(setList).catch(handleError);
  }, [game.id, handleError]);
  useEffect(() => {
    load();
  }, [load]);

  if (!list) return <Skeleton className="h-40" />;
  return (
    <div className="space-y-3">
      <p className="text-[13px] text-ink-3">{t("games.nowHint")}</p>
      <div className="card-inset divide-y divide-line px-4">
        {list.map((tw) => (
          <div key={tw.id} className="grid grid-cols-[1fr_auto] items-center gap-6 py-3.5">
            <div className="min-w-0">
              <div className="text-sm font-medium">{td(`tweak.${tw.id}.name`)}</div>
              <div className="mt-0.5 text-[13px] text-ink-2">{td(`tweak.${tw.id}.what`)}</div>
              <div className="mt-2 flex flex-wrap gap-1.5">
                <TimingBadge timing={tw.timing} />
                <GainBadge gain={tw.gain} />
                <StateBadge tweak={tw} />
              </div>
              {tw.detail && <div className="mt-1.5 text-[12px] text-ink-3">{td(`msg.${tw.detail.key}`, tw.detail.params)}</div>}
            </div>
            <Toggle
              checked={tw.state === "on"}
              disabled={tw.state === "unavailable"}
              loading={busy === tw.id}
              label={td(`tweak.${tw.id}.name`)}
              onChange={async (v) => {
                setBusy(tw.id);
                await setTweak(tw.id, v, null, game.id);
                setBusy(null);
                load();
              }}
            />
          </div>
        ))}
      </div>
      {extras && <GameConfigCard gameId={game.id} />}
      {extras && <OverlaysCard />}
    </div>
  );
}

/* ---------- Fortnite ---------- */

const fpsPresets = [60, 120, 144, 165, 240, 360];

export function FortniteIniTab() {
  const { t } = useT();
  const [data, setData] = useState<FortniteSettings | null>(null);
  const [draft, setDraft] = useState<FortniteSettings | null>(null);
  const [saving, setSaving] = useState(false);
  const toast = useApp((s) => s.toast);
  const handleError = useApp((s) => s.handleError);
  const refreshJournal = useApp((s) => s.refreshJournal);

  useEffect(() => {
    api
      .getFortniteSettings()
      .then((d) => {
        setData(d);
        setDraft(d);
      })
      .catch(handleError);
  }, [handleError]);

  if (!data || !draft) return <Skeleton className="h-40" />;
  if (!data.exists) return <EmptyState icon={Gamepad2} title={t("fortnite.missingTitle")} description={t("msg.ini_missing")} />;
  const dirty = JSON.stringify(draft) !== JSON.stringify(data);

  const save = async () => {
    setSaving(true);
    try {
      const d = await api.saveFortniteSettings(draft);
      setData(d);
      setDraft(d);
      toast({ level: "success", title: t("notice.success"), message: t("fortnite.saved") });
      void refreshJournal();
    } catch (e) {
      handleError(e);
    } finally {
      setSaving(false);
    }
  };
  const restore = async (b: string) => {
    try {
      const d = await api.restoreFortniteBackup(b);
      setData(d);
      setDraft(d);
      toast({ level: "success", title: t("notice.success"), message: t("fortnite.restored") });
      void refreshJournal();
    } catch (e) {
      handleError(e);
    }
  };
  const num = "h-9 w-24 rounded-lg border border-line bg-panel-2 px-2 text-sm text-ink focus:border-accent focus:outline-none";

  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-center gap-2 text-[12px] text-ink-3">
        <FolderOpen className="h-3.5 w-3.5" />
        <span className="selectable truncate">{prettyPath(data.iniPath)}</span>
        {data.readOnly && <Badge variant="warn">{t("fortnite.readOnly")}</Badge>}
      </div>
      <div className="card-inset divide-y divide-line px-4">
        <div className="py-3.5">
          <div className="mb-2 text-sm font-medium">{t("fortnite.fpsLimit")}</div>
          <div className="flex flex-wrap items-center gap-1.5">
            {fpsPresets.map((f) => (
              <Chip key={f} active={draft.frameRateLimit === f} onClick={() => setDraft({ ...draft, frameRateLimit: f })}>
                {f}
              </Chip>
            ))}
            <Chip active={draft.frameRateLimit === 0} onClick={() => setDraft({ ...draft, frameRateLimit: 0 })}>
              {t("fortnite.unlimited")}
            </Chip>
            <span className="ml-2 text-[12px] text-ink-3">{t("fortnite.custom")}</span>
            <input type="number" min={0} max={1000} value={draft.frameRateLimit ?? ""} onChange={(e) => setDraft({ ...draft, frameRateLimit: e.target.value === "" ? null : Number(e.target.value) })} className={num} />
          </div>
        </div>
        <Row title={t("fortnite.vsync")} desc={t("fortnite.vsyncHint")}>
          <Toggle checked={draft.vsync ?? false} onChange={(v) => setDraft({ ...draft, vsync: v })} label={t("fortnite.vsync")} />
        </Row>
        <Row title={t("fortnite.windowMode")}>
          <SegmentedControl
            options={[
              { value: "0", label: t("fortnite.mode0") },
              { value: "1", label: t("fortnite.mode1") },
              { value: "2", label: t("fortnite.mode2") },
            ]}
            value={String(draft.fullscreenMode ?? 0)}
            onChange={(v) => setDraft({ ...draft, fullscreenMode: Number(v) })}
          />
        </Row>
        <Row title={t("fortnite.resolution")}>
          <input type="number" value={draft.resX ?? ""} onChange={(e) => setDraft({ ...draft, resX: e.target.value === "" ? null : Number(e.target.value) })} className={num} />
          <span className="text-ink-3">×</span>
          <input type="number" value={draft.resY ?? ""} onChange={(e) => setDraft({ ...draft, resY: e.target.value === "" ? null : Number(e.target.value) })} className={num} />
        </Row>
      </div>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="max-w-md text-[12px] text-ink-3">{t("fortnite.saveHint")}</p>
        <Button variant="primary" icon={Save} loading={saving} disabled={!dirty} onClick={() => void save()}>
          {t("fortnite.save")}
        </Button>
      </div>
      {data.backups.length > 0 && (
        <div>
          <div className="mb-1.5 text-sm font-semibold">{t("fortnite.backups")}</div>
          <ul className="card-inset divide-y divide-line">
            {data.backups.slice(0, 6).map((b) => (
              <li key={b} className="flex items-center justify-between gap-3 px-3 py-2 text-[12px]">
                <span className="selectable truncate font-mono text-ink-2">{b}</span>
                <Button size="sm" variant="ghost" icon={Undo2} onClick={() => void restore(b)}>
                  {t("fortnite.restore")}
                </Button>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

const tipGroups: { group: "display" | "quality" | "advanced"; items: string[] }[] = [
  { group: "display", items: ["window_mode", "resolution", "vsync", "fps_limit", "rendering"] },
  { group: "quality", items: ["res3d", "view_distance", "shadows", "aa", "textures", "effects"] },
  { group: "advanced", items: ["reflex", "multithread", "motion_blur", "shader_cache"] },
];

function FortniteTipsTab() {
  const { t, td } = useT();
  return (
    <div className="space-y-5">
      <p className="text-[13px] text-ink-2">{t("fortniteTips.intro")}</p>
      {tipGroups.map((g) => (
        <section key={g.group}>
          <h3 className="mb-2 text-sm font-semibold">{t(`fortniteTips.groups.${g.group}`)}</h3>
          <div className="card-inset divide-y divide-line">
            {g.items.map((id) => (
              <div key={id} className="grid gap-1 px-4 py-3 sm:grid-cols-[220px_1fr] sm:gap-4">
                <div>
                  <div className="text-sm font-medium">{td(`fortniteTips.items.${id}.setting`)}</div>
                  <div className="text-[13px] text-accent">{td(`fortniteTips.items.${id}.value`)}</div>
                </div>
                <div className="text-[13px] leading-relaxed text-ink-2">{td(`fortniteTips.items.${id}.why`)}</div>
              </div>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}

/* ---------- Detailvenster ---------- */

type Tab = "profile" | "now" | "fortnite" | "tips";

function GameDetail({ game, onClose }: { game: Game; onClose: () => void }) {
  const { t } = useT();
  const profileFor = useApp((s) => s.profileFor);
  const saveProfile = useApp((s) => s.saveProfile);
  const runningGames = useApp((s) => s.runningGames);
  const boost = useApp((s) => s.boost);
  const runBoost = useApp((s) => s.runBoost);
  const refreshGames = useApp((s) => s.refreshGames);
  const handleError = useApp((s) => s.handleError);
  const toast = useApp((s) => s.toast);
  const [tab, setTab] = useState<Tab>("profile");
  const [profile, setProfile] = useState<GameProfile>(() => profileFor(game.id));
  const isFortnite = game.id === "epic:fortnite";
  const running = runningGames.includes(game.id);
  const tabs: Tab[] = isFortnite ? ["profile", "now", "fortnite", "tips"] : ["profile", "now"];

  return (
    <Modal
      open
      onClose={onClose}
      width="max-w-3xl"
      title={
        <span className="flex items-center gap-3">
          <span className="h-10 w-[86px] shrink-0 overflow-hidden rounded-md border border-line">
            <GameCover game={game} />
          </span>
          <span className="min-w-0">
            <span className="flex items-center gap-2">
              {game.name}
              {running && <Badge variant="ok">{t("games.playing")}</Badge>}
            </span>
            <span className="block text-[12px] font-normal text-ink-3">{t(`launcher.${game.launcher}`)}</span>
          </span>
        </span>
      }
      footer={
        <>
          {game.launcher === "custom" && (
            <Button
              variant="danger"
              icon={Trash2}
              onClick={async () => {
                try {
                  await api.removeCustomGame(game.id);
                  toast({ level: "info", title: t("notice.info"), message: t("games.removed", { name: game.name }) });
                  onClose();
                  void refreshGames();
                } catch (e) {
                  handleError(e);
                }
              }}
            >
              {t("common.remove")}
            </Button>
          )}
          <div className="flex-1" />
          <Button icon={Play} onClick={() => api.launchGame(game.id).catch(handleError)}>
            {t("games.launch")}
          </Button>
          <Button variant="primary" icon={Zap} disabled={boost.active} onClick={() => void runBoost(game.id, running).then(() => onClose())}>
            {t("games.boostNow")}
          </Button>
        </>
      }
    >
      <div className="mb-5 flex gap-1 border-b border-line">
        {tabs.map((id) => (
          <button key={id} onClick={() => setTab(id)} className={cn("-mb-px border-b-2 px-3 py-2 text-sm font-medium transition-colors", tab === id ? "border-accent text-ink" : "border-transparent text-ink-3 hover:text-ink-2")}>
            {id === "tips" && <Lightbulb className="mr-1 inline h-3.5 w-3.5" />}
            {t(`games.tabs.${id}`)}
          </button>
        ))}
      </div>
      {tab === "profile" && (
        <ProfileTab
          game={game}
          profile={profile}
          onChange={(p) => {
            setProfile(p);
            void saveProfile(p);
          }}
        />
      )}
      {tab === "now" && <NowTab game={game} />}
      {tab === "fortnite" && <FortniteIniTab />}
      {tab === "tips" && <FortniteTipsTab />}
    </Modal>
  );
}

/* ---------- Pagina ---------- */

export function GamesPage() {
  const { t } = useT();
  const games = useApp((s) => s.games);
  const loading = useApp((s) => s.gamesLoading);
  const refreshGames = useApp((s) => s.refreshGames);
  const runningGames = useApp((s) => s.runningGames);
  const profiles = useApp((s) => s.profiles);
  const handleError = useApp((s) => s.handleError);
  const toast = useApp((s) => s.toast);
  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);

  const filtered = useMemo(() => games.filter((g) => g.name.toLowerCase().includes(search.toLowerCase())), [games, search]);
  const selectedGame = games.find((g) => g.id === selected) ?? null;

  const addGame = async () => {
    const path = await pickExe(t("games.add"));
    if (!path) return;
    setAdding(true);
    try {
      const g = await api.addCustomGame(path, null);
      toast({ level: "success", title: t("notice.success"), message: t("games.added", { name: g.name }) });
      await refreshGames();
      setSelected(g.id);
    } catch (e) {
      handleError(e);
    } finally {
      setAdding(false);
    }
  };

  return (
    <div className="space-y-6">
      <PageHeader
        title={t("games.title")}
        subtitle={t("games.subtitle")}
        actions={
          <>
            <SearchInput value={search} onChange={setSearch} placeholder={t("games.search")} className="w-56" />
            <Button icon={RefreshCw} onClick={() => void refreshGames()} loading={loading}>
              {t("games.rescan")}
            </Button>
            <Button variant="primary" icon={Plus} onClick={() => void addGame()} loading={adding}>
              {t("games.add")}
            </Button>
          </>
        }
      />

      {loading && games.length === 0 ? (
        <div className="grid grid-cols-2 gap-4 lg:grid-cols-3 2xl:grid-cols-4">
          {Array.from({ length: 6 }).map((_, i) => (
            <Skeleton key={i} className="aspect-[460/275]" />
          ))}
        </div>
      ) : filtered.length === 0 ? (
        <EmptyState icon={Gamepad2} title={games.length === 0 ? t("games.emptyTitle") : t("games.noMatch")} description={games.length === 0 ? t("games.emptyDesc") : undefined} />
      ) : (
        <div className="grid grid-cols-2 gap-4 lg:grid-cols-3 2xl:grid-cols-4">
          {filtered.map((g) => {
            const p = profiles.find((x) => x.gameId === g.id);
            const running = runningGames.includes(g.id);
            return (
              <button key={g.id} onClick={() => setSelected(g.id)} className="card group overflow-hidden text-left transition-colors hover:border-line-2">
                <div className="relative aspect-[460/215] overflow-hidden border-b border-line">
                  <GameCover game={g} className="transition-transform duration-300 group-hover:scale-[1.03]" />
                  <div className="absolute left-2 top-2 flex gap-1.5">
                    {running && <Badge variant="ok" className="!bg-[#0f1115]/90">{t("games.playing")}</Badge>}
                    {p?.autoBoost && <Badge variant="accent" className="!bg-[#0f1115]/90">{t("games.autoBoost")}</Badge>}
                  </div>
                </div>
                <div className="px-3.5 py-3">
                  <div className="truncate text-sm font-semibold">{g.name}</div>
                  <div className="mt-0.5 flex items-center gap-2 text-[12px] text-ink-3">
                    {t(`launcher.${g.launcher}`)}
                    {!g.exePath && <span className="text-warn">· {t("games.noExe")}</span>}
                  </div>
                </div>
              </button>
            );
          })}
        </div>
      )}

      {selectedGame && <GameDetail key={selectedGame.id} game={selectedGame} onClose={() => setSelected(null)} />}
    </div>
  );
}
