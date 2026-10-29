import { Laptop, Lock, Settings, ShieldCheck } from "lucide-react";
import { useState } from "react";
import { LaptopStatus } from "@/components/LaptopStatus";
import { TweakCard } from "@/components/TweakCard";
import { Button, Card, PageHeader, Row, SectionTitle, Skeleton, Toggle } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import type { TweakInfo } from "@/lib/types";
import { useApp } from "@/store/app";
import { BigWinCard, LAPTOP_CHECKS } from "./Tweaks";

/** Alles voor laptops op één plek: status, tweaks, controles en de lader-instelling. */
export function LaptopPage() {
  const { t, td } = useT();
  const device = useApp((s) => s.device);
  const tweaks = useApp((s) => s.tweaks);
  const loading = useApp((s) => s.tweaksLoading);
  const bigWins = useApp((s) => s.bigWins);
  const settings = useApp((s) => s.settings);
  const appInfo = useApp((s) => s.appInfo);
  const setTweak = useApp((s) => s.setTweak);
  const updateSettings = useApp((s) => s.updateSettings);
  const setPage = useApp((s) => s.setPage);
  const toast = useApp((s) => s.toast);
  const [busy, setBusy] = useState<string | null>(null);

  const isLaptop = device?.effective === "laptop";
  const list = tweaks.filter((tw) => tw.category === "laptop");
  // Laptopcontroles, plus lader en snelle videokaart: die horen op een laptop er ook bij.
  const checkIds = ["power_source", ...LAPTOP_CHECKS, "gpu_choice"];
  const order = { issue: 0, info: 1, ok: 2, unavailable: 3 };
  const checks = (bigWins ?? []).filter((w) => checkIds.includes(w.id)).sort((a, b) => order[a.status] - order[b.status]);
  const model = [device?.manufacturer, device?.model].filter((x) => x && x.trim()).join(" ");

  const onToggle = async (tw: TweakInfo, enabled: boolean, option: string | null) => {
    setBusy(tw.id);
    const info = await setTweak(tw.id, enabled, option);
    setBusy(null);
    if (info) {
      const name = td(`tweak.${tw.id}.name`);
      toast({ level: "success", title: t(`timing.${tw.timing}`), message: enabled ? t("tweakCard.applied", { name }) : t("tweakCard.disabled", { name }) });
    }
  };

  return (
    <div className="space-y-8">
      <PageHeader
        title={t("laptop.title")}
        subtitle={
          <span className="inline-flex items-center gap-2">
            <Laptop className="h-4 w-4 text-accent" />
            {device ? t("device.label", { kind: t(`device.${device.effective}` as "device.pc") }) : ""}
            {model && <span className="text-ink-3">· {model}</span>}
          </span>
        }
      />

      {device && !isLaptop && (
        <Card className="flex flex-wrap items-center gap-4">
          <Lock className="h-5 w-5 shrink-0 text-ink-3" />
          <div className="min-w-0 flex-1">
            <div className="text-sm font-semibold">{t("laptop.pcTitle")}</div>
            <div className="text-[13px] text-ink-2">{t("laptop.pcText")}</div>
          </div>
          <Button size="sm" icon={Settings} onClick={() => setPage("settings")}>
            {t("laptop.pcButton")}
          </Button>
        </Card>
      )}

      {isLaptop && (
        <section>
          <SectionTitle>{t("laptop.statusTitle")}</SectionTitle>
          <LaptopStatus />
        </section>
      )}

      <section>
        <SectionTitle sub={t("laptop.tweaksSub")}>{t("laptop.tweaksTitle")}</SectionTitle>
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

      {isLaptop && (
        <section>
          <SectionTitle sub={t("laptop.checksSub")}>{t("laptop.checksTitle")}</SectionTitle>
          {bigWins === null ? (
            <Skeleton className="h-48" />
          ) : (
            <div className="grid gap-4 lg:grid-cols-2">
              {checks.map((w) => (
                <BigWinCard key={w.id} win={w} onFixed={(all) => useApp.setState({ bigWins: all })} />
              ))}
            </div>
          )}
        </section>
      )}

      {isLaptop && settings && (
        <section>
          <SectionTitle>{t("laptop.settingsTitle")}</SectionTitle>
          <Card className="!py-1">
            <Row title={t("settings.boostCharger")} desc={t("settings.boostChargerDesc")}>
              <Toggle checked={settings.boostRequiresCharger} onChange={(v) => void updateSettings({ boostRequiresCharger: v })} label={t("settings.boostCharger")} />
            </Row>
          </Card>
        </section>
      )}

      <div className="flex items-center gap-2 px-1 text-[12px] text-ink-3">
        <ShieldCheck className="h-3.5 w-3.5" />
        {t("laptop.safe")}
      </div>
    </div>
  );
}
