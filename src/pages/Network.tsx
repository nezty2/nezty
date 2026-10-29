import { Award, Cable, Globe, Info, RefreshCw, ShieldQuestion, Wifi } from "lucide-react";
import { BandwidthCard } from "@/components/BandwidthCard";
import { BufferbloatCard } from "@/components/BufferbloatCard";
import { useCallback, useEffect, useMemo, useState } from "react";
import { TweakCard } from "@/components/TweakCard";
import { Badge, Button, Card, Kv, PageHeader, SectionTitle, Skeleton, cn } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import { api } from "@/lib/ipc";
import { formatMs } from "@/lib/format";
import type { ConnectionOverview, GameRegion, PingResult, RegionId, TweakInfo } from "@/lib/types";
import { useApp } from "@/store/app";

type Quality = "excellent" | "good" | "ok" | "bad";
function quality(ms: number | null | undefined): Quality | null {
  if (ms == null) return null;
  return ms < 20 ? "excellent" : ms < 45 ? "good" : ms < 90 ? "ok" : "bad";
}
const qualityColor: Record<Quality, string> = { excellent: "text-ok", good: "text-ok", ok: "text-warn", bad: "text-bad" };

const INTERNET = [
  { id: "cloudflare", label: "Cloudflare", host: "1.1.1.1" },
  { id: "google", label: "Google", host: "8.8.8.8" },
];

/** Klikbare meetkaart: klik om (opnieuw) te meten. */
function PingTile({ title, sub, result, loading, recommended, onTest }: { title: string; sub?: string; result?: PingResult; loading: boolean; recommended?: boolean; onTest: () => void }) {
  const { t, lang } = useT();
  const q = quality(result?.avgMs);
  return (
    <button
      onClick={onTest}
      disabled={loading}
      className={cn("card flex w-full flex-col p-4 text-left transition-colors hover:border-line-2 disabled:cursor-wait", recommended && "border-accent ring-1 ring-accent/40")}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="truncate text-sm font-semibold">{title}</div>
          {sub && <div className="truncate text-[12px] text-ink-3">{sub}</div>}
        </div>
        {recommended ? <Badge variant="accent" icon={Award}>{t("common.recommended")}</Badge> : <RefreshCw className={cn("h-4 w-4 text-ink-3", loading && "animate-spin")} />}
      </div>
      <div className="mt-3 flex items-end justify-between gap-2">
        <div className={cn("text-[24px] font-semibold leading-none tnum", q ? qualityColor[q] : "text-ink-3")}>
          {loading && !result ? "…" : result ? (result.avgMs == null ? t("network.noReply") : formatMs(lang, result.avgMs)) : "-"}
        </div>
        {q && <span className={cn("text-[12px] font-medium", qualityColor[q])}>{t(`network.quality.${q}`)}</span>}
      </div>
      {result && result.avgMs != null && (
        <div className="mt-1.5 text-[12px] text-ink-3">
          {t("network.jitter", { ms: formatMs(lang, result.jitterMs) })}
          {` · ${t("network.loss", { pct: Math.round(result.lossPct) })}`}
        </div>
      )}
    </button>
  );
}

export function NetworkPage() {
  const { t, td, lang } = useT();
  const handleError = useApp((s) => s.handleError);
  const toast = useApp((s) => s.toast);
  const tweaks = useApp((s) => s.tweaks);
  const setTweak = useApp((s) => s.setTweak);
  const appInfo = useApp((s) => s.appInfo);
  const [regions, setRegions] = useState<GameRegion[]>([]);
  const [results, setResults] = useState<Record<string, PingResult>>({});
  const [loading, setLoading] = useState<Set<string>>(new Set());
  const [overview, setOverview] = useState<ConnectionOverview | null>(null);
  const [flushing, setFlushing] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);

  const test = useCallback(
    async (id: string, host: string) => {
      setLoading((s) => new Set(s).add(id));
      try {
        const r = await api.pingHost(host, 10);
        setResults((cur) => ({ ...cur, [id]: r }));
      } catch (e) {
        handleError(e);
      } finally {
        setLoading((s) => {
          const n = new Set(s);
          n.delete(id);
          return n;
        });
      }
    },
    [handleError],
  );

  const testAll = useCallback(
    async (list: GameRegion[]) => {
      // Een paar tegelijk, zodat het snel gaat zonder de verbinding te belasten.
      const jobs = [...INTERNET.map((i) => ({ id: i.id, host: i.host })), ...list.map((r) => ({ id: r.id, host: r.host }))];
      for (let i = 0; i < jobs.length; i += 4) {
        await Promise.all(jobs.slice(i, i + 4).map((j) => test(j.id, j.host)));
      }
    },
    [test],
  );

  useEffect(() => {
    api
      .getGameRegions()
      .then((r) => {
        setRegions(r);
        void testAll(r);
      })
      .catch(handleError);
    api.getConnectionOverview().then(setOverview).catch(handleError);
  }, [handleError, testAll]);

  // Beste regio: laagste gemiddelde ping. Per spelregio de snelste stad.
  const best = useMemo(() => {
    let b: { region: GameRegion; ms: number } | null = null;
    for (const r of regions) {
      const ms = results[r.id]?.avgMs;
      if (ms != null && (!b || ms < b.ms)) b = { region: r, ms };
    }
    return b;
  }, [regions, results]);

  const byRegion = useMemo(() => {
    const m = new Map<RegionId, GameRegion[]>();
    for (const r of regions) m.set(r.region, [...(m.get(r.region) ?? []), r]);
    return Array.from(m.entries());
  }, [regions]);

  const netTweaks = tweaks.filter((tw) => tw.category === "network" && !tw.experimental);
  const onToggle = async (tw: TweakInfo, enabled: boolean, option: string | null) => {
    setBusy(tw.id);
    await setTweak(tw.id, enabled, option);
    setBusy(null);
  };
  const onWifi = overview?.adapters.some((a) => a.kind === "wifi" && a.status.toLowerCase() === "up") && !overview?.adapters.some((a) => a.kind === "ethernet" && a.status.toLowerCase() === "up");

  return (
    <div className="space-y-8">
      <PageHeader
        title={t("network.title")}
        subtitle={t("network.subtitle")}
        actions={
          <>
            <Button icon={RefreshCw} loading={loading.size > 0} onClick={() => void testAll(regions)}>
              {loading.size > 0 ? t("network.testing") : t("network.testAll")}
            </Button>
            <Button
              icon={Globe}
              loading={flushing}
              onClick={async () => {
                setFlushing(true);
                try {
                  await api.flushDns();
                  toast({ level: "success", title: t("notice.success"), message: t("network.flushed") });
                } catch (e) {
                  handleError(e);
                } finally {
                  setFlushing(false);
                }
              }}
            >
              {t("network.flushDns")}
            </Button>
          </>
        }
      />

      {overview && (
        <Card className={cn("flex items-start gap-3", onWifi ? "border-warn/50" : "border-ok/40")}>
          {onWifi ? <Wifi className="mt-0.5 h-5 w-5 shrink-0 text-warn" /> : <Cable className="mt-0.5 h-5 w-5 shrink-0 text-ok" />}
          <div>
            <div className="text-sm font-semibold">{onWifi ? t("network.onWifiTitle") : t("network.onCableTitle")}</div>
            <p className="mt-0.5 text-[13px] text-ink-2">{onWifi ? t("network.onWifiText") : t("network.onCableText")}</p>
          </div>
        </Card>
      )}

      <Card className="flex items-start gap-3">
        <ShieldQuestion className="mt-0.5 h-5 w-5 shrink-0 text-info" />
        <div className="space-y-1 text-[13px] text-ink-2">
          <div className="text-sm font-semibold text-ink">{t("network.honestTitle")}</div>
          <p>{t("network.honestPing")}</p>
          <p>{t("network.honestDns")}</p>
          <p>{t("network.honestHelps")}</p>
        </div>
      </Card>

      {best && (
        <Card className="flex items-start gap-4 border-accent">
          <Award className="mt-0.5 h-6 w-6 shrink-0 text-accent" />
          <div>
            <div className="text-[15px] font-semibold">{t("network.bestTitle", { region: td(`region.${best.region.region}`) })}</div>
            <p className="mt-1 text-[13px] text-ink-2">{t("network.bestAdvice", { region: td(`region.${best.region.region}`), ms: formatMs(lang, best.ms) })}</p>
            <p className="mt-1 text-[12px] text-ink-3">{t("network.fortniteHow")}</p>
            <p className="mt-0.5 text-[12px] text-ink-3">{t("network.otherGamesHow")}</p>
          </div>
        </Card>
      )}

      <section>
        <SectionTitle sub={t("network.internetDesc")}>{t("network.internet")}</SectionTitle>
        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
          {INTERNET.map((i) => (
            <PingTile key={i.id} title={i.label} sub={i.host} result={results[i.id]} loading={loading.has(i.id)} onTest={() => void test(i.id, i.host)} />
          ))}
        </div>
      </section>

      <section>
        <SectionTitle sub={t("network.regionsDesc")}>{t("network.regions")}</SectionTitle>
        {regions.length === 0 ? (
          <Skeleton className="h-32" />
        ) : (
          <div className="space-y-4">
            {byRegion.map(([region, list]) => (
              <div key={region}>
                <div className="mb-2 text-[13px] font-medium text-ink-2">{td(`region.${region}`)}</div>
                <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
                  {list.map((r) => (
                    <PingTile key={r.id} title={r.city} result={results[r.id]} loading={loading.has(r.id)} recommended={best?.region.id === r.id} onTest={() => void test(r.id, r.host)} />
                  ))}
                </div>
              </div>
            ))}
          </div>
        )}
        <p className="mt-3 flex items-center gap-2 text-[12px] text-ink-3">
          <Info className="h-3.5 w-3.5" />
          {t("network.indicative")}
        </p>
      </section>

      <section>
        <SectionTitle>{t("network.connection")}</SectionTitle>
        {onWifi && (
          <div className="mb-3 flex items-start gap-2 rounded-lg border border-warn/30 bg-warn/10 px-3 py-2.5 text-[13px] text-warn">
            <Wifi className="mt-0.5 h-4 w-4 shrink-0" />
            {t("network.wifiTip")}
          </div>
        )}
        {overview === null ? (
          <Skeleton className="h-32" />
        ) : (
          <div className="grid gap-3 md:grid-cols-2">
            {overview.adapters.map((a) => {
              const up = a.status.toLowerCase() === "up";
              return (
                <Card key={a.name} className={cn(!up && "opacity-60")}>
                  <div className="mb-1 flex items-center justify-between gap-2">
                    <div className="flex items-center gap-2 text-sm font-semibold">
                      {a.kind === "wifi" ? <Wifi className="h-4 w-4 text-accent" /> : <Cable className="h-4 w-4 text-accent" />}
                      {a.kind === "wifi" ? t("network.wifi") : a.kind === "ethernet" ? t("network.ethernet") : t("network.other")}
                    </div>
                    <Badge variant={up ? "ok" : "neutral"}>{up ? t("network.connected") : t("network.disconnected")}</Badge>
                  </div>
                  <div className="mb-2 truncate text-[12px] text-ink-3">{a.description}</div>
                  <div className="divide-y divide-line">
                    <Kv k={t("network.speed")} v={a.linkSpeedMbps ? (a.linkSpeedMbps >= 1000 ? `${a.linkSpeedMbps / 1000} Gbit/s` : `${a.linkSpeedMbps} Mbit/s`) : "-"} />
                    <Kv k={t("network.ip")} v={a.ipv4 ?? "-"} mono />
                    <Kv k={t("network.gateway")} v={a.gateway ?? "-"} mono />
                    <Kv k={t("network.dns")} v={a.dns.length ? a.dns.join(", ") : "-"} mono />
                  </div>
                </Card>
              );
            })}
          </div>
        )}
      </section>

      <BandwidthCard />

      <BufferbloatCard />

      <section>
        <SectionTitle>{t("network.tweaks")}</SectionTitle>
        <div className="grid gap-4 lg:grid-cols-2">
          {netTweaks.map((tw) => (
            <TweakCard key={tw.id} tweak={tw} elevated={appInfo?.isElevated ?? false} busy={busy === tw.id} onToggle={(v, opt) => void onToggle(tw, v, opt)} />
          ))}
        </div>
      </section>
    </div>
  );
}
