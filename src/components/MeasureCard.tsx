import { CheckCircle2, Gauge, Info, Play, Undo2, Zap } from "lucide-react";
import { useEffect, useState } from "react";
import { useT } from "@/i18n/hooks";
import { api, listenEvent } from "@/lib/ipc";
import type { GameRegion, Measurement } from "@/lib/types";
import { useApp } from "@/store/app";
import { FrametimeChart } from "./FrametimeChart";
import { Button, Card, SectionTitle, cn } from "./ui";

const pct = (a: number, b: number) => (a > 0 ? ((b - a) / a) * 100 : 0);

/** Zinnen in gewone taal over het verschil. Alleen wat gemeten is, niets beloofd. */
function useCompare(before: Measurement, after: Measurement) {
  const { t, lang } = useT();
  const f0 = before.fps;
  const f1 = after.fps;
  const n = (v: number) => Math.round(v).toLocaleString(lang);
  const lines: string[] = [];
  const fpsChange = pct(f0.avgFps, f1.avgFps);
  lines.push(t("measure.lineFps", { a: n(f0.avgFps), b: n(f1.avgFps), pct: `${fpsChange >= 0 ? "+" : ""}${fpsChange.toFixed(0)}%` }));
  lines.push(t("measure.lineLow", { a: n(f0.low1Fps), b: n(f1.low1Fps) }));
  const s0 = f0.stuttersPerMin;
  const s1 = f1.stuttersPerMin;
  let stutterGain = 0;
  if (s0 < 0.5 && s1 < 0.5) lines.push(t("measure.lineNoStutter"));
  else {
    stutterGain = s0 > 0 ? ((s0 - s1) / s0) * 100 : -100;
    if (Math.abs(stutterGain) < 10) lines.push(t("measure.lineStutterSame"));
    else if (stutterGain > 0) lines.push(t("measure.lineStutterLess", { pct: stutterGain.toFixed(0) }));
    else lines.push(t("measure.lineStutterMore", { pct: Math.abs(stutterGain).toFixed(0) }));
  }
  const p0 = before.ping?.avgMs ?? null;
  const p1 = after.ping?.avgMs ?? null;
  if (p0 != null && p1 != null) {
    lines.push(Math.abs(p1 - p0) < 1 ? t("measure.linePingSame", { ms: n(p1) }) : t("measure.linePing", { a: n(p0), b: n(p1) }));
    const loss = Math.max(before.ping?.lossPct ?? 0, after.ping?.lossPct ?? 0);
    if (loss > 0) lines.push(t("measure.lineLoss", { pct: n(loss) }));
  }
  const lowChange = pct(f0.low1Fps, f1.low1Fps);
  const improved = fpsChange >= 3 || lowChange >= 5 || stutterGain >= 15;
  const worse = fpsChange <= -5 && lowChange <= 0;
  return { lines, improved, worse };
}

function Result({ before, after }: { before: Measurement; after: Measurement }) {
  const { t } = useT();
  const tweaks = useApp((s) => s.tweaks);
  const { lines, improved, worse } = useCompare(before, after);
  const gpuBound = (after.fps.gpuBusy ?? 0) >= 0.9;
  const cpuBound = !gpuBound && (after.fps.cpuBusy ?? 0) >= 0.9;
  const experimentalOn = tweaks.some((tw) => tw.experimental && tw.state === "on");
  const max = Math.max(...before.fps.graph, ...after.fps.graph, before.fps.medianMs * 3, 1);
  const scale = Math.min(max, Math.max(before.fps.medianMs, after.fps.medianMs) * 6);

  return (
    <div className="mt-4 space-y-4 border-t border-line pt-4">
      <div className={cn("text-[15px] font-semibold", improved ? "text-ok" : worse ? "text-warn" : "text-ink")}>
        {improved ? t("measure.verdictBetter") : worse ? t("measure.verdictWorse") : t("measure.verdictSame")}
      </div>
      <ul className="space-y-1 text-[13px] text-ink">
        {lines.map((l) => (
          <li key={l}>{l}</li>
        ))}
      </ul>
      {!improved && (
        <div className="space-y-1 rounded-lg border border-line bg-panel-2 px-3 py-2.5 text-[13px] text-ink-2">
          <div className="font-medium text-ink">{t("measure.whyTitle")}</div>
          <p>{gpuBound ? t("measure.whyGpu") : cpuBound ? t("measure.whyCpu") : t("measure.whyOk")}</p>
          {experimentalOn && <p>{t("measure.whyExperimental")}</p>}
          <p>{t("measure.whySame")}</p>
        </div>
      )}
      <div className="grid gap-4 lg:grid-cols-2">
        <FrametimeChart title={t("measure.before")} data={before.fps.graph} max={scale} median={before.fps.medianMs} seconds={Math.round(before.fps.seconds)} />
        <FrametimeChart title={t("measure.after")} data={after.fps.graph} max={scale} median={after.fps.medianMs} seconds={Math.round(after.fps.seconds)} />
      </div>
      <p className="text-[12px] text-ink-3">{t("measure.chartNote")}</p>
    </div>
  );
}

/** Test vóór en na: meten zonder boost, boost aan, opnieuw meten, eerlijk resultaat. */
export function MeasureCard() {
  const { t } = useT();
  const games = useApp((s) => s.games);
  const running = useApp((s) => s.runningGames);
  const boost = useApp((s) => s.boost);
  const runBoost = useApp((s) => s.runBoost);
  const runUnboost = useApp((s) => s.runUnboost);
  const handleError = useApp((s) => s.handleError);
  const [regions, setRegions] = useState<GameRegion[]>([]);
  const [gameId, setGameId] = useState<string>("");
  const [host, setHost] = useState<string>("");
  const [seconds, setSeconds] = useState(60);
  const [before, setBefore] = useState<Measurement | null>(null);
  const [after, setAfter] = useState<Measurement | null>(null);
  const [busy, setBusy] = useState<"before" | "after" | "boost" | "undo" | null>(null);
  const [elapsed, setElapsed] = useState(0);

  useEffect(() => {
    api.getGameRegions().then((r) => {
      setRegions(r);
      setHost((h) => h || (r.find((x) => x.id === "eu-central") ?? r[0])?.host || "");
    }).catch(() => undefined);
    let off: (() => void) | undefined;
    void listenEvent<number>("measure-progress", setElapsed).then((u) => (off = u));
    return () => off?.();
  }, []);

  // Standaard de game die nu draait.
  useEffect(() => {
    if (!gameId) setGameId(running[0] ?? games[0]?.id ?? "");
  }, [running, games, gameId]);

  const measure = async (which: "before" | "after") => {
    setBusy(which);
    setElapsed(0);
    try {
      const m = await api.measureSession(gameId, seconds, host || null);
      if (which === "before") {
        setBefore(m);
        setAfter(null);
      } else setAfter(m);
    } catch (e) {
      handleError(e);
    } finally {
      setBusy(null);
    }
  };

  const isRunning = running.includes(gameId);
  const sel = "h-9 rounded-lg border border-line bg-panel-2 px-3 text-sm text-ink focus:border-accent focus:outline-none";

  return (
    <section>
      <SectionTitle sub={t("measure.sub")}>{t("measure.title")}</SectionTitle>
      <Card>
        <div className="flex flex-wrap items-end gap-3">
          <label className="flex flex-col gap-1 text-[12px] text-ink-3">
            {t("measure.game")}
            <select className={sel} value={gameId} onChange={(e) => { setGameId(e.target.value); setBefore(null); setAfter(null); }}>
              {games.map((g) => (
                <option key={g.id} value={g.id}>
                  {g.name}
                  {running.includes(g.id) ? ` (${t("measure.runningNow")})` : ""}
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-[12px] text-ink-3">
            {t("measure.region")}
            <select className={sel} value={host} onChange={(e) => setHost(e.target.value)}>
              <option value="">{t("measure.noPing")}</option>
              {regions.map((r) => (
                <option key={r.id} value={r.host}>
                  {r.city}
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-[12px] text-ink-3">
            {t("measure.duration")}
            <select className={sel} value={seconds} onChange={(e) => setSeconds(Number(e.target.value))}>
              <option value={30}>30 s</option>
              <option value={60}>60 s</option>
            </select>
          </label>
        </div>

        {!isRunning && <p className="mt-3 flex items-center gap-2 text-[13px] text-warn"><Info className="h-4 w-4" />{t("measure.startGame")}</p>}

        <ol className="mt-4 space-y-3">
          <li className="flex flex-wrap items-center gap-3">
            <span className="w-5 text-center text-sm font-semibold text-ink-3">1</span>
            {boost.active && !before ? (
              <>
                <span className="text-[13px] text-ink-2">{t("measure.boostOnFirst")}</span>
                <Button size="sm" icon={Undo2} loading={busy === "undo"} onClick={async () => { setBusy("undo"); await runUnboost(); setBusy(null); }}>
                  {t("measure.undoBoost")}
                </Button>
              </>
            ) : (
              <Button icon={Play} loading={busy === "before"} disabled={!isRunning || busy !== null} onClick={() => void measure("before")}>
                {busy === "before" ? t("measure.measuring", { s: String(Math.max(0, seconds - elapsed)) }) : before ? t("measure.againBefore") : t("measure.measureBefore")}
              </Button>
            )}
            {before && <span className="flex items-center gap-1.5 text-[13px] text-ink-2"><CheckCircle2 className="h-4 w-4 text-ok" />{t("measure.summary", { fps: Math.round(before.fps.avgFps), low: Math.round(before.fps.low1Fps) })}</span>}
          </li>
          <li className="flex flex-wrap items-center gap-3">
            <span className="w-5 text-center text-sm font-semibold text-ink-3">2</span>
            {boost.active ? (
              <span className="flex items-center gap-1.5 text-[13px] text-ink-2"><Zap className="h-4 w-4 text-ok" fill="currentColor" />{t("measure.boostActive")}</span>
            ) : (
              <Button icon={Zap} loading={busy === "boost"} disabled={!before || busy !== null} onClick={async () => { setBusy("boost"); await runBoost(gameId, false); setBusy(null); }}>
                {t("measure.boostOn")}
              </Button>
            )}
          </li>
          <li className="flex flex-wrap items-center gap-3">
            <span className="w-5 text-center text-sm font-semibold text-ink-3">3</span>
            <Button icon={Gauge} loading={busy === "after"} disabled={!before || !boost.active || !isRunning || busy !== null} onClick={() => void measure("after")}>
              {busy === "after" ? t("measure.measuring", { s: String(Math.max(0, seconds - elapsed)) }) : t("measure.measureAfter")}
            </Button>
            {after && <span className="flex items-center gap-1.5 text-[13px] text-ink-2"><CheckCircle2 className="h-4 w-4 text-ok" />{t("measure.summary", { fps: Math.round(after.fps.avgFps), low: Math.round(after.fps.low1Fps) })}</span>}
          </li>
        </ol>
        {busy === "before" || busy === "after" ? <p className="mt-3 text-[13px] text-ink-2">{t("measure.playNow")}</p> : null}
        <p className="mt-3 text-[12px] text-ink-3">{t("measure.fairNote")}</p>

        {before && after && <Result before={before} after={after} />}
      </Card>
    </section>
  );
}
