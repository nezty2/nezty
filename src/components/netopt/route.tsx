import { Gamepad2, Home, Monitor, Server } from "lucide-react";
import { Fragment, useEffect, useState } from "react";
import { useT } from "@/i18n/hooks";
import { api, listenEvent } from "@/lib/ipc";
import { formatMs } from "@/lib/format";
import type { LiveSample, LiveStats, RouteHop, RouteReport, ServerLocation } from "@/lib/types";
import { Badge, cn } from "../ui";

export type Quality = "excellent" | "good" | "ok" | "bad";
export function quality(ms: number | null | undefined): Quality | null {
  if (ms == null) return null;
  return ms < 20 ? "excellent" : ms < 45 ? "good" : ms < 90 ? "ok" : "bad";
}
export const qualityColor: Record<Quality, string> = { excellent: "text-ok", good: "text-ok", ok: "text-warn", bad: "text-bad" };

/** Kort label voor een tussenstap: het domein uit de naam ("kpn.net"), anders het adres. */
export function hopLabel(h: RouteHop): string {
  if (h.hostName) {
    const parts = h.hostName.split(".").filter(Boolean);
    if (parts.length >= 2) return parts.slice(-2).join(".");
    return h.hostName;
  }
  return h.ip ?? "?";
}

/** "Frankfurt · AWS" voor een serverlocatie. */
export function placeLabel(loc: ServerLocation | null | undefined): string | null {
  if (!loc) return null;
  const provider = loc.provider === "aws" ? "AWS" : loc.provider === "gcp" ? "Google Cloud" : "Valve";
  return `${loc.city ?? loc.region} · ${provider}`;
}

/** Live meting (elke halve seconde) naar `host`, zolang het component er is. */
export function useLiveMonitor(host: string | null) {
  const [values, setValues] = useState<(number | null)[]>([]);
  const [stats, setStats] = useState<LiveStats | null>(null);
  const [last, setLast] = useState<number | null>(null);
  useEffect(() => {
    setValues([]);
    setStats(null);
    setLast(null);
    if (!host) return;
    let unlisten: (() => void) | null = null;
    let stopped = false;
    void listenEvent<LiveSample>("netmon-sample", (s) => {
      if (s.host !== host) return;
      setValues((v) => [...v.slice(-59), s.ms]);
      setStats(s.stats);
      setLast(s.ms);
    }).then((u) => {
      if (stopped) u();
      else unlisten = u;
    });
    api.gamenetMonitorStart(host).catch(() => {});
    return () => {
      stopped = true;
      unlisten?.();
      void api.gamenetMonitorStop(host).catch(() => {});
    };
  }, [host]);
  return { values, stats, last };
}

type Node = { kind: "pc" } | { kind: "hop"; hop: RouteHop } | { kind: "silent"; count: number } | { kind: "unknown" } | { kind: "server"; hop: RouteHop | null };

function nodesOf(route: RouteReport | null): Node[] {
  const out: Node[] = [{ kind: "pc" }];
  if (!route) return [...out, { kind: "server", hop: null }];
  for (const h of route.hops) {
    if (h.isTarget) continue;
    if (h.verdict === "silent") {
      const prev = out[out.length - 1];
      if (prev.kind === "silent") prev.count += 1;
      else out.push({ kind: "silent", count: 1 });
    } else {
      out.push({ kind: "hop", hop: h });
    }
  }
  // De server zelf zweeg: hoeveel routers er nog tussen zitten, is dan niet te zien.
  if (!route.reached && route.hops.length) out.push({ kind: "unknown" });
  out.push({ kind: "server", hop: route.hops.find((h) => h.isTarget) ?? null });
  return out;
}

/**
 * De route als lijn, zoals in ExitLag: jouw pc, de tussenstappen en de server. Stille routers (die geen ping
 * beantwoorden, heel normaal) worden samengevoegd. `liveMs` is de live ping naar de server.
 */
export function RouteLine({ route, serverLabel, liveMs, loading }: { route: RouteReport | null; serverLabel: string; liveMs: number | null; loading: boolean }) {
  const { t, lang } = useT();
  const nodes = nodesOf(route);
  const dot = (h: RouteHop) => (h.verdict === "loss" ? "bg-bad" : h.verdict === "icmp_limited" ? "bg-ink-3" : "bg-ok");
  return (
    <div className="overflow-x-auto pb-1">
      {/* Verbindingslijnen rekken mee; past de route niet, dan schuift alleen deze balk. */}
      <div className={cn("flex w-full min-w-max items-center gap-1.5", loading && "animate-pulse")}>
        {nodes.map((n, i) => (
          <Fragment key={i}>
            {i > 0 && <div className="h-px min-w-3 flex-1 bg-line-2" />}
            {n.kind === "pc" && (
              <div className="flex items-center gap-2 rounded-lg border border-line bg-panel-2 px-2.5 py-2">
                <Monitor className="h-4 w-4 text-accent" />
                <span className="text-[13px] font-medium">{t("netopt.route.pc")}</span>
              </div>
            )}
            {n.kind === "hop" && (
              <div className="rounded-lg border border-line px-2.5 py-1.5" title={[n.hop.hostName, n.hop.ip].filter(Boolean).join("\n")}>
                <div className="flex items-center gap-1.5 text-[12px] font-medium">
                  <span className={cn("h-1.5 w-1.5 rounded-full", dot(n.hop))} />
                  {n.hop.ttl === 1 ? (
                    <span className="flex items-center gap-1">
                      <Home className="h-3 w-3 text-ink-3" />
                      {t("netopt.route.router")}
                    </span>
                  ) : n.hop.private ? (
                    t("netopt.route.provider")
                  ) : (
                    hopLabel(n.hop)
                  )}
                </div>
                <div className="tnum text-[11px] text-ink-3">{n.hop.avgMs != null ? formatMs(lang, n.hop.avgMs) : "-"}</div>
              </div>
            )}
            {n.kind === "silent" && (
              <span className="shrink-0 rounded-full border border-dashed border-line px-2 py-1 text-[11px] text-ink-3" title={t("netopt.route.silentHint")}>
                {t("netopt.route.silent", { count: n.count })}
              </span>
            )}
            {n.kind === "unknown" && (
              <span className="shrink-0 rounded-full border border-dashed border-line px-2 py-1 text-[11px] text-ink-3" title={t("netopt.route.silentHint")}>
                {t("netopt.route.unknownHops")}
              </span>
            )}
            {n.kind === "server" && (
              <div className="flex items-center gap-2 rounded-lg border border-accent/50 bg-accent/10 px-2.5 py-2">
                <Server className="h-4 w-4 text-accent" />
                <div>
                  <div className="flex items-center gap-1.5 text-[13px] font-medium">
                    <Gamepad2 className="h-3.5 w-3.5 text-ink-3" />
                    {serverLabel}
                  </div>
                  <div className={cn("tnum text-[11px]", liveMs != null ? qualityColor[quality(liveMs)!] : "text-ink-3")}>
                    {liveMs != null ? formatMs(lang, liveMs) : n.hop && !n.hop.received ? t("netopt.route.noPing") : "-"}
                  </div>
                </div>
              </div>
            )}
          </Fragment>
        ))}
      </div>
    </div>
  );
}

/** Elke stap als rij, zoals pathping: ping, verlies en het oordeel. */
export function RouteTable({ route }: { route: RouteReport }) {
  const { t, lang } = useT();
  const verdict: Record<RouteHop["verdict"], { variant: "ok" | "neutral" | "bad"; key: "ok" | "silent" | "limited" | "loss" }> = {
    ok: { variant: "ok", key: "ok" },
    silent: { variant: "neutral", key: "silent" },
    icmp_limited: { variant: "neutral", key: "limited" },
    loss: { variant: "bad", key: "loss" },
  };
  return (
    <div className="card-inset overflow-x-auto">
      <table className="w-full min-w-[560px] text-left text-[13px]">
        <thead className="text-[11px] uppercase tracking-wide text-ink-3">
          <tr className="border-b border-line">
            <th className="px-3 py-2 font-medium">#</th>
            <th className="px-3 py-2 font-medium">{t("netopt.table.hop")}</th>
            <th className="px-3 py-2 text-right font-medium">{t("netopt.table.ping")}</th>
            <th className="px-3 py-2 text-right font-medium">{t("netopt.table.loss")}</th>
            <th className="px-3 py-2 font-medium">{t("netopt.table.verdict")}</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-line">
          {route.hops.map((h) => (
            <tr key={h.ttl} className={cn(h.isTarget && "bg-accent/5")}>
              <td className="tnum px-3 py-2 text-ink-3">{h.isTarget && !h.received ? "?" : h.ttl}</td>
              <td className="px-3 py-2">
                <div className="font-medium">{h.isTarget ? t("netopt.table.server") : h.ttl === 1 ? t("netopt.route.router") : h.hostName ?? h.ip ?? t("netopt.table.unknown")}</div>
                {h.ip && <div className="selectable font-mono text-[11px] text-ink-3">{h.ip}</div>}
              </td>
              <td className="tnum px-3 py-2 text-right">{h.avgMs != null ? formatMs(lang, h.avgMs) : "-"}</td>
              <td className={cn("tnum px-3 py-2 text-right", h.verdict === "loss" && "text-bad")}>{h.received ? `${Math.round(h.lossPct)}%` : "-"}</td>
              <td className="px-3 py-2">
                <Badge variant={verdict[h.verdict].variant}>{t(`netopt.verdict.${verdict[h.verdict].key}`)}</Badge>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
