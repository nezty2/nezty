import { Cable, Network, ShieldQuestion, Star, Wifi } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useT } from "@/i18n/hooks";
import { api, listenEvent } from "@/lib/ipc";
import { formatMs } from "@/lib/format";
import type { MultiNetStatus, NetConnection } from "@/lib/types";
import { useApp } from "@/store/app";
import { Badge, Button, Card, Kv, Row, SectionTitle, Skeleton, Toggle, cn } from "../ui";
import { quality, qualityColor } from "./route";

/** Multi-Internet: elke verbinding apart meten, er één voorrang geven, en automatisch wisselen bij uitval. */
export function MultiTab() {
  const { t } = useT();
  const handleError = useApp((s) => s.handleError);
  const updateSettings = useApp((s) => s.updateSettings);
  const [status, setStatus] = useState<MultiNetStatus | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(() => {
    api.multinetStatus(true).then(setStatus).catch(() => {});
  }, []);
  useEffect(() => {
    load();
    const id = window.setInterval(load, 5000);
    let unlisten: (() => void) | null = null;
    void listenEvent("multinet-changed", load).then((u) => (unlisten = u));
    return () => {
      window.clearInterval(id);
      unlisten?.();
    };
  }, [load]);

  const act = async (key: string, fn: () => Promise<MultiNetStatus>) => {
    setBusy(key);
    try {
      setStatus(await fn());
      load();
    } catch (e) {
      handleError(e);
    } finally {
      setBusy(null);
    }
  };

  const conns = status?.connections ?? [];
  const hasOurs = conns.some((c) => c.preferredByNezty);
  return (
    <div className="space-y-6">
      <Card className="flex items-start gap-3">
        <ShieldQuestion className="mt-0.5 h-5 w-5 shrink-0 text-info" />
        <div className="space-y-1 text-[13px] text-ink-2">
          <div className="text-sm font-semibold text-ink">{t("netopt.multi.honestTitle")}</div>
          <p>{t("netopt.multi.honestWhat")}</p>
          <p>{t("netopt.multi.honestSwitch")}</p>
        </div>
      </Card>

      <section>
        <SectionTitle sub={t("netopt.multi.connectionsSub")}>{t("netopt.multi.connections")}</SectionTitle>
        {!status ? (
          <Skeleton className="h-32" />
        ) : conns.length === 0 ? (
          <Card className="text-[13px] text-ink-2">{t("netopt.multi.none")}</Card>
        ) : (
          <div className="grid gap-3 md:grid-cols-2">
            {conns.map((c) => (
              <ConnectionCard
                key={c.luid}
                c={c}
                single={conns.length < 2}
                busy={busy === c.luid}
                onPrefer={() => void act(c.luid, () => api.multinetPrefer(c.luid))}
                onClear={() => void act(c.luid, () => api.multinetClear())}
              />
            ))}
          </div>
        )}
        {status && conns.length === 1 && <p className="mt-3 text-[12px] text-ink-3">{t("netopt.multi.onlyOne")}</p>}
      </section>

      <Card className="!py-1">
        <Row title={t("netopt.multi.auto")} desc={conns.length < 2 ? t("netopt.multi.autoNeedsTwo") : t("netopt.multi.autoDesc")}>
          <Toggle
            checked={status?.auto ?? false}
            disabled={conns.length < 2 && !status?.auto}
            label={t("netopt.multi.auto")}
            onChange={async (v) => {
              await updateSettings({ multinetAuto: v });
              load();
            }}
          />
        </Row>
      </Card>
      {hasOurs && conns.length < 2 && (
        <Button size="sm" loading={busy === "clear"} onClick={() => void act("clear", () => api.multinetClear())}>
          {t("netopt.multi.clear")}
        </Button>
      )}
    </div>
  );
}

function ConnectionCard({ c, single, busy, onPrefer, onClear }: { c: NetConnection; single: boolean; busy: boolean; onPrefer: () => void; onClear: () => void }) {
  const { t, lang } = useT();
  const q = quality(c.pingMs);
  const Icon = c.kind === "wifi" ? Wifi : c.kind === "ethernet" ? Cable : Network;
  return (
    <Card className={cn(c.active && "border-accent/50")}>
      <div className="mb-1 flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 text-sm font-semibold">
          <Icon className="h-4 w-4 text-accent" />
          {c.name}
        </div>
        <div className="flex gap-1">
          {c.preferredByNezty && <Badge variant="accent" icon={Star}>{t("netopt.multi.preferred")}</Badge>}
          {c.active && <Badge variant="ok">{t("netopt.multi.active")}</Badge>}
        </div>
      </div>
      <div className="mb-2 truncate text-[12px] text-ink-3">{c.description}</div>
      <div className="divide-y divide-line">
        <Kv k={t("netopt.multi.ping")} v={<span className={cn("tnum", q ? qualityColor[q] : "text-ink-3")}>{c.pingMs != null ? formatMs(lang, c.pingMs) : t("network.noReply")}</span>} />
        <Kv k={t("netopt.multi.loss")} v={<span className="tnum">{c.lossPct != null ? `${Math.round(c.lossPct)}%` : "-"}</span>} />
        <Kv k={t("netopt.multi.metric")} v={<span className="tnum">{c.automaticMetric ? t("netopt.multi.metricAuto", { metric: c.metric }) : c.metric}</span>} />
      </div>
      {!single && (
        <div className="mt-3">
          {c.preferredByNezty ? (
            <Button size="sm" loading={busy} onClick={onClear}>
              {t("netopt.multi.clear")}
            </Button>
          ) : (
            <Button size="sm" variant="primary" icon={Star} loading={busy} onClick={onPrefer}>
              {t("netopt.multi.prefer")}
            </Button>
          )}
        </div>
      )}
    </Card>
  );
}
