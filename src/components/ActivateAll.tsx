import { AnimatePresence, motion } from "framer-motion";
import { Check, CheckCircle2, ChevronDown, Circle, Clock, History, MinusCircle, X, Zap } from "lucide-react";
import { useState } from "react";
import { useT } from "@/i18n/hooks";
import type { ApplyAllReport, ApplyItem, TweakInfo } from "@/lib/types";
import { useApp } from "@/store/app";
import { Loader, ProgressRing } from "./Loader";
import { Button, Card, IconButton, SectionTitle, cn } from "./ui";

/** Zelfde lijst en volgorde als tweaks::ACTIVATE_ALL in de backend. */
export const ACTIVATE_ALL_TWEAKS = [
  "game_mode",
  "windowed_opt",
  "mouse_precision_off",
  "accessibility_hotkeys_off",
  "power_mode_best",
  "cpu_full_speed",
  "wifi_power_max",
  "wifi_gaming_mode",
];

/** Alles wat "Alles activeren" doet, in de volgorde van de backend. Tweaks hebben hun eigen naam en uitleg. */
const COVERS: { id: string; kind: "check" | "tweak" | "game" }[] = [
  { id: "power_source", kind: "check" },
  { id: "refresh_rate", kind: "check" },
  ...ACTIVATE_ALL_TWEAKS.map((id) => ({ id, kind: "tweak" as const })),
  { id: "gpu_high_perf", kind: "game" },
  { id: "fortnite_vsync", kind: "game" },
  { id: "fortnite_reflex", kind: "game" },
  { id: "game_vsync", kind: "game" },
];

/** Overgeslagen omdat het op dit apparaat niet nodig is: geen fout, dus apart en kort samengevat. */
const NOT_NEEDED = ["platform_locked", "not_hybrid", "not_available", "wifi_none", "windowed_opt_os", "hags_unsupported"];

/** Namen voor games met een eigen instellingenbestand, ook als ze niet bij de gevonden games staan. */
const GAME_NAMES: Record<string, string> = {
  "epic:fortnite": "Fortnite",
  "steam:cs2": "Counter-Strike 2",
  "minecraft:java": "Minecraft Java Edition",
};

function useItemLabel() {
  const { t, td } = useT();
  const games = useApp((s) => s.games);
  const gameName = (id: string | null) => (id ? (games.find((g) => g.id === id)?.name ?? GAME_NAMES[id] ?? id) : "");
  return (it: ApplyItem) => {
    if (it.id === "gpu_high_perf" && it.gameId) return t("tweaks.result.gpuFor", { game: gameName(it.gameId) });
    if (it.id === "game_vsync" && it.gameId) return t("tweaks.result.vsyncFor", { game: gameName(it.gameId) });
    if (it.id === "gpu_high_perf" || it.id === "power_source" || it.id === "refresh_rate" || it.id.startsWith("fortnite_") || it.id === "game_vsync") {
      return td(`tweaks.covers.item.${it.id}.name`, undefined, it.id);
    }
    return td(`tweak.${it.id}.name`, undefined, it.id);
  };
}

/* ---------- Uitkomst ---------- */

function ApplyResult({ report, onClose }: { report: ApplyAllReport; onClose: () => void }) {
  const { t, td } = useT();
  const label = useItemLabel();
  const notNeeded = report.items.filter((i) => i.outcome === "skipped" && i.reason && NOT_NEEDED.includes(i.reason.key));
  const shown = report.items
    .filter((i) => i.outcome === "failed" || (i.outcome === "skipped" && !notNeeded.includes(i)))
    .sort((a, b) => Number(b.outcome === "failed") - Number(a.outcome === "failed"));
  // Overgeslagen met een reden waar de gebruiker iets aan kan doen (game open, beheerder nodig, zelf oplossen).
  const fixable = shown.filter((i) => i.outcome === "skipped").length;
  const [open, setOpen] = useState(report.failed > 0 || fixable > 0);
  const restart = report.restart.required;
  const failed = report.failed > 0;
  const title = failed ? t("tweaks.result.titleFailed") : restart ? t("tweaks.result.titleRestart") : t("tweaks.result.title");
  // De tekst klopt altijd met de getallen eronder: "alles staat aan" alleen als er niets mislukt of te doen is.
  const sub = failed
    ? t("tweaks.result.someFailed")
    : restart
      ? t("status.restartSubGeneric")
      : fixable > 0
        ? t("tweaks.result.someSkipped")
        : report.applied === 0
          ? t("tweaks.result.nothing")
          : t("tweaks.result.sub");
  const stats: { key: "applied" | "already" | "skipped" | "failed"; value: number; tone: string }[] = [
    { key: "applied", value: report.applied, tone: report.applied > 0 ? "text-ok" : "text-ink" },
    { key: "already", value: report.already, tone: "text-ink" },
    { key: "skipped", value: report.skipped, tone: "text-ink" },
    { key: "failed", value: report.failed, tone: report.failed > 0 ? "text-bad" : "text-ink" },
  ];

  return (
    <motion.div
      initial={{ opacity: 0, height: 0 }}
      animate={{ opacity: 1, height: "auto" }}
      exit={{ opacity: 0, height: 0 }}
      transition={{ duration: 0.28, ease: [0.22, 1, 0.36, 1] }}
      className="overflow-hidden"
    >
      <div className="mt-6 border-t border-line pt-5">
        <div className="flex items-start justify-between gap-3">
          <div>
            <div className={cn("text-[15px] font-semibold", restart && !failed && "text-warn")}>{title}</div>
            <div className="mt-0.5 text-[13px] text-ink-2">{sub}</div>
          </div>
          <IconButton icon={X} label={t("common.close")} onClick={onClose} />
        </div>

        <div className="mt-4 grid grid-cols-2 gap-3 sm:grid-cols-4">
          {stats.map((s) => (
            <div key={s.key} className="card-inset px-3 py-2.5">
              <div className={cn("text-[22px] font-semibold leading-none tnum", s.tone)}>{s.value}</div>
              <div className="mt-1.5 text-[12px] text-ink-3">{t(`tweaks.result.${s.key}`)}</div>
            </div>
          ))}
        </div>

        {report.restorePoint && (
          <p className="mt-3 flex items-center gap-2 text-[12px] text-ink-3">
            <History className="h-3.5 w-3.5 shrink-0" />
            {t(`result.restore.${report.restorePoint.key}`)}
          </p>
        )}

        {(shown.length > 0 || notNeeded.length > 0) && (
          <div className="mt-4">
            <button onClick={() => setOpen((v) => !v)} className="flex items-center gap-1 text-[12px] font-medium text-ink-3 hover:text-ink-2">
              <ChevronDown className={cn("h-3.5 w-3.5 transition-transform", open && "rotate-180")} />
              {open ? t("tweaks.result.hideDetails") : t("tweaks.result.showDetails")}
            </button>
            <AnimatePresence initial={false}>
              {open && (
                <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }} transition={{ duration: 0.18 }} className="overflow-hidden">
                  <ul className="card-inset mt-2 divide-y divide-line">
                    {shown.map((i, n) => (
                      <li key={`${i.id}:${i.gameId ?? ""}:${n}`} className="flex items-start gap-3 px-3 py-2.5">
                        {i.outcome === "failed" ? <X className="mt-0.5 h-4 w-4 shrink-0 text-bad" /> : <MinusCircle className="mt-0.5 h-4 w-4 shrink-0 text-ink-3" />}
                        <div className="min-w-0 flex-1">
                          <div className="text-[13px] font-medium">{label(i)}</div>
                          <div className="text-[12px] text-ink-3">
                            {i.reason ? td(`msg.${i.reason.key}`, i.reason.params, i.reason.key) : t(i.outcome === "failed" ? "tweaks.result.failed" : "tweaks.result.skipped")}
                          </div>
                        </div>
                      </li>
                    ))}
                    {notNeeded.length > 0 && (
                      <li className="flex items-start gap-3 px-3 py-2.5">
                        <MinusCircle className="mt-0.5 h-4 w-4 shrink-0 text-ink-3" />
                        <div className="min-w-0 flex-1 text-[12px] text-ink-3">
                          {t("tweaks.result.notNeeded", { count: notNeeded.length })} {notNeeded.map(label).join(", ")}.
                        </div>
                      </li>
                    )}
                  </ul>
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        )}
      </div>
    </motion.div>
  );
}

/* ---------- De grote knop ---------- */

/** Bovenaan de tweakspagina: de ring met de stand, de knop "Alles activeren" en daarna de uitkomst. */
export function ActivateHero() {
  const { t } = useT();
  const applyStatus = useApp((s) => s.applyStatus);
  const applying = useApp((s) => s.applying);
  const progress = useApp((s) => s.applyProgress);
  const report = useApp((s) => s.applyReport);
  const runApplyAll = useApp((s) => s.runApplyAll);
  const clearApplyReport = useApp((s) => s.clearApplyReport);

  const all = applyStatus?.allActive ?? false;
  // Niets meer aan te zetten (hoogstens wacht er nog iets op een herstart van de game): dan heet de knop "opnieuw".
  const nothingLeft = all || (applyStatus?.missing.length === 0 && applyStatus.total > 0);
  const total = progress?.total ?? 0;
  const done = progress?.done ?? 0;
  const pct = applying ? (total > 0 ? done / total : 0) : applyStatus && applyStatus.total > 0 ? applyStatus.active / applyStatus.total : 0;
  const step = applying ? (progress?.step ?? "start") : null;
  const tone = applying ? "accent" : all ? "ready" : "neutral";

  return (
    <Card className="relative overflow-hidden p-6">
      <div className="pointer-events-none absolute -left-20 -top-24 h-64 w-64 rounded-full bg-accent opacity-[0.09] blur-3xl" />
      <div className="relative flex flex-wrap items-center gap-x-8 gap-y-5">
        <ProgressRing value={pct} tone={tone} busy={applying || !applyStatus}>
          {applying ? (
            <>
              <div className="text-[30px] font-semibold leading-none tracking-tight tnum">{Math.round(pct * 100)}%</div>
              <div className="mt-1.5 text-[11px] text-ink-3 tnum">{total > 0 ? t("tweaks.hero.progress", { current: Math.min(done + 1, total), total }) : t("tweaks.steps.start")}</div>
            </>
          ) : !applyStatus ? (
            <div className="text-[12px] text-ink-3">{t("status.checking")}</div>
          ) : all ? (
            <>
              <motion.span initial={{ scale: 0.6, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} transition={{ type: "spring", stiffness: 300, damping: 18 }}>
                <Check className="h-9 w-9 text-ok" strokeWidth={2.5} />
              </motion.span>
              <div className="mt-1 text-[12px] font-medium text-ok">{t("tweaks.hero.allActive")}</div>
            </>
          ) : (
            <>
              <div className="text-[28px] font-semibold leading-none tracking-tight tnum">
                {applyStatus.active}
                <span className="text-[18px] text-ink-3">/{applyStatus.total}</span>
              </div>
              <div className="mt-1.5 text-[11px] text-ink-3">{t("tweaks.hero.activeLabel")}</div>
            </>
          )}
        </ProgressRing>

        <div className="min-w-0 max-w-xl flex-1">
          <h2 className="text-[20px] font-semibold tracking-tight">{t("tweaks.hero.title")}</h2>
          <p className="mt-1.5 text-[13px] leading-relaxed text-ink-2">{t("tweaks.hero.text")}</p>
          <div className="mt-5 flex flex-wrap items-center gap-4">
            <Button variant="primary" size="lg" icon={Zap} disabled={applying} onClick={() => void runApplyAll()} className="min-w-[200px]">
              {applying ? t("tweaks.hero.busy") : nothingLeft ? t("tweaks.hero.again") : t("tweaks.hero.button")}
            </Button>
            <div className="min-h-[20px] min-w-0 flex-1 text-[13px] text-ink-2" aria-live="polite">
              <AnimatePresence mode="wait" initial={false}>
                {step && (
                  <motion.span
                    key={step}
                    className="block truncate"
                    initial={{ opacity: 0, y: 4 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0, y: -4 }}
                    transition={{ duration: 0.18 }}
                  >
                    {t(`tweaks.steps.${step}` as "tweaks.steps.start")}
                  </motion.span>
                )}
              </AnimatePresence>
            </div>
          </div>
          <p className="mt-3 text-[12px] text-ink-3">{t("tweaks.hero.note")}</p>
        </div>
      </div>
      <AnimatePresence initial={false}>{report && !applying && <ApplyResult key="result" report={report} onClose={clearApplyReport} />}</AnimatePresence>
    </Card>
  );
}

/* ---------- Wat er in zit ---------- */

type RowState = "active" | "game" | "todo" | "na";

/** De lijst van wat "Alles activeren" doet, met een groen vinkje bij alles wat al aan staat en werkt. */
export function ActivateCovers() {
  const { t, td } = useT();
  const applyStatus = useApp((s) => s.applyStatus);
  const tweaks = useApp((s) => s.tweaks);
  const device = useApp((s) => s.device);
  const games = useApp((s) => s.games);

  const naReason = (id: string, tw: TweakInfo | undefined): string => {
    if (tw?.platformLocked) return t(`platform.${tw.platform}` as "platform.pc_only");
    if (tw?.state === "unavailable" && tw.detail) return td(`msg.${tw.detail.key}`, tw.detail.params);
    if (id === "gpu_high_perf" && device && !device.hybrid) return t("msg.not_hybrid");
    if (id === "fortnite_reflex" && applyStatus?.present.concat(applyStatus.missing).some((m) => m.id === "fortnite_vsync")) return t("tweaks.covers.nvidiaOnly");
    if (id.startsWith("fortnite_")) return games.some((g) => g.id === "epic:fortnite") ? t("msg.ini_missing") : t("fn.notFound");
    if (id === "game_vsync") return t("tweaks.covers.noGames");
    return t("tweaks.covers.na");
  };

  const rows = applyStatus
    ? COVERS.map((c) => {
        const missing = applyStatus.missing.filter((m) => m.id === c.id).length;
        const present = applyStatus.present.filter((m) => m.id === c.id).length;
        // Staat aan, maar werkt pas na een herstart van de game: grijs, geen groen vinkje.
        const waiting = applyStatus.pendingGame.filter((m) => m.id === c.id).length;
        const tw = c.kind === "tweak" ? tweaks.find((x) => x.id === c.id) : undefined;
        const all = missing + present + waiting;
        const state: RowState = all === 0 ? "na" : missing > 0 ? "todo" : waiting > 0 ? "game" : "active";
        const name = c.kind === "tweak" ? td(`tweak.${c.id}.name`, undefined, c.id) : td(`tweaks.covers.item.${c.id}.name`);
        const what = c.kind === "tweak" ? td(`tweak.${c.id}.what`) : td(`tweaks.covers.item.${c.id}.what`);
        const multi = c.id === "gpu_high_perf" || c.id === "game_vsync";
        const right =
          state === "na"
            ? naReason(c.id, tw)
            : multi && all > 1
              ? t("tweaks.covers.games", { active: present, total: all })
              : state === "active"
                ? t("tweaks.covers.active")
                : state === "game"
                  ? t("tweaks.covers.afterGame")
                  : t("tweaks.covers.todo");
        return { id: c.id, state, name, what, right };
      }).sort((a, b) => Number(a.state === "na") - Number(b.state === "na"))
    : [];

  return (
    <section>
      <SectionTitle sub={t("tweaks.covers.sub")}>{t("tweaks.covers.title")}</SectionTitle>
      <Card className="!px-2 !py-2">
        {!applyStatus ? (
          <Loader label={t("tweaks.loading")} />
        ) : (
          <ul className="grid gap-x-2 md:grid-cols-2">
            {rows.map((r) => {
              const Icon = r.state === "active" ? CheckCircle2 : r.state === "game" ? Clock : r.state === "todo" ? Circle : MinusCircle;
              return (
                <li key={r.id} className={cn("flex items-start gap-3 rounded-lg px-3 py-3", r.state === "na" && "opacity-60")}>
                  <Icon className={cn("mt-0.5 h-[18px] w-[18px] shrink-0", r.state === "active" ? "text-ok" : "text-ink-3")} />
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-0.5">
                      <span className="text-sm font-medium">{r.name}</span>
                      <span className={cn("text-[12px]", r.state === "active" ? "text-ok" : "text-ink-3")}>{r.right}</span>
                    </div>
                    <p className="mt-0.5 text-[12px] leading-snug text-ink-3">{r.what}</p>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </Card>
    </section>
  );
}
