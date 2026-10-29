import { AlertTriangle, ArrowRight, CheckCircle2, Clock, Play, ShieldCheck, Zap } from "lucide-react";
import { useState } from "react";
import { useT } from "@/i18n/hooks";
import { api } from "@/lib/ipc";
import { formatMb, formatNumber } from "@/lib/format";
import type { BoostReport, FpsResult } from "@/lib/types";
import { useApp } from "@/store/app";
import { Button, Modal, cn } from "./ui";

function FpsCompare({ before, after }: { before: FpsResult; after: FpsResult }) {
  const { t, lang } = useT();
  const rows = [
    { label: t("result.avg"), b: before.avgFps, a: after.avgFps },
    { label: t("result.low1"), b: before.low1Fps, a: after.low1Fps },
  ];
  return (
    <div className="card-inset overflow-hidden">
      <table className="w-full text-sm">
        <thead>
          <tr className="text-[12px] text-ink-3">
            <th className="px-4 py-2 text-left font-medium" />
            <th className="px-4 py-2 text-right font-medium">{t("result.before")}</th>
            <th className="px-4 py-2 text-right font-medium">{t("result.after")}</th>
            <th className="px-4 py-2 text-right font-medium" />
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => {
            const diff = r.a - r.b;
            const pct = r.b > 0 ? (diff / r.b) * 100 : 0;
            return (
              <tr key={r.label} className="border-t border-line">
                <td className="px-4 py-2.5 text-ink-2">{r.label}</td>
                <td className="px-4 py-2.5 text-right tnum">{formatNumber(lang, r.b)} FPS</td>
                <td className="px-4 py-2.5 text-right font-semibold tnum">{formatNumber(lang, r.a)} FPS</td>
                <td className={cn("px-4 py-2.5 text-right text-[13px] font-medium tnum", Math.abs(pct) < 2 ? "text-ink-3" : pct > 0 ? "text-ok" : "text-warn")}>
                  {pct > 0 ? "+" : ""}
                  {formatNumber(lang, pct, 1)}%
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

/** Resultaatscherm na een boost: wat er echt gebeurd is, en wanneer het werkt. */
export function BoostResultModal({ report, open, onClose }: { report: BoostReport | null; open: boolean; onClose: () => void }) {
  const { t, td, lang } = useT();
  const toast = useApp((s) => s.toast);
  const handleError = useApp((s) => s.handleError);
  const [restarting, setRestarting] = useState(false);
  if (!report) return null;

  const closedMb = report.closedProcesses.reduce((a, b) => a + b.memoryMb, 0);
  const applied = report.applied;
  const now = applied.filter((a) => a.timing === "instant");
  const later = applied.filter((a) => a.timing === "game_restart");
  const powerMax = applied.some((a) => a.id === "power_plan");
  const name = (id: string) => td(`tweak.${id}.name`, undefined, id);

  const headline: { icon: typeof Zap; text: string }[] = [];
  if (report.closedProcesses.length > 0) headline.push({ icon: CheckCircle2, text: t("result.closedApps", { count: report.closedProcesses.length }) });
  if (closedMb > 50) headline.push({ icon: CheckCircle2, text: t("result.freedMem", { size: formatMb(lang, closedMb) }) });
  if (powerMax) headline.push({ icon: Zap, text: t("result.powerMax") });
  if (applied.length > 0) headline.push({ icon: CheckCircle2, text: t("result.tweaksApplied", { count: applied.length }) });
  if (report.alreadyOk.length > 0) headline.push({ icon: ShieldCheck, text: t("result.alreadyOk", { count: report.alreadyOk.length }) });

  const restart = async () => {
    setRestarting(true);
    try {
      const n = await api.restartClosedApps();
      toast({ level: "success", title: t("notice.success"), message: t("result.restarted", { count: n }) });
    } catch (e) {
      handleError(e);
    } finally {
      setRestarting(false);
    }
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      width="max-w-xl"
      title={t("result.title")}
      subtitle={report.gameName ?? undefined}
      footer={
        <>
          {report.closedProcesses.length > 0 && (
            <Button variant="ghost" icon={Play} loading={restarting} onClick={() => void restart()}>
              {t("result.restartApps")}
            </Button>
          )}
          <Button variant="primary" onClick={onClose}>
            {t("common.ok")}
          </Button>
        </>
      }
    >
      <div className="space-y-5">
        {headline.length > 0 ? (
          <ul className="space-y-2">
            {headline.map((h, i) => (
              <li key={i} className="flex items-center gap-2.5 text-[15px]">
                <h.icon className="h-4 w-4 shrink-0 text-ok" />
                {h.text}
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-sm text-ink-2">{t("result.nothingNew")}</p>
        )}

        {report.fpsBefore && report.fpsAfter && (
          <div>
            <div className="mb-2 text-sm font-semibold">{t("result.fpsTitle")}</div>
            <FpsCompare before={report.fpsBefore} after={report.fpsAfter} />
            <p className="mt-2 text-[12px] text-ink-3">{t("result.fpsNote")}</p>
          </div>
        )}

        {now.length > 0 && (
          <div>
            <div className="mb-1.5 flex items-center gap-1.5 text-[13px] font-semibold text-ok">
              <Zap className="h-3.5 w-3.5" /> {t("result.worksNow")}
            </div>
            <div className="text-[13px] text-ink-2">{now.map((a) => name(a.id)).join(" · ")}</div>
          </div>
        )}

        {later.length > 0 && (
          <div className={cn("rounded-lg border p-3", report.gameRunning ? "border-warn/30 bg-warn/10" : "border-line bg-panel-2")}>
            <div className={cn("mb-1 flex items-center gap-1.5 text-[13px] font-semibold", report.gameRunning ? "text-warn" : "text-info")}>
              <Clock className="h-3.5 w-3.5" />
              {report.gameRunning && report.gameName ? t("result.afterGameRestart", { game: report.gameName }) : t("result.afterGameRestartGeneric")}
            </div>
            <div className="text-[13px] text-ink-2">{later.map((a) => name(a.id)).join(" · ")}</div>
          </div>
        )}

        {report.skipped.length > 0 && (
          <div>
            <div className="mb-1.5 text-[13px] font-semibold text-ink-3">{t("result.skipped")}</div>
            <ul className="space-y-1 text-[13px] text-ink-2">
              {report.skipped.map((s) => (
                <li key={s.id} className="flex items-start gap-1.5">
                  <ArrowRight className="mt-1 h-3 w-3 shrink-0 text-ink-3" />
                  <span>
                    <span className="text-ink">{name(s.id)}</span>: {td(`msg.${s.reason.key}`, s.reason.params)}
                  </span>
                </li>
              ))}
            </ul>
          </div>
        )}

        {report.restorePoint && (
          <div className={cn("flex items-start gap-2 text-[13px]", report.restorePoint.ok ? "text-ok" : "text-warn")}>
            {report.restorePoint.ok ? <ShieldCheck className="mt-0.5 h-4 w-4 shrink-0" /> : <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />}
            {t(`result.restore.${report.restorePoint.key}`)}
          </div>
        )}

        {report.warnings.map((w, i) => (
          <div key={i} className="flex items-start gap-2 text-[13px] text-warn">
            <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
            {td(`msg.${w.key}`, w.params)}
          </div>
        ))}
      </div>
    </Modal>
  );
}
