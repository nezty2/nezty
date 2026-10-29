import { KeyRound, LogOut, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { useRemainingText } from "@/components/license/LicenseBlock";
import { ContactInfo, useServerNow } from "@/components/license/LicenseParts";
import { Button, Card, PageHeader, SectionTitle, cn } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import { api } from "@/lib/ipc";
import { isLocked, progress, tone } from "@/lib/license";
import { useApp } from "@/store/app";

/** Licentiepagina: type, einddatum, laatste 4 tekens, verlengen en uitloggen. */
export function LicensePage() {
  const { t, lang } = useT();
  const license = useApp((s) => s.license);
  const config = useApp((s) => s.licenseConfig);
  const loadConfig = useApp((s) => s.loadLicenseConfig);
  const setLicense = useApp((s) => s.setLicense);
  const handleError = useApp((s) => s.handleError);
  const [renew, setRenew] = useState(false);
  const [confirmLogout, setConfirmLogout] = useState(false);
  const [busy, setBusy] = useState<null | "refresh" | "logout">(null);
  const now = useServerNow(license?.kind === "demo" ? 1000 : 30_000);
  const text = useRemainingText();

  useEffect(() => {
    void loadConfig();
  }, [loadConfig]);

  if (!license || license.mode === "off" || license.mode === "signed_out") return null;
  const tn = tone(license, now);
  const fmt = (ms: number | null) => (ms == null ? "-" : new Date(ms).toLocaleString(lang, { day: "numeric", month: "long", year: "numeric", hour: "2-digit", minute: "2-digit" }));

  const recheck = async () => {
    setBusy("refresh");
    try {
      setLicense(await api.licenseRefresh());
    } catch (e) {
      handleError(e);
    } finally {
      setBusy(null);
    }
  };
  const logout = async () => {
    setBusy("logout");
    try {
      setLicense(await api.licenseLogout());
    } catch (e) {
      handleError(e);
    } finally {
      setBusy(null);
    }
  };

  const rows: [string, string][] = [
    [t("license.fieldType"), license.kind === "demo" ? t("license.typeDemo") : t("license.typeMonth")],
    [t("license.fieldUser"), license.username ?? "-"],
    [t("license.fieldKey"), license.kind === "demo" ? "-" : `•••• ${license.keyHint ?? ""}`],
    [t("license.fieldStart"), fmt(license.activatedAt)],
    [t("license.fieldEnd"), fmt(license.expiresAt)],
  ];

  return (
    <div className="space-y-6">
      <PageHeader title={t("license.title")} subtitle={t("license.subtitle")} />

      <Card className="space-y-5 p-6">
        <div className="flex flex-wrap items-center gap-4">
          <div className="flex h-11 w-11 items-center justify-center rounded-xl bg-accent-soft">
            <KeyRound className={cn("h-5 w-5", tn === "normal" ? "text-accent" : tn === "warn" ? "text-warn" : "text-bad")} />
          </div>
          <div className="min-w-0 flex-1">
            <div className={cn("text-xl font-semibold tnum", tn === "warn" && "text-warn", tn === "bad" && "text-bad")}>{text(license, now)}</div>
            <div className="text-[13px] text-ink-3">{t(`license.state.${license.mode}` as "license.state.active")}</div>
          </div>
        </div>
        <div className="h-1.5 overflow-hidden rounded-full bg-line">
          <div className={cn("h-full rounded-full", tn === "normal" ? "bg-accent" : tn === "warn" ? "bg-warn" : "bg-bad")} style={{ width: `${Math.round((isLocked(license) ? 1 : progress(license, now)) * 100)}%` }} />
        </div>
        <dl className="grid gap-x-6 gap-y-3 sm:grid-cols-2">
          {rows.map(([k, v]) => (
            <div key={k}>
              <dt className="text-[12px] text-ink-3">{k}</dt>
              <dd className="selectable text-sm font-medium">{v}</dd>
            </div>
          ))}
        </dl>
        <div className="flex flex-wrap gap-2 border-t border-line pt-4">
          <Button variant="primary" onClick={() => setRenew((v) => !v)}>
            {t("license.renew")}
          </Button>
          <Button icon={RefreshCw} loading={busy === "refresh"} onClick={() => void recheck()}>
            {t("license.recheck")}
          </Button>
          {confirmLogout ? (
            <>
              <Button variant="ghost" icon={LogOut} loading={busy === "logout"} onClick={() => void logout()}>
                {t("license.logoutConfirm")}
              </Button>
              <Button variant="ghost" onClick={() => setConfirmLogout(false)}>
                {t("common.cancel")}
              </Button>
            </>
          ) : (
            <Button variant="ghost" icon={LogOut} onClick={() => setConfirmLogout(true)}>
              {t("license.logout")}
            </Button>
          )}
        </div>
      </Card>

      {renew && (
        <section>
          <SectionTitle sub={t("license.renewText")}>{t("license.renew")}</SectionTitle>
          <Card>
            <ContactInfo config={config} />
          </Card>
        </section>
      )}

      <p className="px-1 text-[12px] text-ink-3">{t("license.note")}</p>
    </div>
  );
}
