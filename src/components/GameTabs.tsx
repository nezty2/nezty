import { FolderOpen, Gamepad2, Plus, Save, Trash2, Undo2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { GameConfigCard } from "@/components/GameConfigCard";
import { OverlaysCard } from "@/components/OverlaysCard";
import { GainBadge, StateBadge, TimingBadge } from "@/components/TweakCard";
import { useAppIcons } from "@/components/useAppIcons";
import { Badge, Button, Chip, EmptyState, Row, SegmentedControl, Skeleton, Spinner, TextInput, Toggle } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import { api } from "@/lib/ipc";
import { prettyPath } from "@/lib/privacy";
import { formatMb } from "@/lib/format";
import type { FortniteSettings, Game, GameProfile, ProcessInfo, TweakInfo } from "@/lib/types";
import { useApp } from "@/store/app";

/** Tweaks die Game Boost op een laptop standaard meeneemt (zelfde lijst als in de backend). */
const LAPTOP_BOOST_DEFAULTS = ["power_mode_best", "battery_saver_off"];

/* ---------- Boost-instellingen per game ---------- */

export function ProfileTab({ game, profile, onChange }: { game: Game; profile: GameProfile; onChange: (p: GameProfile) => void }) {
  const { t, td, lang } = useT();
  const tweaks = useApp((s) => s.tweaks);
  const settings = useApp((s) => s.settings);
  const [procs, setProcs] = useState<ProcessInfo[] | null>(null);
  const [newApp, setNewApp] = useState("");
  const device = useApp((s) => s.device);
  const isLaptop = device?.effective === "laptop";
  // Tweaks die niet bij dit apparaat passen slaat Game Boost toch over: dan ook niet tonen.
  const boostable = tweaks.filter((tw) => tw.scope === "global" && tw.kind === "toggle" && tw.timing !== "pc_restart" && !tw.platformLocked);
  const boostOn = (id: string) => profile.globalTweaks[id] ?? (isLaptop && LAPTOP_BOOST_DEFAULTS.includes(id));
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
              <Toggle checked={boostOn(tw.id)} onChange={(v) => set({ globalTweaks: { ...profile.globalTweaks, [tw.id]: v } })} label={td(`tweak.${tw.id}.name`)} />
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

export function FortniteTipsTab() {
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
