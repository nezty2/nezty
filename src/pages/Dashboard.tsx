import { Activity, AlertTriangle, ChevronRight, Cpu, Gauge, History, MemoryStick, Monitor, Play, ShieldAlert, ShieldCheck, Sparkles, Thermometer, Undo2, Zap, type LucideIcon } from "lucide-react";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { GameCover } from "@/components/GameCover";
import { Sparkline } from "@/components/Sparkline";
import { Badge, Button, Card, Chip, PageHeader, SectionTitle, cn } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import { api } from "@/lib/ipc";
import { formatMb, formatMs, formatPct, formatTemp, timeAgo } from "@/lib/format";
import { useApp } from "@/store/app";

function StatTile({ icon: Icon, label, value, sub, series, min, max, format }: { icon: LucideIcon; label: string; value: string; sub?: ReactNode; series: (number | null)[]; min?: number; max?: number; format?: (v: number) => string }) {
  return (
    <div className="card flex min-w-0 flex-col p-4">
      <div className="flex items-center gap-2 text-[13px] text-ink-3">
        <Icon className="h-4 w-4" />
        {label}
      </div>
      <div className="mt-2 text-[26px] font-semibold leading-none tracking-tight tnum">{value}</div>
      <div className="mt-1.5 truncate text-[12px] text-ink-3">{sub}</div>
      <Sparkline values={series} min={min} max={max} height={36} className="mt-3 h-9" format={format} />
    </div>
  );
}

function BoostCard() {
  const { t, lang } = useT();
  const games = useApp((s) => s.games);
  const boost = useApp((s) => s.boost);
  const boosting = useApp((s) => s.boosting);
  const stage = useApp((s) => s.boostStage);
  const runBoost = useApp((s) => s.runBoost);
  const runUnboost = useApp((s) => s.runUnboost);
  const setShowResult = useApp((s) => s.setShowResult);
  const settings = useApp((s) => s.settings);
  const runningGames = useApp((s) => s.runningGames);
  const profileFor = useApp((s) => s.profileFor);
  const tweaks = useApp((s) => s.tweaks);
  const [target, setTarget] = useState<string | null>(null);
  const [measure, setMeasure] = useState(true);

  // Draait er een game? Dan die vooraf kiezen.
  useEffect(() => {
    if (target == null && runningGames.length > 0) setTarget(runningGames[0]);
  }, [runningGames, target]);

  const sortedGames = useMemo(() => [...games].sort((a, b) => Number(runningGames.includes(b.id)) - Number(runningGames.includes(a.id))).slice(0, 9), [games, runningGames]);
  const profile = target ? profileFor(target) : null;
  const tweakMap = profile?.globalTweaks ?? settings?.defaultProfileTweaks ?? {};
  const globalCount = tweaks.filter((tw) => tw.scope === "global" && tw.kind === "toggle" && tw.timing !== "pc_restart" && tweakMap[tw.id]).length;
  const perGameCount = profile ? [profile.fullscreenOptOff, profile.gpuHighPerf, profile.highPriority].filter(Boolean).length : 0;
  const closeCount = profile ? (profile.useGlobalCloseList ? (settings?.closeDuringGaming.length ?? 0) : 0) + profile.closeApps.length : (settings?.closeDuringGaming.length ?? 0);
  const targetGame = games.find((g) => g.id === target) ?? null;
  const targetRunning = !!target && runningGames.includes(target);
  const report = boost.report;

  if (boost.active && report) {
    const closedMb = report.closedProcesses.reduce((a, b) => a + b.memoryMb, 0);
    return (
      <Card className="flex flex-col gap-4">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <div className="flex items-center gap-2">
              <span className="live-dot h-2.5 w-2.5 rounded-full bg-ok" />
              <h2 className="text-lg font-semibold">{report.gameName ? t("boost.activeFor", { game: report.gameName }) : t("boost.active")}</h2>
            </div>
            <p className="mt-1 text-[13px] text-ink-3">{t("boost.since", { time: timeAgo(lang, report.startedAt) })}</p>
          </div>
          <div className="flex gap-2">
            <Button onClick={() => setShowResult(true)}>{t("boost.showResult")}</Button>
            <Button variant="danger" icon={Undo2} onClick={() => void runUnboost()} loading={boosting}>
              {t("boost.stop")}
            </Button>
          </div>
        </div>
        <div className="grid grid-cols-3 gap-3">
          <div className="card-inset p-3">
            <div className="text-[22px] font-semibold tnum">{report.applied.length}</div>
            <div className="text-[12px] text-ink-3">{t("result.statTweaks")}</div>
          </div>
          <div className="card-inset p-3">
            <div className="text-[22px] font-semibold tnum">{report.closedProcesses.length}</div>
            <div className="text-[12px] text-ink-3">{t("result.statApps")}</div>
          </div>
          <div className="card-inset p-3">
            <div className="text-[22px] font-semibold tnum">{report.fpsAfter ? Math.round(report.fpsAfter.avgFps) : formatMb(lang, closedMb)}</div>
            <div className="text-[12px] text-ink-3">{report.fpsAfter ? t("result.statFps") : t("result.statMem")}</div>
          </div>
        </div>
        {report.gameRunning && report.applied.some((a) => a.timing === "game_restart") && report.gameName && (
          <p className="flex items-start gap-2 text-[13px] text-warn">
            <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
            {t("boost.gameRunning", { game: report.gameName })}
          </p>
        )}
      </Card>
    );
  }

  return (
    <Card className="flex flex-col gap-5">
      <div>
        <h2 className="text-lg font-semibold">{t("boost.title")}</h2>
        <p className="mt-1 text-[13px] text-ink-2">{t("boost.desc")}</p>
      </div>

      <div>
        <div className="mb-2 text-[13px] font-medium text-ink-3">{t("boost.forWhich")}</div>
        <div className="flex flex-wrap gap-2">
          <Chip active={target === null} onClick={() => setTarget(null)} icon={Monitor}>
            {t("boost.systemOnly")}
          </Chip>
          {sortedGames.map((g) => (
            <Chip key={g.id} active={target === g.id} onClick={() => setTarget(g.id)}>
              {runningGames.includes(g.id) && <span className="h-1.5 w-1.5 rounded-full bg-ok" />}
              {g.name}
            </Chip>
          ))}
        </div>
      </div>

      {targetRunning && targetGame && (
        <div className="space-y-2 rounded-lg border border-line bg-panel-2 p-3.5">
          <label className="flex cursor-pointer items-start gap-2.5">
            <input type="checkbox" className="mt-0.5 h-4 w-4" checked={measure} onChange={(e) => setMeasure(e.target.checked)} />
            <span>
              <span className="text-sm font-medium">{t("boost.measureFps")}</span>
              <span className="mt-0.5 block text-[12px] text-ink-3">{t("boost.measureFpsHint")}</span>
            </span>
          </label>
          <p className="flex items-start gap-2 text-[12px] text-warn">
            <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0" />
            {t("boost.gameRunning", { game: targetGame.name })}
          </p>
        </div>
      )}

      <div className="flex flex-wrap items-center gap-4">
        <Button variant="primary" size="lg" icon={Zap} loading={boosting} onClick={() => void runBoost(target, targetRunning && measure)} className="min-w-[180px]">
          {boosting ? (stage ? t(`boost.stage.${stage}` as never) : t("common.loading")) : t("boost.start")}
        </Button>
        <div className="text-[13px] text-ink-3">
          {t("boost.summaryTweaks", { count: globalCount + perGameCount })} · {t("boost.summaryApps", { count: closeCount })}
          {profile?.autoBoost && targetGame && <div className="text-accent">{t("boost.autoStarts", { game: targetGame.name })}</div>}
        </div>
      </div>
    </Card>
  );
}

function BigWinsTeaser() {
  const { t } = useT();
  const bigWins = useApp((s) => s.bigWins);
  const setPage = useApp((s) => s.setPage);
  const issues = (bigWins ?? []).filter((b) => b.status === "issue");
  return (
    <button onClick={() => setPage("tweaks")} className="card flex w-full items-center gap-3 p-4 text-left transition-colors hover:border-line-2">
      <span className={cn("flex h-9 w-9 shrink-0 items-center justify-center rounded-lg", issues.length ? "bg-accent-soft text-accent" : "bg-ok/10 text-ok")}>
        {issues.length ? <Sparkles className="h-4 w-4" /> : <ShieldCheck className="h-4 w-4" />}
      </span>
      <div className="min-w-0 flex-1">
        <div className="text-sm font-semibold">{t("tweaks.bigWins")}</div>
        <div className="text-[13px] text-ink-3">{issues.length ? t("dashboard.bigWins", { count: issues.length }) : t("dashboard.bigWinsAllGood")}</div>
      </div>
      <ChevronRight className="h-4 w-4 text-ink-3" />
    </button>
  );
}

function StatusCard() {
  const { t, lang } = useT();
  const appInfo = useApp((s) => s.appInfo);
  const settings = useApp((s) => s.settings);
  const restorePoint = useApp((s) => s.restorePoint);
  const relaunch = useApp((s) => s.relaunchElevated);
  const refreshRestorePoint = useApp((s) => s.refreshRestorePoint);
  const updateSettings = useApp((s) => s.updateSettings);
  const toast = useApp((s) => s.toast);
  const handleError = useApp((s) => s.handleError);
  const [creating, setCreating] = useState(false);
  const elevated = appInfo?.isElevated ?? false;
  const last = restorePoint?.lastResult ?? null;

  const createRp = async () => {
    setCreating(true);
    try {
      const r = await api.createRestorePoint();
      toast({ level: r.ok ? "success" : "warning", title: t(r.ok ? "notice.success" : "notice.warning"), message: t(`result.restore.${r.key}`) });
      if (r.ok) void updateSettings({ restorePointDone: true });
      void refreshRestorePoint();
    } catch (e) {
      handleError(e);
    } finally {
      setCreating(false);
    }
  };

  const restoreText = settings?.restorePointDone
    ? t("dashboard.restoreDone", { time: last ? timeAgo(lang, last.ts) : "" })
    : last && !last.ok
      ? t(`result.restore.${last.key}`)
      : settings?.restorePointPolicy === "never"
        ? t("dashboard.restoreNever")
        : t("dashboard.restoreAuto");

  return (
    <div className="card divide-y divide-line px-4">
      <div className="flex items-center gap-3 py-3.5">
        {elevated ? <ShieldCheck className="h-4 w-4 shrink-0 text-ok" /> : <ShieldAlert className="h-4 w-4 shrink-0 text-warn" />}
        <div className="min-w-0 flex-1">
          <div className="text-[12px] text-ink-3">{t("dashboard.rights")}</div>
          <div className="text-[13px]">{elevated ? t("dashboard.rightsAdmin") : t("dashboard.rightsLimited")}</div>
        </div>
        {!elevated && (
          <Button size="sm" onClick={() => void relaunch()}>
            {t("common.restartAdmin")}
          </Button>
        )}
      </div>
      <div className="flex items-center gap-3 py-3.5">
        <History className={cn("h-4 w-4 shrink-0", settings?.restorePointDone ? "text-ok" : "text-ink-3")} />
        <div className="min-w-0 flex-1">
          <div className="text-[12px] text-ink-3">{t("dashboard.restorePoint")}</div>
          <div className="text-[13px]">{restoreText}</div>
        </div>
        {!settings?.restorePointDone && (
          <Button size="sm" loading={creating} onClick={() => void createRp()}>
            {t("dashboard.createNow")}
          </Button>
        )}
      </div>
    </div>
  );
}

function GamesStrip() {
  const { t } = useT();
  const games = useApp((s) => s.games);
  const runningGames = useApp((s) => s.runningGames);
  const setPage = useApp((s) => s.setPage);
  const handleError = useApp((s) => s.handleError);
  if (games.length === 0) return null;
  return (
    <div>
      <SectionTitle
        right={
          <Button size="sm" variant="ghost" onClick={() => setPage("games")}>
            {t("dashboard.allGames")}
          </Button>
        }
      >
        {t("dashboard.games")}
      </SectionTitle>
      <div className="grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-5">
        {games.slice(0, 5).map((g) => (
          <div key={g.id} className="card group overflow-hidden">
            <div className="relative aspect-[460/215] overflow-hidden">
              <GameCover game={g} />
              {runningGames.includes(g.id) && (
                <span className="absolute left-2 top-2">
                  <Badge variant="ok" className="!bg-[#0f1115]/90">{t("games.playing")}</Badge>
                </span>
              )}
            </div>
            <div className="flex items-center justify-between gap-2 px-3 py-2.5">
              <div className="truncate text-[13px] font-medium">{g.name}</div>
              <button
                title={t("games.launch")}
                onClick={() => api.launchGame(g.id).catch(handleError)}
                className="rounded-md p-1.5 text-ink-3 hover:bg-panel-2 hover:text-ink"
              >
                <Play className="h-3.5 w-3.5" />
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

export function DashboardPage() {
  const { t, lang } = useT();
  const metrics = useApp((s) => s.metrics);
  const appInfo = useApp((s) => s.appInfo);
  const settings = useApp((s) => s.settings);
  const latest = metrics.latest;
  const h = metrics.history;
  const series = useMemo(
    () => ({
      cpu: h.map((s) => s.cpuPct),
      gpu: h.map((s) => s.gpuPct),
      ram: h.map((s) => s.ramUsedMb),
      temp: h.map((s) => s.gpuTempC ?? s.cpuTempC),
      ping: h.map((s) => s.pingMs),
    }),
    [h],
  );
  const temp = latest?.gpuTempC ?? latest?.cpuTempC ?? null;
  const tempSource = latest?.gpuTempC != null ? t("dashboard.tempGpu") : latest?.cpuTempC != null ? t("dashboard.tempCpu") : t("dashboard.tempNone");

  return (
    <div className="space-y-6">
      <PageHeader
        title={t("dashboard.title")}
        subtitle={
          <span className="inline-flex items-center gap-2">
            <span className="live-dot h-2 w-2 rounded-full bg-ok" />
            {t("dashboard.live")} · {appInfo?.cpuName} · {appInfo?.gpuName}
          </span>
        }
      />

      <div className="grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-5">
        <StatTile icon={Cpu} label={t("dashboard.cpu")} value={latest ? formatPct(lang, latest.cpuPct) : "-"} sub={appInfo?.cpuName} series={series.cpu} min={0} max={100} format={(v) => formatPct(lang, v, 1)} />
        <StatTile
          icon={Monitor}
          label={t("dashboard.gpu")}
          value={latest?.gpuPct != null ? formatPct(lang, latest.gpuPct) : "-"}
          sub={latest?.gpuMemUsedMb != null ? t("dashboard.vram", { used: formatMb(lang, latest.gpuMemUsedMb), total: formatMb(lang, latest.gpuMemTotalMb) }) : appInfo?.gpuName}
          series={series.gpu}
          min={0}
          max={100}
          format={(v) => formatPct(lang, v)}
        />
        <StatTile
          icon={MemoryStick}
          label={t("dashboard.ram")}
          value={latest ? formatMb(lang, latest.ramUsedMb) : "-"}
          sub={latest ? t("dashboard.ramFree", { free: formatMb(lang, latest.ramFreeMb), total: formatMb(lang, latest.ramTotalMb) }) : ""}
          series={series.ram}
          min={0}
          max={latest?.ramTotalMb}
          format={(v) => formatMb(lang, v)}
        />
        <StatTile icon={Thermometer} label={t("dashboard.temp")} value={formatTemp(lang, temp)} sub={tempSource} series={series.temp} min={20} max={100} format={(v) => formatTemp(lang, v)} />
        <StatTile
          icon={Activity}
          label={t("dashboard.ping")}
          value={latest?.pingMs != null ? formatMs(lang, latest.pingMs) : "-"}
          sub={t("dashboard.pingTo", { host: latest?.pingHost || settings?.dashboardPingHost || "" })}
          series={series.ping}
          min={0}
          format={(v) => formatMs(lang, v)}
        />
      </div>

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1.6fr)_minmax(320px,1fr)]">
        <BoostCard />
        <div className="space-y-4">
          <BigWinsTeaser />
          <StatusCard />
          <div className="flex items-center gap-2 px-1 text-[12px] text-ink-3">
            <Gauge className="h-3.5 w-3.5" />
            {t("welcome.safe")}
          </div>
        </div>
      </div>

      <GamesStrip />
    </div>
  );
}
