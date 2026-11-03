import { AnimatePresence, motion } from "framer-motion";
import { CheckCircle2, ChevronDown, Circle, Clock, Info, Pickaxe } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Badge, Button, Card, cn } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import type { TKey } from "@/i18n";
import { api } from "@/lib/ipc";
import type { FortniteOverview } from "@/lib/types";
import { useApp } from "@/store/app";
import { FortniteIniTab } from "./GameTabs";
import { Loader } from "./Loader";

/**
 * Fortnite onder "Meer opties": de keuzes die niet in "Alles activeren" zitten (FPS-limiet en de rendermodus
 * Prestaties), plus de eigen editor met back-ups. V-Sync en Reflex regelt "Alles activeren" al.
 */
export function FortniteSection() {
  const { t } = useT();
  const games = useApp((s) => s.games);
  const handleError = useApp((s) => s.handleError);
  const toast = useApp((s) => s.toast);
  const refreshStatus = useApp((s) => s.refreshStatus);
  const [ov, setOv] = useState<FortniteOverview | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [advanced, setAdvanced] = useState(false);
  const detected = games.some((g) => g.id === "epic:fortnite");

  const load = useCallback(async () => {
    try {
      setOv(await api.getFortniteOverview());
    } catch (e) {
      handleError(e);
    }
  }, [handleError]);

  useEffect(() => {
    void load();
  }, [load]);

  const apply = async (id: string) => {
    setBusy(id);
    try {
      const next = await api.applyFortniteRecommended([id]);
      setOv(next);
      // Fortnite open: de backend onthoudt het en zet het na het afsluiten.
      toast({ level: "success", title: t("notice.success"), message: next.pending.includes(id) ? t("fn.queued") : t("fn.applied", { count: 1 }) });
      void refreshStatus();
    } catch (e) {
      handleError(e);
    } finally {
      setBusy(null);
    }
  };

  if (!ov) return <Loader />;
  if (!detected && !ov.iniExists) return null;

  const fps = ov.checks.find((c) => c.id === "fps_limit");
  const render = ov.checks.find((c) => c.id === "render_mode");
  const fullscreen = ov.checks.find((c) => c.id === "fullscreen");
  // Met Fortnite open kan het wel: dan staat het klaar voor na het afsluiten.
  const locked = !ov.iniExists;
  const fpsText = (v: string | null) => (v == null ? t("fn.notSet") : Number(v) === 0 ? t("fn.unlimited") : `${Math.round(Number(v))} FPS`);

  const rows: { id: string; ok: boolean; name: string; why: string; now: string; action: string; done: string }[] = [];
  if (fps) {
    rows.push({
      id: "fps_limit",
      ok: fps.ok,
      name: t("fn.check.fps_limit.name"),
      why: t("fn.check.fps_limit.why"),
      now: fps.ok ? fpsText(fps.current) : `${fpsText(fps.current)} · ${t("fn.advice", { value: fpsText(fps.recommended) })}`,
      action: t("fn.set"),
      done: t("fn.allGood"),
    });
  }
  if (fullscreen) {
    const modeText = (v: string | null) => (v === "0" || v === "1" || v === "2" ? t(`fortnite.mode${v}` as TKey) : t("fn.notSet"));
    rows.push({
      id: "fullscreen",
      ok: fullscreen.ok,
      name: t("fn.check.fullscreen.name"),
      why: t("fn.check.fullscreen.why"),
      now: fullscreen.ok ? modeText(fullscreen.current) : `${modeText(fullscreen.current)} · ${t("fn.advice", { value: t("fortnite.mode0") })}`,
      action: t("fn.set"),
      done: t("fn.allGood"),
    });
  }
  if (render) {
    rows.push({
      id: "render_mode",
      ok: render.ok,
      name: t("fn.renderTitle"),
      why: `${t(`fn.renderAdvice.${ov.advice}` as TKey)} ${t("fn.renderDiff")}`,
      now: t("fn.renderNow", { mode: t(`fn.mode.${ov.renderMode}` as TKey) }),
      action: t("fn.renderSet"),
      done: t("fn.renderOk"),
    });
  }

  return (
    <Card>
      <div className="flex items-center gap-3">
        <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-accent-soft text-accent">
          <Pickaxe className="h-4 w-4" />
        </span>
        <div className="min-w-0">
          <h3 className="text-[15px] font-semibold">Fortnite</h3>
          <p className="text-[13px] text-ink-3">{t("fn.subtitle")}</p>
        </div>
      </div>

      {!ov.iniExists && <p className="mt-3 text-[13px] text-ink-3">{t("msg.ini_missing")}</p>}
      {ov.running && (
        <p className="card-inset mt-3 flex items-start gap-2 px-3 py-2.5 text-[13px] text-ink-2">
          <Info className="mt-0.5 h-4 w-4 shrink-0 text-ink-3" />
          {t("fn.running")}
        </p>
      )}

      {rows.length > 0 && (
        <ul className="mt-3 divide-y divide-line">
          {rows.map((r) => (
            <li key={r.id} className="flex flex-wrap items-start gap-3 py-3.5">
              {r.ok ? <CheckCircle2 className="mt-0.5 h-[18px] w-[18px] shrink-0 text-ok" /> : <Circle className="mt-0.5 h-[18px] w-[18px] shrink-0 text-ink-3" />}
              <div className="min-w-0 flex-1">
                <div className="text-sm font-medium">{r.name}</div>
                <div className="mt-0.5 text-[13px] leading-relaxed text-ink-2">{r.why}</div>
                <div className="mt-1 text-[12px] text-ink-3">{r.now}</div>
              </div>
              {r.ok ? (
                <Badge variant="ready" icon={CheckCircle2}>
                  {r.done}
                </Badge>
              ) : ov.pending.includes(r.id) ? (
                <Badge variant="pending" icon={Clock}>
                  {t("fn.pending")}
                </Badge>
              ) : (
                <Button size="sm" loading={busy === r.id} disabled={locked} onClick={() => void apply(r.id)}>
                  {r.action}
                </Button>
              )}
            </li>
          ))}
        </ul>
      )}
      {rows.length > 0 && <p className="text-[12px] text-ink-3">{ov.running ? t("fn.closeFirst") : t("fn.backupNote")}</p>}

      {ov.iniExists && (
        <div className="mt-4 border-t border-line pt-3">
          <button onClick={() => setAdvanced((v) => !v)} className="flex w-full items-center justify-between gap-3 text-left">
            <span>
              <span className="block text-sm font-medium">{t("fn.advancedTitle")}</span>
              <span className="block text-[12px] text-ink-3">{t("fn.advancedSub")}</span>
            </span>
            <ChevronDown className={cn("h-4 w-4 shrink-0 text-ink-3 transition-transform", advanced && "rotate-180")} />
          </button>
          <AnimatePresence initial={false}>
            {advanced && (
              <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }} transition={{ duration: 0.2 }} className="overflow-hidden">
                <div className="pt-4">
                  <FortniteIniTab />
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      )}
    </Card>
  );
}
