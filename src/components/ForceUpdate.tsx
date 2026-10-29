import { Download, RotateCcw } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useT } from "@/i18n/hooks";
import { api } from "@/lib/ipc";
import { useApp } from "@/store/app";
import { useUpdate } from "@/store/update";
import { Button } from "./ui";

/**
 * Verplichte update: zodra er een nieuwe versie is, blokkeert dit scherm de app en begint het bijwerken
 * meteen. "Alles terugzetten" blijft hier bereikbaar, zodat gebruikers hun pc altijd kunnen terugzetten.
 */
export function ForceUpdate() {
  const { t } = useT();
  const { status, info, progress, install } = useUpdate();
  const handleError = useApp((s) => s.handleError);
  const [reverting, setReverting] = useState(false);
  const [reverted, setReverted] = useState<number | null>(null);
  const started = useRef<string | null>(null);

  // Meteen beginnen, één keer per versie. Mislukt het, dan kan de gebruiker het opnieuw proberen.
  useEffect(() => {
    if (status === "available" && info && started.current !== info.version) {
      started.current = info.version;
      void install();
    }
  }, [status, info, install]);

  if (!info || !(status === "available" || status === "installing" || status === "installError")) return null;

  const revert = async () => {
    setReverting(true);
    try {
      setReverted((await api.revertAll()).reverted);
    } catch (e) {
      handleError(e);
    } finally {
      setReverting(false);
    }
  };

  const failed = status === "installError";
  const pct = progress == null ? null : Math.round(progress);

  return (
    <div className="fixed inset-0 z-[95] flex items-center justify-center bg-bg/90 px-4 backdrop-blur">
      <div className="card w-full max-w-[440px] space-y-5 p-6 text-center">
        <div className="text-xl font-semibold tracking-tight">
          nezty <span className="text-accent">tweaks</span>
        </div>
        <div>
          <h2 className="text-lg font-semibold">{t("forceUpdate.title")}</h2>
          <p className="mt-1.5 text-[13px] leading-relaxed text-ink-2">{t("forceUpdate.text", { version: info.version })}</p>
        </div>

        {info.notes && (
          <div className="card-inset px-4 py-3 text-left">
            <div className="text-[11px] uppercase tracking-wide text-ink-3">{t("forceUpdate.notes")}</div>
            <p className="mt-1 whitespace-pre-line text-[13px] leading-relaxed text-ink-2">{info.notes}</p>
          </div>
        )}

        {failed ? (
          <div className="space-y-3">
            <p className="text-[13px] text-bad">{t("forceUpdate.failed")}</p>
            <Button variant="primary" icon={Download} className="w-full justify-center" onClick={() => void install()}>
              {t("forceUpdate.retry")}
            </Button>
          </div>
        ) : (
          <div className="space-y-2">
            <div className="h-1.5 overflow-hidden rounded-full bg-line">
              <div className="h-full rounded-full bg-accent transition-[width] duration-300" style={{ width: `${pct ?? 15}%` }} />
            </div>
            <p className="text-[12px] text-ink-3">
              {t("forceUpdate.installing")} {pct != null ? `${pct}%` : ""} · {t("forceUpdate.restart")}
            </p>
          </div>
        )}

        <div className="border-t border-line pt-4">
          <Button variant="ghost" size="sm" icon={RotateCcw} loading={reverting} onClick={() => void revert()}>
            {t("forceUpdate.revert")}
          </Button>
          {reverted != null && <p className="mt-1 text-[12px] text-ink-3">{t("forceUpdate.reverted", { count: reverted })}</p>}
        </div>
      </div>
    </div>
  );
}
