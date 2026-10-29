import { CheckCircle2, Rocket } from "lucide-react";
import { useState } from "react";
import { useT } from "@/i18n/hooks";
import type { TKey } from "@/i18n";
import type { TweakInfo } from "@/lib/types";
import { useApp } from "@/store/app";
import { GainBadge, TimingBadge } from "./TweakCard";
import { Badge, Button, Card, SectionTitle, Toggle, cn } from "./ui";

/** De tweaks met echt meetbaar effect op FPS en vertraging, in volgorde van nut. */
const BEST = ["power_plan", "game_bar_dvr_off", "game_mode", "windowed_opt", "hags", "mouse_precision_off", "usb_suspend_off", "adapter_power_saving_off", "delivery_optimization_off"];

export function BestTweaks() {
  const { t, td } = useT();
  const tweaks = useApp((s) => s.tweaks);
  const setTweak = useApp((s) => s.setTweak);
  const toast = useApp((s) => s.toast);
  const [busy, setBusy] = useState<string | null>(null);
  const list = BEST.map((id) => tweaks.find((tw) => tw.id === id)).filter((tw): tw is TweakInfo => !!tw && tw.state !== "unavailable");
  // Met één klik alleen de veilige: HAGS vraagt een herstart en werkt niet op elke pc even goed.
  const todo = list.filter((tw) => tw.state !== "on" && tw.risk === "low");
  if (list.length === 0) return null;

  const applyAll = async () => {
    let done = 0;
    for (const tw of todo) {
      setBusy(tw.id);
      if (await setTweak(tw.id, true, null)) done++;
    }
    setBusy(null);
    toast({ level: "success", title: t("notice.success"), message: t("tweaks.bestDone", { count: String(done) }) });
  };

  return (
    <section>
      <SectionTitle sub={t("tweaks.bestSub")}>{t("tweaks.bestTitle")}</SectionTitle>
      <Card>
        <ul className="divide-y divide-line">
          {list.map((tw, i) => (
            <li key={tw.id} className="flex items-center gap-3 py-2.5 first:pt-0">
              <span className="w-5 text-center text-[13px] font-semibold tnum text-ink-3">{i + 1}</span>
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="text-sm font-medium">{td(`tweak.${tw.id}.name` as TKey)}</span>
                  <GainBadge gain={tw.gain} />
                  <TimingBadge timing={tw.timing} />
                </div>
                <div className="mt-0.5 truncate text-[12px] text-ink-3">{td(`tweak.${tw.id}.effect` as TKey)}</div>
              </div>
              <Toggle
                checked={tw.state === "on"}
                loading={busy === tw.id}
                label={td(`tweak.${tw.id}.name` as TKey)}
                onChange={async (v) => {
                  setBusy(tw.id);
                  await setTweak(tw.id, v, null);
                  setBusy(null);
                }}
              />
            </li>
          ))}
        </ul>
        <div className="mt-3 flex flex-wrap items-center gap-3 border-t border-line pt-3">
          {todo.length === 0 ? (
            <Badge variant="ok" icon={CheckCircle2}>
              {t("tweaks.bestAllOn")}
            </Badge>
          ) : (
            <Button variant="primary" icon={Rocket} loading={busy !== null} onClick={() => void applyAll()}>
              {busy ? t("tweaks.bestBusy", { name: td(`tweak.${busy}.name` as TKey) }) : t("tweaks.bestApply", { count: String(todo.length) })}
            </Button>
          )}
          <span className={cn("text-[12px] text-ink-3")}>{t("tweaks.bestNote")}</span>
        </div>
      </Card>
    </section>
  );
}
