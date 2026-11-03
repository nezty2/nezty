import { Cable, Eye, EyeOff, Globe, RefreshCw, ShieldQuestion, Wifi } from "lucide-react";
import { BandwidthCard } from "@/components/BandwidthCard";
import { BufferbloatCard } from "@/components/BufferbloatCard";
import { useCallback, useEffect, useState } from "react";
import { TweakCard } from "@/components/TweakCard";
import { Badge, Button, Card, Kv, PageHeader, SectionTitle, Skeleton, cn } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import { api } from "@/lib/ipc";
import { formatMs } from "@/lib/format";
import type { ConnectionOverview, PingResult, TweakInfo } from "@/lib/types";
import { useApp } from "@/store/app";

const INTERNET = [
  { id: "cloudflare", label: "Cloudflare", host: "1.1.1.1" },
  { id: "google", label: "Google", host: "8.8.8.8" },
];

type Quality = "excellent" | "good" | "ok" | "bad";
function quality(ms: number | null | undefined): Quality | null {
  if (ms == null) return null;
  return ms < 20 ? "excellent" : ms < 45 ? "good" : ms < 90 ? "ok" : "bad";
}
const qualityColor: Record<Quality, string> = { excellent: "text-ok", good: "text-ok", ok: "text-warn", bad: "text-bad" };

/** Klikbare meetkaart: klik om (opnieuw) te meten. */
function PingTile({ title, sub, result, loading, onTest }: { title: string; sub?: string; result?: PingResult; loading: boolean; onTest: () => void }) {
  const { t, lang } = useT();
  const q = quality(result?.avgMs);
  return (
    <button onClick={onTest} disabled={loading} className="card flex w-full flex-col p-4 text-left transition-colors hover:border-line-2 disabled:cursor-wait">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <div className="truncate text-sm font-semibold">{title}</div>
          {sub && <div className="truncate text-[12px] text-ink-3">{sub}</div>}
        </div>
        <RefreshCw className={cn("h-4 w-4 text-ink-3", loading && "animate-spin")} />
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
  const { t } = useT();
  return (
    <div className="space-y-6">
      <PageHeader title={t("network.title")} subtitle={t("network.subtitle")} />
      <OverviewTab />
    </div>
  );
}

/** Je internet, je verbinding, wie je internet gebruikt, bufferbloat en de netwerktweaks. */
function OverviewTab() {
  const { t } = useT();
  const handleError = useApp((s) => s.handleError);
  const toast = useApp((s) => s.toast);
  const tweaks = useApp((s) => s.tweaks);
  const setTweak = useApp((s) => s.setTweak);
  const appInfo = useApp((s) => s.appInfo);
  const [results, setResults] = useState<Record<string, PingResult>>({});
  const [loading, setLoading] = useState<Set<string>>(new Set());
  const [overview, setOverview] = useState<ConnectionOverview | null>(null);
  const [flushing, setFlushing] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  // IP-adres, router en DNS blijven verborgen tot je ze zelf toont (handig als je streamt).
  const [showDetails, setShowDetails] = useState(false);
  const hide = (v: string) => (showDetails ? v : "••••••");

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
  const testAll = useCallback(() => Promise.all(INTERNET.map((i) => test(i.id, i.host))), [test]);

  useEffect(() => {
    void testAll();
    api.getConnectionOverview().then(setOverview).catch(handleError);
  }, [handleError, testAll]);

  const netTweaks = tweaks.filter((tw) => tw.category === "network" && tw.scope === "global" && !tw.experimental);
  const onToggle = async (tw: TweakInfo, enabled: boolean, option: string | null) => {
    setBusy(tw.id);
    await setTweak(tw.id, enabled, option);
    setBusy(null);
  };
  const onWifi = overview?.adapters.some((a) => a.kind === "wifi" && a.status.toLowerCase() === "up") && !overview?.adapters.some((a) => a.kind === "ethernet" && a.status.toLowerCase() === "up");

  return (
    <div className="space-y-8">
      <div className="flex flex-wrap justify-end gap-2">
        <Button icon={RefreshCw} loading={loading.size > 0} onClick={() => void testAll()}>
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
      </div>

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

      <section>
        <SectionTitle sub={t("network.internetDesc")}>{t("network.internet")}</SectionTitle>
        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
          {INTERNET.map((i) => (
            <PingTile key={i.id} title={i.label} sub={i.host} result={results[i.id]} loading={loading.has(i.id)} onTest={() => void test(i.id, i.host)} />
          ))}
        </div>
      </section>

      <section>
        <SectionTitle
          right={
            <Button size="sm" variant="ghost" icon={showDetails ? EyeOff : Eye} onClick={() => setShowDetails((v) => !v)}>
              {showDetails ? t("network.hideDetails") : t("network.showDetails")}
            </Button>
          }
        >
          {t("network.connection")}
        </SectionTitle>
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
                    <Kv k={t("network.ip")} v={hide(a.ipv4 ?? "-")} mono />
                    <Kv k={t("network.gateway")} v={hide(a.gateway ?? "-")} mono />
                    <Kv k={t("network.dns")} v={hide(a.dns.length ? a.dns.join(", ") : "-")} mono />
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
