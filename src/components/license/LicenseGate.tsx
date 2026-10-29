import { Lock, RefreshCw, RotateCcw, WifiOff } from "lucide-react";
import { useState } from "react";
import { Button, Card } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import { api } from "@/lib/ipc";
import { useApp } from "@/store/app";
import { ContactInfo } from "./LicenseParts";

/**
 * Vergrendelde pagina bij een verlopen, ingetrokken of te lang offline licentie.
 * "Alles terugzetten" (op de pagina Logboek) blijft altijd bereikbaar.
 */
export function LicenseGate() {
  const { t } = useT();
  const license = useApp((s) => s.license);
  const config = useApp((s) => s.licenseConfig);
  const setPage = useApp((s) => s.setPage);
  const setLicense = useApp((s) => s.setLicense);
  const handleError = useApp((s) => s.handleError);
  const [busy, setBusy] = useState(false);
  const [contact, setContact] = useState(false);
  const offline = license?.mode === "offline";

  const recheck = async () => {
    setBusy(true);
    try {
      setLicense(await api.licenseRefresh());
    } catch (e) {
      handleError(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="mx-auto max-w-xl py-10">
      <Card className="space-y-4 p-6 text-center">
        <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-xl bg-bad/10">{offline ? <WifiOff className="h-6 w-6 text-bad" /> : <Lock className="h-6 w-6 text-bad" />}</div>
        <div>
          <h2 className="text-lg font-semibold">{t(`license.gate.${license?.mode === "revoked" ? "revoked" : offline ? "offline" : "expired"}`)}</h2>
          <p className="mt-1.5 text-[13px] leading-relaxed text-ink-2">{offline ? t("license.gate.offlineText") : t("license.gate.text")}</p>
        </div>
        <div className="flex flex-wrap justify-center gap-2">
          {offline ? (
            <Button variant="primary" icon={RefreshCw} loading={busy} onClick={() => void recheck()}>
              {t("license.recheck")}
            </Button>
          ) : (
            <Button variant="primary" onClick={() => setContact((v) => !v)}>
              {t("license.renew")}
            </Button>
          )}
          <Button icon={RotateCcw} onClick={() => setPage("log")}>
            {t("license.gate.revert")}
          </Button>
        </div>
        {contact && (
          <div className="text-left">
            <p className="mb-2 text-[13px] text-ink-2">{t("license.renewText")}</p>
            <ContactInfo config={config} />
          </div>
        )}
        <p className="text-[12px] text-ink-3">{t("license.gate.revertNote")}</p>
      </Card>
    </div>
  );
}
