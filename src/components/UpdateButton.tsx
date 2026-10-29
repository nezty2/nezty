import { Download, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useT } from "@/i18n/hooks";
import { useApp } from "@/store/app";
import { useUpdate } from "@/store/update";
import { Button, Spinner } from "./ui";

/**
 * Blauw rond pictogram linksonder in de zijbalk zodra er een nieuwe versie is. Een klik toont wat er nieuw is
 * en laat je met één klik bijwerken. Zoekt zelf kort na het starten (als dat aan staat).
 */
export function UpdateButton() {
  const { t } = useT();
  const ready = useApp((s) => s.ready);
  const autoCheck = useApp((s) => s.settings?.autoUpdateCheck ?? true);
  const { status, info, progress, check, install } = useUpdate();
  const [open, setOpen] = useState(false);

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

  const show = info && (status === "available" || status === "installing" || status === "installError");
  if (!show) return null;

  const installing = status === "installing";
  const label = t("update.available", { version: info.version });
  return (
    <div className="relative">
      <button
        onClick={() => setOpen((v) => !v)}
        title={label}
        aria-label={label}
        aria-expanded={open}
        className="flex h-10 w-full items-center gap-3 rounded-lg px-1.5 text-sm font-medium text-ink hover:bg-panel-2"
      >
        <span className="relative flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-[#2f7cf6] text-white">
          {installing ? <Spinner className="h-4 w-4" /> : <Download className="h-4 w-4" />}
          {!installing && <span className="absolute -right-0.5 -top-0.5 h-2.5 w-2.5 rounded-full border-2 border-panel bg-ok" />}
        </span>
        <span className="hidden truncate min-[1120px]:inline">{t("update.short", { version: info.version })}</span>
      </button>
      {open && (
        <div className="absolute bottom-0 left-full z-50 ml-3 w-[320px] rounded-xl border border-line bg-panel p-4 shadow-xl shadow-black/50">
          <div className="flex items-start gap-2">
            <div className="min-w-0 flex-1">
              <div className="text-[15px] font-semibold">{label}</div>
              <div className="mt-0.5 text-[13px] text-ink-2">{status === "installError" ? t("update.installError") : t("update.availableDesc")}</div>
            </div>
            <button onClick={() => setOpen(false)} className="rounded p-1 text-ink-3 hover:text-ink" aria-label={t("update.later")} title={t("update.later")}>
              <X className="h-4 w-4" />
            </button>
          </div>
          {info.notes && <div className="mt-2 line-clamp-4 whitespace-pre-line text-[13px] text-ink">{info.notes}</div>}
          <Button className="mt-3 w-full" variant="primary" icon={Download} loading={installing} disabled={installing} onClick={() => void install()}>
            {installing ? (progress != null ? t("update.installing", { pct: progress }) : t("update.installingNoPct")) : t("update.install")}
          </Button>
        </div>
      )}
    </div>
  );
}
