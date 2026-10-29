import { KeyRound, Lock, WifiOff } from "lucide-react";
import { cn } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import { clock, isLocked, parts, progress, remainingMs, tone } from "@/lib/license";
import type { LicenseStatus } from "@/lib/types";
import { useApp } from "@/store/app";
import { useServerNow } from "./LicenseParts";

/** Resterende tijd in gewone taal: "Nog 23 dagen en 4 uur" of "Demo: nog 1:42:10". */
export function useRemainingText() {
  const { t } = useT();
  return (l: LicenseStatus, now: number): string => {
    if (l.mode === "revoked") return t("license.revoked");
    if (l.mode === "expired") return t("license.expired");
    if (l.mode === "offline") return t("license.offline");
    const left = remainingMs(l, now);
    if (l.kind === "demo") return t("license.demoLeft", { clock: clock(left) });
    const p = parts(left);
    if (p.days > 0) return t(p.hours > 0 ? "license.leftDaysHours" : "license.leftDays", { days: p.days, hours: p.hours });
    return t("license.leftHoursMinutes", { hours: p.hours, minutes: p.minutes });
  };
}

const toneText = { normal: "text-accent", warn: "text-warn", bad: "text-bad" };
const toneBar = { normal: "bg-accent", warn: "bg-warn", bad: "bg-bad" };

/** Vast blok bovenin de zijbalk. Klik opent de licentiepagina. */
export function LicenseBlock() {
  const { t } = useT();
  const license = useApp((s) => s.license);
  const page = useApp((s) => s.page);
  const setPage = useApp((s) => s.setPage);
  const now = useServerNow(license?.kind === "demo" ? 1000 : 30_000);
  const text = useRemainingText();
  if (!license || license.mode === "off" || license.mode === "signed_out") return null;
  const tn = tone(license, now);
  const Icon = license.mode === "offline" ? WifiOff : isLocked(license) ? Lock : KeyRound;
  const pct = isLocked(license) ? 1 : progress(license, now);

  return (
    <div className="px-2.5 pb-1">
      <button
        type="button"
        onClick={() => setPage("license")}
        title={t("license.open")}
        className={cn("w-full rounded-lg border px-3 py-2.5 text-left transition-colors", page === "license" ? "border-accent/40 bg-accent-soft" : "border-line bg-panel-2/60 hover:border-line-2")}
      >
        <div className="flex items-center gap-2">
          <Icon className={cn("h-4 w-4 shrink-0", toneText[tn])} />
          <div className="hidden min-w-0 flex-1 min-[1120px]:block">
            <div className="text-[11px] uppercase tracking-wide text-ink-3">{license.kind === "demo" ? t("license.typeDemo") : t("license.typeMonth")}</div>
            <div className={cn("truncate text-[13px] font-semibold tnum", tn === "normal" ? "text-ink" : toneText[tn])}>{text(license, now)}</div>
          </div>
        </div>
        <div className="mt-2 hidden h-1 overflow-hidden rounded-full bg-line min-[1120px]:block">
          <div className={cn("h-full rounded-full transition-[width] duration-700", toneBar[tn])} style={{ width: `${Math.round(pct * 100)}%` }} />
        </div>
      </button>
    </div>
  );
}
