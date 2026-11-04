import { AlertCircle, CheckCircle2, ChevronDown, ExternalLink, HelpCircle, MinusCircle, Wrench } from "lucide-react";
import { useState } from "react";
import { useT } from "@/i18n/hooks";
import { api } from "@/lib/ipc";
import { formatDate, formatMb } from "@/lib/format";
import type { BigWin } from "@/lib/types";
import { useApp } from "@/store/app";
import { GainBadge } from "./TweakCard";
import { Button, cn } from "./ui";

const statusIcon = { ok: CheckCircle2, issue: AlertCircle, info: HelpCircle, unavailable: MinusCircle };
// Groen is "in orde". "Om te checken" is gewoon informatie, dus grijs: oranje betekent alleen "herstart je pc".
const statusColor = { ok: "text-ok", issue: "text-accent", info: "text-ink-3", unavailable: "text-ink-3" };

/** Controles die alleen op de laptoppagina staan. */
export const LAPTOP_CHECKS = ["laptop_heat", "laptop_gpu_mode", "laptop_wifi", "gpu_battery_limit"];

/** Eén controle (big win): wat nezty zag, en zo mogelijk een knop om het op te lossen of uitleg. */
export function BigWinCard({ win, onFixed }: { win: BigWin; onFixed: (list: BigWin[]) => void }) {
  const { t, td, tl, lang } = useT();
  const setPage = useApp((s) => s.setPage);
  const handleError = useApp((s) => s.handleError);
  const toast = useApp((s) => s.toast);
  const [busy, setBusy] = useState(false);
  const [howTo, setHowTo] = useState(false);
  const Icon = statusIcon[win.status];
  const base = `bigwin.${win.id}`;
  const params: Record<string, string> = { ...win.params };
  if (params.mb) params.size = formatMb(lang, Number(params.mb));
  if (params.date) params.date = formatDate(lang, params.date);
  if (!params.app) params.app = t("bigwin.brandApp");

  const fix = async () => {
    setBusy(true);
    try {
      const list = await api.fixBigWin(win.id);
      onFixed(list);
      toast({ level: "success", title: t("notice.success"), message: t("bigwin.fixed") });
    } catch (e) {
      handleError(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className={cn("card flex flex-col p-5", win.status === "issue" && "border-accent/50")}>
      <div className="flex items-start gap-3">
        <Icon className={cn("mt-0.5 h-5 w-5 shrink-0", statusColor[win.status])} />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="text-[15px] font-semibold">{td(`${base}.title`)}</h3>
            {win.status === "issue" && <GainBadge gain={win.gain} />}
          </div>
          <p className="mt-1.5 text-[13px] leading-relaxed text-ink-2">{td(`${base}.${win.variant}`, params)}</p>
        </div>
      </div>

      {((win.id === "ram_speed" && (win.status === "issue" || win.status === "info")) ||
        (win.id === "desktop_igpu" && win.variant === "unused_igpu") ||
        (win.id === "bloat_services" && win.variant === "found")) && (
        <div className="mt-3 pl-8">
          <button onClick={() => setHowTo((v) => !v)} className="flex items-center gap-1 text-[12px] font-medium text-ink-3 hover:text-ink-2">
            <ChevronDown className={cn("h-3.5 w-3.5 transition-transform", howTo && "rotate-180")} />
            {td(`${base}.howTo`)}
          </button>
          {howTo && (
            <ol className="card-inset mt-2 list-decimal space-y-1 py-2.5 pl-8 pr-3 text-[13px] text-ink-2">
              {tl(`${base}.steps`).map((s, i) => (
                <li key={i}>{s}</li>
              ))}
            </ol>
          )}
        </div>
      )}

      <div className="mt-4 flex flex-wrap items-center gap-2 pl-8">
        {win.action === "fix" && win.status === "issue" && (
          <>
            <Button size="sm" variant="primary" icon={Wrench} loading={busy} onClick={() => void fix()}>
              {td(`${base}.fix`, params)}
            </Button>
            {win.id === "refresh_rate" && <span className="text-[12px] text-ink-3">{t("bigwin.refresh_rate.fixNote")}</span>}
          </>
        )}
        {win.action === "link" && win.link && (win.status !== "ok" || win.id === "gpu_driver") && (
          <Button size="sm" variant={win.status === "issue" ? "primary" : "secondary"} icon={ExternalLink} onClick={() => api.openUrl(win.link!).catch(handleError)}>
            {td(`${base}.link`)}
          </Button>
        )}
        {win.action === "open_page" && win.status !== "ok" && (
          <Button size="sm" onClick={() => setPage("background")}>
            {td(`${base}.open`, undefined, t("bigwin.heavy_apps.open"))}
          </Button>
        )}
        <span className={cn("ml-auto text-[12px]", statusColor[win.status])}>{t(`bigwin.status.${win.status}`)}</span>
      </div>
    </div>
  );
}
