import { BatteryCharging, BatteryWarning, Flame, Monitor } from "lucide-react";
import { useT } from "@/i18n/hooks";
import { formatPct, formatTemp } from "@/lib/format";
import { StatTile } from "@/pages/Dashboard";
import { useApp } from "@/store/app";

const shortGpu = (name: string | null | undefined) => (name ?? "").replace(/\((R|TM)\)/gi, "").replace(/\s+/g, " ").trim();

/** Alleen op een laptop: lader, batterij, hitte en welke videokaart het scherm aanstuurt. */
export function LaptopStatus() {
  const { t, lang } = useT();
  const metrics = useApp((s) => s.metrics);
  const device = useApp((s) => s.device);
  const latest = metrics.latest;
  const h = metrics.history;
  const temps = [latest?.cpuTempC != null ? `CPU ${formatTemp(lang, latest.cpuTempC)}` : null, latest?.gpuTempC != null ? `GPU ${formatTemp(lang, latest.gpuTempC)}` : null].filter(Boolean).join(" · ");
  const throttle = latest?.thermalThrottle ?? null;
  const fast = shortGpu(device?.fastGpu);
  const display = shortGpu(device?.displayGpu) || "-";
  const fastSub = device?.hybrid && fast ? (latest?.gpuPct != null && latest.gpuPct >= 5 ? t("dashboard.fastBusy", { gpu: fast, pct: formatPct(lang, latest.gpuPct) }) : t("dashboard.fastIdle", { gpu: fast })) : t("dashboard.displayVia");
  return (
    <div className="space-y-3">
      {latest?.onAc === false && (
        <div className="flex items-center gap-3 rounded-lg border border-warn/40 bg-warn/10 px-4 py-3 text-[13px]">
          <BatteryWarning className="h-5 w-5 shrink-0 text-warn" />
          <div>
            <div className="font-semibold text-ink">{t("dashboard.chargerBanner")}</div>
            <div className="text-ink-2">{t("dashboard.chargerBannerSub")}</div>
          </div>
        </div>
      )}
      <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
        <StatTile
          icon={BatteryCharging}
          label={t("dashboard.battery")}
          value={latest?.batteryPct != null ? `${latest.batteryPct}%` : "-"}
          sub={latest?.onAc == null ? "" : latest.onAc ? t("dashboard.charging") : t("dashboard.onBattery")}
          series={h.map((s) => s.batteryPct)}
          min={0}
          max={100}
          format={(v) => `${Math.round(v)}%`}
          tone={latest?.onAc === false ? "warn" : "normal"}
        />
        <StatTile
          icon={Flame}
          label={t("dashboard.heat")}
          value={temps || "-"}
          sub={throttle == null ? t("dashboard.heatUnknown") : throttle ? t("dashboard.heatThrottle") : t("dashboard.heatOk")}
          series={h.map((s) => s.cpuTempC ?? s.gpuTempC)}
          min={20}
          max={100}
          format={(v) => formatTemp(lang, v)}
          tone={throttle ? "bad" : "normal"}
        />
        <StatTile icon={Monitor} label={t("dashboard.gpuActive")} value={display} sub={fastSub} series={h.map((s) => s.gpuPct)} min={0} max={100} format={(v) => formatPct(lang, v)} />
      </div>
    </div>
  );
}

