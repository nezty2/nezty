import { Download, X } from "lucide-react";
import { useEffect } from "react";
import { useT } from "@/i18n/hooks";
import { useApp } from "@/store/app";
import { useUpdate } from "@/store/update";
import { Button } from "./ui";

/** Balk bovenaan als er een nieuwe versie is. Zoekt zelf kort na het starten (als dat aan staat). */
export function UpdateBanner() {
  const { t } = useT();
  const ready = useApp((s) => s.ready);
  const autoCheck = useApp((s) => s.settings?.autoUpdateCheck ?? true);
  const { status, info, progress, dismissed, check, install, dismiss } = useUpdate();

  useEffect(() => {
    if (!ready || !autoCheck) return;
    let last = 0;
    // Niet opnieuw zoeken als er al een update klaarstaat of wordt geïnstalleerd.
    const run = () => {
      const s = useUpdate.getState().status;
      if (s === "available" || s === "installing" || s === "checking") return;
      last = Date.now();
      void check();
    };
    const first = setTimeout(run, 8000);
    // Daarna elke 3 uur zolang nezty openstaat.
    const every = setInterval(run, 3 * 60 * 60 * 1000);
    // En als het venster terugkomt na minstens een uur op de achtergrond.
    const onVisible = () => {
      if (document.visibilityState === "visible" && Date.now() - last > 60 * 60 * 1000) run();
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      clearTimeout(first);
      clearInterval(every);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [ready, autoCheck, check]);

  const show = info && !dismissed && (status === "available" || status === "installing" || status === "installError");
  if (!show) return null;

  const installing = status === "installing";
  return (
    <div className="mb-6 flex flex-wrap items-center gap-4 rounded-xl border border-accent/40 bg-accent-soft px-5 py-4">
      <Download className="h-5 w-5 shrink-0 text-accent" />
      <div className="min-w-0 flex-1">
        <div className="text-[15px] font-semibold">{t("update.available", { version: info.version })}</div>
        <div className="mt-0.5 text-[13px] text-ink-2">{status === "installError" ? t("update.installError") : t("update.availableDesc")}</div>
        {info.notes && <div className="mt-1.5 line-clamp-3 whitespace-pre-line text-[13px] text-ink">{info.notes}</div>}
      </div>
      <div className="flex shrink-0 items-center gap-2">
        <Button variant="primary" icon={Download} loading={installing} disabled={installing} onClick={() => void install()}>
          {installing ? (progress != null ? t("update.installing", { pct: progress }) : t("update.installingNoPct")) : t("update.install")}
        </Button>
        {!installing && (
          <button onClick={dismiss} className="rounded p-1.5 text-ink-3 hover:text-ink" aria-label={t("update.later")} title={t("update.later")}>
            <X className="h-4 w-4" />
          </button>
        )}
      </div>
    </div>
  );
}
