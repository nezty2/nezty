import { Award, CheckCircle2, Home, Info, RefreshCw, Server, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useT } from "@/i18n/hooks";
import { api } from "@/lib/ipc";
import { formatMs } from "@/lib/format";
import type { NetRegion, PingResult, RegionId, RouteReport } from "@/lib/types";
import { useApp } from "@/store/app";
import { targetOf, useNetOpt } from "@/store/netopt";
import { Badge, Button, Card, SectionTitle, Skeleton, cn } from "../ui";
import { isValveGame, useSelectedGame } from "./OptimizeTab";
import { RouteTable, quality, qualityColor } from "./route";

/** De conclusie van de routemeting: waar het begint, in gewone taal. */
function Conclusion({ route }: { route: RouteReport }) {
  const { t, lang } = useT();
  const s = route.summary;
  const Icon = s.kind === "clean" ? CheckCircle2 : s.kind === "home" ? Home : s.kind === "unknown" ? Info : TriangleAlert;
  const tone = s.kind === "clean" ? "border-ok/40" : s.kind === "unknown" ? "" : "border-warn/50";
  return (
    <Card className={cn("flex items-start gap-3", tone)}>
      <Icon className={cn("mt-0.5 h-5 w-5 shrink-0", s.kind === "clean" ? "text-ok" : s.kind === "unknown" ? "text-info" : "text-warn")} />
      <div className="space-y-1 text-[13px] text-ink-2">
        <div className="text-sm font-semibold text-ink">{t(`netopt.summary.${s.kind}.title`)}</div>
        <p>{t(`netopt.summary.${s.kind}.text`, { hop: s.hop ?? "" })}</p>
        {s.stepMs != null && s.stepTo != null && <p className="text-ink-3">{t("netopt.summary.step", { from: s.stepFrom ?? 0, to: s.stepTo, ms: formatMs(lang, s.stepMs) })}</p>}
        {!route.reached && route.hops.length > 0 && <p className="text-ink-3">{t("netopt.summary.serverSilent")}</p>}
      </div>
    </Card>
  );
}

export function AnalyzeTab() {
  const { t, td, lang } = useT();
  const handleError = useApp((s) => s.handleError);
  const { regionId, regions, server, route, routeHost, routeLoading, loadRegions, runRoute, setRegion } = useNetOpt();
  const game = useSelectedGame();
  const target = targetOf(server, regions, regionId, isValveGame(game) ? "valve-ams" : "aws-eu-central");

  useEffect(() => {
    loadRegions().catch(handleError);
  }, [loadRegions, handleError]);
  const fresh = route && routeHost === target?.host ? route : null;
  useEffect(() => {
    if (target && !fresh && !routeLoading) runRoute(target.host).catch(handleError);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [target?.host]);

  // Regio-scanner: alle datacenters in één spelregio meten, vier tegelijk.
  const [group, setGroup] = useState<RegionId>("eu");
  const [results, setResults] = useState<Record<string, PingResult>>({});
  const [scanning, setScanning] = useState(false);
  const inGroup = useMemo(() => regions.filter((r) => r.group === group), [regions, group]);
  const groups = useMemo(() => Array.from(new Set(regions.map((r) => r.group))), [regions]);
  const scan = useCallback(
    async (list: NetRegion[]) => {
      setScanning(true);
      try {
        for (let i = 0; i < list.length; i += 4) {
          const part = await Promise.all(list.slice(i, i + 4).map((r) => api.pingHost(r.host, 10).then((p) => [r.id, p] as const)));
          setResults((cur) => ({ ...cur, ...Object.fromEntries(part) }));
        }
      } catch (e) {
        handleError(e);
      } finally {
        setScanning(false);
      }
    },
    [handleError],
  );
  useEffect(() => {
    if (inGroup.length) void scan(inGroup);
  }, [inGroup, scan]);
  const sorted = [...inGroup].sort((a, b) => (results[a.id]?.avgMs ?? 9999) - (results[b.id]?.avgMs ?? 9999));
  const best = sorted.find((r) => results[r.id]?.avgMs != null);

  return (
    <div className="space-y-8">
      <section className="space-y-3">
        <SectionTitle
          sub={target ? t("netopt.analyze.target", { host: target.kind === "server" ? `${server?.ip ?? target.host}` : `${target.region?.city} (${target.host})` }) : undefined}
          right={
            <Button size="sm" icon={RefreshCw} loading={routeLoading} disabled={!target} onClick={() => target && runRoute(target.host).catch(handleError)}>
              {t("netopt.analyze.again")}
            </Button>
          }
        >
          {t("netopt.analyze.title")}
        </SectionTitle>
        {fresh ? (
          <>
            <Conclusion route={fresh} />
            <RouteTable route={fresh} />
            <p className="flex items-start gap-1.5 text-[12px] text-ink-3">
              <Info className="mt-0.5 h-3.5 w-3.5 shrink-0" />
              {t("netopt.analyze.howToRead")}
            </p>
          </>
        ) : (
          <Skeleton className="h-48" />
        )}
      </section>

      <section className="space-y-3">
        <SectionTitle
          sub={t("netopt.scan.sub")}
          right={
            <Button size="sm" icon={RefreshCw} loading={scanning} disabled={!inGroup.length} onClick={() => void scan(inGroup)}>
              {t("netopt.scan.again")}
            </Button>
          }
        >
          {t("netopt.scan.title")}
        </SectionTitle>
        <div className="flex flex-wrap gap-1.5">
          {groups.map((g) => (
            <button key={g} onClick={() => setGroup(g)} className={cn("rounded-full border px-3 py-1 text-[12px] transition-colors", g === group ? "border-accent bg-accent/10 text-ink" : "border-line text-ink-3 hover:text-ink-2")}>
              {td(`region.${g}`)}
            </button>
          ))}
        </div>
        {best && results[best.id]?.avgMs != null && (
          <Card className="flex items-start gap-3 border-accent">
            <Award className="mt-0.5 h-5 w-5 shrink-0 text-accent" />
            <div className="text-[13px] text-ink-2">
              <div className="text-sm font-semibold text-ink">{t("netopt.scan.best", { city: best.city, ms: formatMs(lang, results[best.id].avgMs) })}</div>
              {best.provider === "valve" ? (
                <p className="mt-0.5">{t("netopt.scan.valveHow")}</p>
              ) : (
                <>
                  <p className="mt-0.5">{t("network.fortniteHow")}</p>
                  <p className="mt-0.5 text-ink-3">{t("network.otherGamesHow")}</p>
                </>
              )}
            </div>
          </Card>
        )}
        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
          {sorted.map((r) => {
            const p = results[r.id];
            const q = quality(p?.avgMs);
            return (
              <button
                key={r.id}
                onClick={() => setRegion(r.id)}
                className={cn("card flex flex-col p-4 text-left transition-colors hover:border-line-2", regionId === r.id && "border-accent ring-1 ring-accent/40")}
                title={t("netopt.scan.pick")}
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <div className="truncate text-sm font-semibold">{r.city}</div>
                    <div className="truncate text-[12px] text-ink-3">{r.provider === "aws" ? "AWS" : "Valve"}</div>
                  </div>
                  {r.provider === "valve" ? <Badge variant="accent" icon={Server}>{t("netopt.scan.realServers")}</Badge> : null}
                </div>
                <div className={cn("mt-3 text-[22px] font-semibold leading-none tnum", q ? qualityColor[q] : "text-ink-3")}>{!p ? "…" : p.avgMs == null ? t("network.noReply") : formatMs(lang, p.avgMs)}</div>
                {p && p.avgMs != null && (
                  <div className="mt-1.5 text-[12px] text-ink-3">
                    {t("network.jitter", { ms: formatMs(lang, p.jitterMs) })} · {t("network.loss", { pct: Math.round(p.lossPct) })}
                  </div>
                )}
              </button>
            );
          })}
        </div>
        <p className="flex items-center gap-2 text-[12px] text-ink-3">
          <Info className="h-3.5 w-3.5" />
          {t("netopt.scan.note")}
        </p>
      </section>
    </div>
  );
}
