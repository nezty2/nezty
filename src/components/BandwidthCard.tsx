import { Activity, ArrowDown, ArrowUp } from "lucide-react";
import { useState } from "react";
import { useT } from "@/i18n/hooks";
import type { TKey } from "@/i18n";
import { api } from "@/lib/ipc";
import type { BandwidthReport } from "@/lib/types";
import { useApp } from "@/store/app";
import { Button, Card, SectionTitle } from "./ui";

function rate(kbps: number, lang: string): string {
  return kbps >= 1000 ? `${(kbps / 1000).toLocaleString(lang, { maximumFractionDigits: 1 })} Mbit/s` : `${Math.round(kbps)} kbit/s`;
}

/** Meet een paar seconden welke programma's je internet gebruiken, met per bekend programma wat je kunt doen. */
export function BandwidthCard() {
  const { t, lang } = useT();
  const handleError = useApp((s) => s.handleError);
  const setTweak = useApp((s) => s.setTweak);
  const tweaks = useApp((s) => s.tweaks);
  const [report, setReport] = useState<BandwidthReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [action, setAction] = useState<string | null>(null);
  const doOff = tweaks.find((tw) => tw.id === "delivery_optimization_off")?.state === "on";

  const measure = async () => {
    setBusy(true);
    try {
      setReport(await api.measureBandwidth(3));
    } catch (e) {
      handleError(e);
    } finally {
      setBusy(false);
    }
  };

  const fix = (label: string | null) => {
    if (label === "delivery_optimization" && !doOff) {
      return (
        <Button
          size="sm"
          loading={action === label}
          onClick={async () => {
            setAction(label);
            await setTweak("delivery_optimization_off", true);
            setAction(null);
          }}
        >
          {t("bandwidth.doOff")}
        </Button>
      );
    }
    if (label === "windows_update") {
      return (
        <Button size="sm" onClick={() => void api.openUrl("ms-settings:windowsupdate").catch(handleError)}>
          {t("bandwidth.openUpdate")}
        </Button>
      );
    }
    if (label === "onedrive") {
      return (
        <Button
          size="sm"
          loading={action === label}
          onClick={async () => {
            setAction(label);
            try {
              await api.closeApps(["OneDrive.exe"]);
              await measure();
            } catch (e) {
              handleError(e);
            } finally {
              setAction(null);
            }
          }}
        >
          {t("bandwidth.closeOneDrive")}
        </Button>
      );
    }
    return null;
  };

  return (
    <section>
      <SectionTitle sub={t("bandwidth.sub")}>{t("bandwidth.title")}</SectionTitle>
      <Card>
        <div className="flex flex-wrap items-center gap-3">
          <Button icon={Activity} loading={busy} onClick={() => void measure()}>
            {report ? t("bandwidth.again") : t("bandwidth.measure")}
          </Button>
          {report && (
            <span className="text-[13px] text-ink-2">
              {t("bandwidth.total", { down: rate(report.totalDownKbps, lang), up: rate(report.totalUpKbps, lang) })}
            </span>
          )}
        </div>
        {report && !report.elevated && <p className="mt-3 text-[13px] text-warn">{t("bandwidth.needsAdmin")}</p>}
        {report && report.elevated && report.users.length === 0 && <p className="mt-3 text-[13px] text-ok">{t("bandwidth.quiet")}</p>}
        {report && report.users.length > 0 && (
          <ul className="mt-3 divide-y divide-line">
            {report.users.map((u) => (
              <li key={u.name + (u.label ?? "")} className="flex items-start gap-3 py-2.5">
                <div className="min-w-0 flex-1">
                  <div className="text-sm font-medium">{u.label ? t(`bandwidth.label.${u.label}.name` as TKey) : u.name}</div>
                  <div className="mt-0.5 text-[13px] text-ink-2">{u.label ? t(`bandwidth.label.${u.label}.tip` as TKey) : t("bandwidth.unknownTip")}</div>
                </div>
                <div className="shrink-0 text-right text-[12px] tnum text-ink-2">
                  <div className="flex items-center justify-end gap-1">
                    <ArrowDown className="h-3 w-3" />
                    {rate(u.downKbps, lang)}
                  </div>
                  <div className="flex items-center justify-end gap-1">
                    <ArrowUp className="h-3 w-3" />
                    {rate(u.upKbps, lang)}
                  </div>
                </div>
                <div className="shrink-0">{fix(u.label)}</div>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </section>
  );
}
