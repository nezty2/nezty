import { Gauge } from "lucide-react";
import { useEffect, useState } from "react";
import { useT } from "@/i18n/hooks";
import type { TKey } from "@/i18n";
import { api, listenEvent } from "@/lib/ipc";
import type { BufferbloatResult } from "@/lib/types";
import { useApp } from "@/store/app";
import { Button, Card, SectionTitle, cn } from "./ui";

const gradeColor: Record<string, string> = { "A+": "text-ok", A: "text-ok", B: "text-warn", C: "text-warn", D: "text-bad", F: "text-bad" };

/** Ping onder belasting: stijgt je ping als je verbinding vol zit? */
export function BufferbloatCard() {
  const { t } = useT();
  const handleError = useApp((s) => s.handleError);
  const [result, setResult] = useState<BufferbloatResult | null>(null);
  const [phase, setPhase] = useState<string | null>(null);

  useEffect(() => {
    let off: (() => void) | undefined;
    void listenEvent<string>("bufferbloat-phase", (p) => setPhase(p === "done" ? null : p)).then((u) => (off = u));
    return () => off?.();
  }, []);

  const run = async () => {
    setPhase("idle");
    try {
      setResult(await api.runBufferbloat());
    } catch (e) {
      handleError(e);
    } finally {
      setPhase(null);
    }
  };

  const ms = (v: number | null) => (v == null ? "-" : `${Math.round(v)} ms`);
  const mbps = (v: number | null) => (v == null ? "" : ` (${Math.round(v)} Mbit/s)`);
  const good = result ? result.grade === "A+" || result.grade === "A" : false;

  return (
    <section>
      <SectionTitle sub={t("bloat.sub")}>{t("bloat.title")}</SectionTitle>
      <Card>
        <div className="flex flex-wrap items-center gap-3">
          <Button icon={Gauge} loading={phase !== null} onClick={() => void run()}>
            {phase ? t(`bloat.phase.${phase}` as TKey) : result ? t("bloat.again") : t("bloat.start")}
          </Button>
          <span className="text-[12px] text-ink-3">{t("bloat.data")}</span>
        </div>
        {result && (
          <div className="mt-4 flex flex-wrap items-start gap-6">
            <div className={cn("text-[40px] font-semibold leading-none", gradeColor[result.grade] ?? "text-ink")}>{result.grade}</div>
            <div className="min-w-0 flex-1 space-y-1 text-[13px]">
              <div>{t("bloat.idle", { ms: ms(result.idleMs) })}</div>
              <div>{t("bloat.down", { ms: ms(result.downloadMs) })}{mbps(result.downloadMbps)}</div>
              <div>{t("bloat.up", { ms: ms(result.uploadMs) })}{mbps(result.uploadMbps)}</div>
              <p className={cn("pt-1", good ? "text-ok" : "text-warn")}>{good ? t("bloat.good", { ms: ms(result.increaseMs) }) : t("bloat.bad", { ms: ms(result.increaseMs) })}</p>
              {!good && <p className="text-ink-2">{t("bloat.fix")}</p>}
            </div>
          </div>
        )}
      </Card>
    </section>
  );
}
