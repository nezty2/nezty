import { AlertTriangle, Mouse, Play } from "lucide-react";
import { useState } from "react";
import { useT } from "@/i18n/hooks";
import { api } from "@/lib/ipc";
import type { MouseRate } from "@/lib/types";
import { useApp } from "@/store/app";
import { Button, Card, cn } from "./ui";

/** Boven deze meting krijgt Fortnite het advies om lager te gaan (Epic waarschuwt zelf voor 8000 Hz). */
const HIGH_HZ = 6000;

/** Meet hoe vaak je muis per seconde zijn positie doorgeeft. Alleen meten, niets aanpassen. */
export function MouseRateCard() {
  const { t } = useT();
  const games = useApp((s) => s.games);
  const handleError = useApp((s) => s.handleError);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<MouseRate | null>(null);
  const fortnite = games.some((g) => g.id === "epic:fortnite");

  const measure = async () => {
    setBusy(true);
    setResult(null);
    try {
      setResult(await api.measureMouseRate(3));
    } catch (e) {
      handleError(e);
    } finally {
      setBusy(false);
    }
  };

  const high = result?.hz != null && result.hz >= HIGH_HZ;
  return (
    <Card>
      <div className="flex items-start gap-3">
        <Mouse className="mt-0.5 h-5 w-5 shrink-0 text-accent" />
        <div className="min-w-0 flex-1">
          <div className="text-sm font-semibold">{t("mouseRate.title")}</div>
          <p className="mt-1 text-[13px] text-ink-2">{t("mouseRate.what")}</p>
          <div className="mt-3 flex flex-wrap items-center gap-3">
            <Button size="sm" variant="primary" icon={Play} loading={busy} onClick={() => void measure()}>
              {busy ? t("mouseRate.moving") : t("mouseRate.measure")}
            </Button>
            {result && (
              <span className={cn("text-[15px] font-semibold tnum", high ? "text-warn" : "text-ok")}>
                {result.hz != null ? t("mouseRate.result", { hz: result.hz }) : t("mouseRate.tooFew")}
              </span>
            )}
          </div>
          {result?.hz != null && (
            <p className="mt-2 text-[12px] text-ink-3">{high && fortnite ? "" : result.hz >= HIGH_HZ ? t("mouseRate.highOther") : t("mouseRate.fine")}</p>
          )}
          {high && fortnite && (
            <p className="mt-2 flex items-start gap-2 rounded-lg border border-warn/30 bg-warn/10 px-3 py-2.5 text-[13px] text-warn">
              <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
              {t("mouseRate.fortnite", { hz: result!.hz! })}
            </p>
          )}
          <p className="mt-2 text-[12px] text-ink-3">{t("mouseRate.note")}</p>
        </div>
      </div>
    </Card>
  );
}
