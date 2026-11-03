import { Cpu, Gauge, Info, MemoryStick, Radar, ShieldQuestion, Zap } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useT } from "@/i18n/hooks";
import { api, listenEvent } from "@/lib/ipc";
import { formatMb, formatMs, timeAgo } from "@/lib/format";
import type { Game, MemGuardStatus, NetRegion, PurgeResult, TweakInfo } from "@/lib/types";
import { useApp } from "@/store/app";
import { targetOf, useNetOpt } from "@/store/netopt";
import { Sparkline } from "../Sparkline";
import { Button, Card, Toggle, cn } from "../ui";
import { RouteLine, placeLabel, quality, qualityColor, useLiveMonitor } from "./route";

/** Games die via Valve-servers spelen (CS2, Dota 2, Deadlock): daar meten we naar de echte Valve-locaties. */
const VALVE_APPS = [730, 570, 1422450];
export function isValveGame(game: Game | null): boolean {
  return !!game && ((game.steamAppId != null && VALVE_APPS.includes(game.steamAppId)) || game.id === "steam:cs2");
}

/** Gekozen of draaiende game; anders de eerste. */
export function useSelectedGame(): Game | null {
  const games = useApp((s) => s.games);
  const runningGames = useApp((s) => s.runningGames);
  const gameId = useNetOpt((s) => s.gameId);
  return useMemo(() => games.find((g) => g.id === gameId) ?? games.find((g) => runningGames.includes(g.id)) ?? games[0] ?? null, [games, runningGames, gameId]);
}

function regionLabel(r: NetRegion): string {
  return `${r.city} · ${r.provider === "aws" ? "AWS" : "Valve"}`;
}

function Stat({ label, value, tone, sub }: { label: string; value: string; tone?: string; sub?: string }) {
  return (
    <div className="card-inset px-4 py-3">
      <div className="text-[11px] uppercase tracking-wide text-ink-3">{label}</div>
      <div className={cn("mt-1 text-[22px] font-semibold leading-none tnum", tone)}>{value}</div>
      {sub && <div className="mt-1 text-[11px] text-ink-3">{sub}</div>}
    </div>
  );
}

export function OptimizeTab() {
  const { t, td, lang } = useT();
  const games = useApp((s) => s.games);
  const runningGames = useApp((s) => s.runningGames);
  const handleError = useApp((s) => s.handleError);
  const setTweak = useApp((s) => s.setTweak);
  const settings = useApp((s) => s.settings);
  const updateSettings = useApp((s) => s.updateSettings);
  const { regionId, regions, server, route, routeHost, routeLoading, setGame, setRegion, setServer, loadRegions, runRoute } = useNetOpt();
  const game = useSelectedGame();
  const running = !!game && runningGames.includes(game.id);

  useEffect(() => {
    loadRegions().catch(handleError);
  }, [loadRegions, handleError]);

  // Draait de game: elke 3 seconden kijken met welke server hij praat.
  useEffect(() => {
    if (!game || !running) {
      setServer(null);
      return;
    }
    let alive = true;
    const tick = () =>
      api
        .gamenetServer(game.id)
        .then((s) => alive && setServer(s))
        .catch(() => {});
    void tick();
    const id = window.setInterval(tick, 3000);
    return () => {
      alive = false;
      window.clearInterval(id);
    };
  }, [game, running, setServer]);

  const target = targetOf(server, regions, regionId, isValveGame(game) ? "valve-ams" : "aws-eu-central");
  useEffect(() => {
    if (target) runRoute(target.host).catch(handleError);
    // Alleen bij een ander doel opnieuw meten.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [target?.host]);

  const routeFresh = route && routeHost === target?.host ? route : null;
  // Bij een datacenter meten we het datacenter zelf (TCP of ping). Bij de echte server: de server, of de laatste
  // router ervoor als de server geen ping beantwoordt.
  const liveHost = !target ? null : target.kind === "region" ? target.host : routeFresh ? (routeFresh.measureHost ?? null) : null;
  const live = useLiveMonitor(liveHost);
  const q = quality(live.stats?.avgMs);

  // Traffic Shaper: de tweak game_qos voor deze game.
  const [qos, setQos] = useState<TweakInfo | null>(null);
  const [qosBusy, setQosBusy] = useState(false);
  const loadQos = useCallback(() => {
    if (!game) return;
    api
      .getGameTweaks(game.id)
      .then((list) => setQos(list.find((x) => x.id === "game_qos") ?? null))
      .catch(handleError);
  }, [game, handleError]);
  useEffect(loadQos, [loadQos]);
  const setQosOn = async (v: boolean) => {
    if (!game) return;
    setQosBusy(true);
    await setTweak("game_qos", v, null, game.id);
    setQosBusy(false);
    loadQos();
  };
  const qosOn = qos?.state === "on";
  const guardOn = settings?.memGuard ?? false;
  const optimized = qosOn && guardOn;

  const serverLabel =
    target?.kind === "server" ? (placeLabel(server?.location) ?? server?.ip ?? t("netopt.route.server")) : target?.region ? regionLabel(target.region) : t("netopt.route.server");
  const serverNote = (() => {
    if (!game) return t("netopt.server.noGame");
    switch (server?.status) {
      case "found":
        return t("netopt.server.found", { ip: `${server.ip}:${server.port}`, pps: Math.round(server.packetsPerSec ?? 0) });
      case "listening":
        return t("netopt.server.listening", { game: game.name });
      case "idle":
        return t("netopt.server.idle", { game: game.name });
      case "needs_admin":
        return t("netopt.server.needsAdmin");
      case "error":
        return server.detail ? td(`msg.${server.detail.key}`, server.detail.params) : t("netopt.server.error");
      default:
        return t("netopt.server.notRunning", { game: game.name });
    }
  })();
  const grouped = useMemo(() => {
    const m = new Map<string, NetRegion[]>();
    for (const r of regions) m.set(r.group, [...(m.get(r.group) ?? []), r]);
    return Array.from(m.entries());
  }, [regions]);

  return (
    <div className="space-y-6">
      <Card className="space-y-5">
        <div className="flex flex-wrap items-end gap-3">
          <label className="min-w-[200px] flex-1">
            <span className="mb-1 block text-[12px] text-ink-3">{t("netopt.game")}</span>
            <select
              value={game?.id ?? ""}
              onChange={(e) => setGame(e.target.value || null)}
              className="h-10 w-full rounded-lg border border-line bg-panel-2 px-3 text-sm text-ink focus:border-accent focus:outline-none"
            >
              {games.map((g) => (
                <option key={g.id} value={g.id}>
                  {g.name}
                  {runningGames.includes(g.id) ? ` (${t("games.playing")})` : ""}
                </option>
              ))}
            </select>
          </label>
          <label className="min-w-[200px] flex-1">
            <span className="mb-1 block text-[12px] text-ink-3">{t("netopt.region")}</span>
            <select
              value={target?.kind === "server" ? "__server" : (target?.region?.id ?? "")}
              disabled={target?.kind === "server"}
              onChange={(e) => setRegion(e.target.value)}
              className="h-10 w-full rounded-lg border border-line bg-panel-2 px-3 text-sm text-ink focus:border-accent focus:outline-none disabled:opacity-70"
            >
              {target?.kind === "server" && <option value="__server">{t("netopt.realServer", { place: serverLabel })}</option>}
              {grouped.map(([group, list]) => (
                <optgroup key={group} label={td(`region.${group}`)}>
                  {list.map((r) => (
                    <option key={r.id} value={r.id}>
                      {regionLabel(r)}
                    </option>
                  ))}
                </optgroup>
              ))}
            </select>
          </label>
          <div className="flex items-center gap-3 rounded-lg border border-line px-4 py-2">
            <div>
              <div className="text-sm font-semibold">{t("netopt.optimize")}</div>
              <div className="text-[11px] text-ink-3">{optimized ? t("netopt.optimizedOn") : t("netopt.optimizedOff")}</div>
            </div>
            <Toggle
              checked={optimized}
              loading={qosBusy}
              disabled={!game || qos?.state === "unavailable"}
              label={t("netopt.optimize")}
              onChange={async (v) => {
                if (qosOn !== v) await setQosOn(v);
                if (guardOn !== v) void updateSettings({ memGuard: v });
              }}
            />
          </div>
        </div>

        <div className="flex items-start gap-2 text-[12px] text-ink-3">
          <Radar className="mt-0.5 h-3.5 w-3.5 shrink-0" />
          <span>{target?.kind === "region" && server?.status !== "found" ? `${serverNote} ${t("netopt.server.regionMeanwhile")}` : serverNote}</span>
        </div>

        <RouteLine route={routeFresh} serverLabel={serverLabel} liveMs={live.last} loading={routeLoading && !routeFresh} />
        {routeFresh && target?.kind === "server" && routeFresh.measureHost && routeFresh.measureHost !== routeFresh.ip && (
          <p className="text-[12px] text-ink-3">{t("netopt.lastHopNote", { host: routeFresh.measureHost })}</p>
        )}

        <div className="grid gap-3 sm:grid-cols-3">
          <Stat label={t("netopt.ping")} value={live.stats?.avgMs != null ? formatMs(lang, live.stats.avgMs) : "…"} tone={q ? qualityColor[q] : undefined} sub={q ? t(`network.quality.${q}`) : undefined} />
          <Stat label={t("netopt.jitter")} value={live.stats?.jitterMs != null ? formatMs(lang, live.stats.jitterMs) : "…"} tone={(live.stats?.jitterMs ?? 0) > 5 ? "text-warn" : undefined} sub={t("netopt.jitterSub")} />
          <Stat label={t("netopt.loss")} value={live.stats ? `${Math.round(live.stats.lossPct)}%` : "…"} tone={(live.stats?.lossPct ?? 0) >= 2 ? "text-bad" : undefined} sub={t("netopt.lossSub", { count: live.stats?.samples ?? 0 })} />
        </div>
        {/* Alleen ontvangen antwoorden: verlies staat al in de tegel hierboven. */}
        <Sparkline values={live.values.filter((v) => v != null)} min={0} height={44} className="h-12" format={(v) => formatMs(lang, v)} />
      </Card>

      <div className="grid gap-4 xl:grid-cols-2">
        <Card className="space-y-3">
          <div className="flex items-start justify-between gap-3">
            <div className="flex items-center gap-2 text-sm font-semibold">
              <Gauge className="h-4 w-4 text-accent" />
              {t("netopt.shaper.title")}
            </div>
            <Toggle checked={qosOn} loading={qosBusy} disabled={!game || qos?.state === "unavailable"} label={t("netopt.shaper.title")} onChange={(v) => void setQosOn(v)} />
          </div>
          <p className="text-[13px] text-ink-2">{t("netopt.shaper.what", { game: game?.name ?? "" })}</p>
          <ul className="space-y-1 text-[12px] text-ink-3">
            <li>{t("netopt.shaper.wifi")}</li>
            <li>{t("netopt.shaper.cable")}</li>
          </ul>
          {qos?.detail && <p className="text-[12px] text-warn">{td(`msg.${qos.detail.key}`, qos.detail.params)}</p>}
          {qosOn && qos?.active === "after_game_restart" && <p className="text-[12px] text-warn">{t("netopt.shaper.restartGame")}</p>}
        </Card>
        <MemoryCard />
      </div>

      <HonestCard />
    </div>
  );
}

/** Geheugenbewaker: standby-cache legen als het vrije geheugen opraakt, alleen tijdens een game. */
function MemoryCard() {
  const { t, lang } = useT();
  const handleError = useApp((s) => s.handleError);
  const updateSettings = useApp((s) => s.updateSettings);
  // Aan of uit komt uit de instellingen, net als bij de schakelaar Optimaliseren: dan lopen ze nooit uiteen.
  const enabled = useApp((s) => s.settings?.memGuard ?? false);
  const [status, setStatus] = useState<MemGuardStatus | null>(null);
  const [purging, setPurging] = useState(false);
  const [result, setResult] = useState<PurgeResult | null>(null);
  const load = useCallback(() => {
    api.memguardStatus().then(setStatus).catch(() => {});
  }, []);
  useEffect(() => {
    load();
    const id = window.setInterval(load, 5000);
    let unlisten: (() => void) | null = null;
    void listenEvent<MemGuardStatus>("memguard", setStatus).then((u) => (unlisten = u));
    return () => {
      window.clearInterval(id);
      unlisten?.();
    };
  }, [load]);

  const m = status?.stats;
  return (
    <Card className="space-y-3">
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-center gap-2 text-sm font-semibold">
          <MemoryStick className="h-4 w-4 text-accent" />
          {t("netopt.mem.title")}
        </div>
        <Toggle
          checked={enabled}
          label={t("netopt.mem.title")}
          onChange={async (v) => {
            await updateSettings({ memGuard: v });
            load();
          }}
        />
      </div>
      <p className="text-[13px] text-ink-2">{t("netopt.mem.what")}</p>
      {m && (
        <div className="card-inset grid grid-cols-2 gap-2 px-3 py-2.5 text-[12px]">
          <div>
            <div className="text-ink-3">{t("netopt.mem.free")}</div>
            <div className={cn("tnum font-medium", status?.needed ? "text-warn" : "text-ink")}>{formatMb(lang, m.freeMb)}</div>
          </div>
          <div>
            <div className="text-ink-3">{t("netopt.mem.standby")}</div>
            <div className="tnum font-medium">{formatMb(lang, m.standbyMb)}</div>
          </div>
        </div>
      )}
      <p className="text-[12px] text-ink-3">
        {status?.needed ? t("netopt.mem.needed") : t("netopt.mem.notNeeded", { free: formatMb(lang, m?.freeMb ?? 0) })}
        {status?.lastPurgeTs ? ` ${t("netopt.mem.last", { ago: timeAgo(lang, status.lastPurgeTs), mb: formatMb(lang, status.lastFreedMb ?? 0) })}` : ""}
      </p>
      <Button
        size="sm"
        icon={Cpu}
        loading={purging}
        onClick={async () => {
          setPurging(true);
          try {
            setResult(await api.memguardPurge());
            load();
          } catch (e) {
            handleError(e);
          } finally {
            setPurging(false);
          }
        }}
      >
        {t("netopt.mem.purge")}
      </Button>
      {result && <p className="text-[12px] text-ink-2">{t("netopt.mem.result", { before: formatMb(lang, result.before.freeMb), after: formatMb(lang, result.after.freeMb) })}</p>}
    </Card>
  );
}

/** Wat nezty bewust niet doet, met de reden. Zo weet je dat de rest echt werkt. */
function HonestCard() {
  const { t } = useT();
  return (
    <Card className="flex items-start gap-3">
      <ShieldQuestion className="mt-0.5 h-5 w-5 shrink-0 text-info" />
      <div className="space-y-1.5 text-[13px] text-ink-2">
        <div className="text-sm font-semibold text-ink">{t("netopt.honest.title")}</div>
        <p>
          <Zap className="mr-1 inline h-3.5 w-3.5 text-ink-3" />
          {t("netopt.honest.nagle")}
        </p>
        <p>
          <Info className="mr-1 inline h-3.5 w-3.5 text-ink-3" />
          {t("netopt.honest.mmcss")}
        </p>
        <p>
          <Info className="mr-1 inline h-3.5 w-3.5 text-ink-3" />
          {t("netopt.honest.priority")}
        </p>
        <p>
          <Info className="mr-1 inline h-3.5 w-3.5 text-ink-3" />
          {t("netopt.honest.routes")}
        </p>
      </div>
    </Card>
  );
}
