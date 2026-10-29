import { Gauge, Info, Mouse, RotateCcw } from "lucide-react";
import { useCallback, useRef, useState, type PointerEvent as RPointerEvent } from "react";
import { LiveInputCard } from "@/components/LiveInputCard";
import { TweakCard } from "@/components/TweakCard";
import { Button, Card, PageHeader, SectionTitle, Skeleton, cn } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import type { TweakInfo } from "@/lib/types";
import { useApp } from "@/store/app";

const STANDARD_RATES = [125, 250, 500, 1000, 2000, 4000, 8000];

function snap(hz: number): number {
  const near = STANDARD_RATES.find((r) => Math.abs(r - hz) / r < 0.15);
  return near ?? Math.round(hz / 10) * 10;
}

/** Meet de pollingrate via de tijd tussen ruwe muisgebeurtenissen (mediaan, robuust tegen pauzes). */
function PollingTester() {
  const { t } = useT();
  const stamps = useRef<number[]>([]);
  const [hz, setHz] = useState<number | null>(null);
  const [measuring, setMeasuring] = useState(false);

  const compute = useCallback(() => {
    const s = stamps.current;
    if (s.length < 60) return;
    const recent = s.slice(-400);
    const intervals: number[] = [];
    for (let i = 1; i < recent.length; i++) {
      const d = recent[i] - recent[i - 1];
      if (d > 0 && d < 40) intervals.push(d);
    }
    if (intervals.length < 50) return;
    intervals.sort((a, b) => a - b);
    const median = intervals[Math.floor(intervals.length / 2)];
    setHz((prev) => Math.max(prev ?? 0, snap(1000 / median)));
  }, []);

  const onEvent = useCallback(
    (e: RPointerEvent<HTMLDivElement>) => {
      const native = e.nativeEvent as PointerEvent & { getCoalescedEvents?: () => PointerEvent[] };
      const list = native.getCoalescedEvents ? native.getCoalescedEvents() : [native];
      for (const ev of list.length ? list : [native]) stamps.current.push(ev.timeStamp);
      if (stamps.current.length > 4000) stamps.current = stamps.current.slice(-2000);
      if (!measuring) setMeasuring(true);
      if (stamps.current.length % 50 === 0) compute();
    },
    [compute, measuring],
  );

  const reset = () => {
    stamps.current = [];
    setHz(null);
    setMeasuring(false);
  };

  const advice = hz == null ? null : hz < 900 ? t("input.pollingLow", { hz }) : t("input.pollingGood", { hz });

  return (
    <Card>
      <div className="flex items-start justify-between gap-4">
        <div>
          <h3 className="flex items-center gap-2 text-[15px] font-semibold">
            <Gauge className="h-4 w-4 text-accent" />
            {t("input.pollingTitle")}
          </h3>
          <p className="mt-1 text-[13px] text-ink-2">{t("input.pollingDesc")}</p>
        </div>
        <Button size="sm" variant="ghost" icon={RotateCcw} onClick={reset}>
          {t("input.pollingReset")}
        </Button>
      </div>
      <div className="mt-4 grid gap-4 md:grid-cols-[minmax(0,1fr)_220px]">
        <div
          onPointerMove={onEvent}
          className="flex h-44 select-none items-center justify-center rounded-lg border border-dashed border-line-2 bg-panel-2 text-[13px] text-ink-3"
          style={{ touchAction: "none" }}
        >
          <span className="flex items-center gap-2">
            <Mouse className="h-4 w-4" />
            {measuring && hz == null ? t("input.pollingMeasuring") : t("input.pollingArea")}
          </span>
        </div>
        <div className="card-inset flex flex-col justify-center p-4 text-center">
          <div className={cn("text-[34px] font-semibold tnum", hz == null ? "text-ink-3" : hz < 900 ? "text-warn" : "text-ok")}>{hz == null ? "-" : t("input.pollingResult", { hz })}</div>
        </div>
      </div>
      {advice && <p className="mt-3 text-[13px] leading-relaxed text-ink">{advice}</p>}
      <p className="mt-2 text-[12px] text-ink-3">{t("input.pollingNote")}</p>
    </Card>
  );
}

export function InputPage() {
  const { t, td } = useT();
  const tweaks = useApp((s) => s.tweaks);
  const loading = useApp((s) => s.tweaksLoading);
  const setTweak = useApp((s) => s.setTweak);
  const appInfo = useApp((s) => s.appInfo);
  const toast = useApp((s) => s.toast);
  const [busy, setBusy] = useState<string | null>(null);
  const list = tweaks.filter((tw) => tw.category === "input");
  const gpu = (appInfo?.gpuName ?? "").toLowerCase();

  const onToggle = async (tw: TweakInfo, enabled: boolean, option: string | null) => {
    setBusy(tw.id);
    const info = await setTweak(tw.id, enabled, option);
    setBusy(null);
    if (info) {
      const name = td(`tweak.${tw.id}.name`);
      toast({ level: "success", title: t(`timing.${tw.timing}`), message: enabled ? t("tweakCard.applied", { name }) : t("tweakCard.disabled", { name }) });
    }
  };

  const tips = [
    gpu.includes("nvidia") || !gpu ? t("input.tipReflex") : null,
    gpu.includes("amd") || gpu.includes("radeon") ? t("input.tipAntiLag") : null,
    gpu.includes("intel") ? t("input.tipIntel") : null,
    t("input.tipWired"),
    t("input.tipSync"),
  ].filter(Boolean) as string[];

  return (
    <div className="space-y-8">
      <PageHeader title={t("input.title")} subtitle={t("input.subtitle")} />

      <div className="flex items-start gap-3 rounded-lg border border-line bg-panel px-4 py-3 text-[13px] text-ink-2">
        <Info className="mt-0.5 h-4 w-4 shrink-0 text-info" />
        {t("input.honest")}
      </div>

      <LiveInputCard />

      <section>
        <SectionTitle>{t("input.settings")}</SectionTitle>
        {loading && list.length === 0 ? (
          <Skeleton className="h-48" />
        ) : (
          <div className="grid gap-4 lg:grid-cols-2">
            {list.map((tw) => (
              <TweakCard key={tw.id} tweak={tw} elevated={appInfo?.isElevated ?? false} busy={busy === tw.id} onToggle={(v, opt) => void onToggle(tw, v, opt)} />
            ))}
          </div>
        )}
      </section>

      <PollingTester />

      <section>
        <SectionTitle>{t("input.tipsTitle")}</SectionTitle>
        <Card>
          <ul className="space-y-2.5 text-[13px] leading-relaxed">
            {tips.map((tip, i) => (
              <li key={i} className="flex gap-2.5">
                <span className="mt-2 h-1.5 w-1.5 shrink-0 rounded-full bg-accent" />
                {tip}
              </li>
            ))}
          </ul>
        </Card>
      </section>
    </div>
  );
}
