import { Info } from "lucide-react";
import { useState } from "react";
import { LiveInputCard } from "@/components/LiveInputCard";
import { TweakCard } from "@/components/TweakCard";
import { Card, PageHeader, SectionTitle, Skeleton } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import type { TweakInfo } from "@/lib/types";
import { useApp } from "@/store/app";

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
