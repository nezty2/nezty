import { CheckCircle2, RefreshCw, ShieldCheck, XCircle } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useT } from "@/i18n/hooks";
import type { TKey } from "@/i18n";
import { api } from "@/lib/ipc";
import type { LiveCheck } from "@/lib/types";
import { useApp } from "@/store/app";
import { Button, Card } from "./ui";

/** Controleert of aangezette muis- en toetsenbordtweaks ook echt actief zijn in Windows. */
export function LiveInputCard() {
  const { t, td } = useT();
  const tweaks = useApp((s) => s.tweaks);
  const handleError = useApp((s) => s.handleError);
  const [list, setList] = useState<LiveCheck[] | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    api.verifyInputLive().then(setList).catch(handleError);
  }, [handleError]);
  // Opnieuw controleren als een tweak verandert.
  useEffect(load, [load, tweaks]);

  const saved = (list ?? []).filter((c) => c.saved);
  const off = saved.filter((c) => !c.live);
  if (!list || saved.length === 0) return null;

  return (
    <Card>
      <div className="flex items-start gap-3">
        <ShieldCheck className={off.length ? "mt-0.5 h-5 w-5 shrink-0 text-warn" : "mt-0.5 h-5 w-5 shrink-0 text-ok"} />
        <div className="min-w-0 flex-1">
          <div className="text-sm font-semibold">{off.length ? t("live.mismatch", { count: String(off.length) }) : t("live.ok", { count: String(saved.length) })}</div>
          <ul className="mt-2 space-y-1">
            {saved.map((c) => (
              <li key={c.id} className="flex items-center gap-2 text-[13px] text-ink-2">
                {c.live ? <CheckCircle2 className="h-4 w-4 text-ok" /> : <XCircle className="h-4 w-4 text-warn" />}
                {td(`tweak.${c.id}.name` as TKey)}
                <span className="text-ink-3">{c.live ? t("live.active") : t("live.notActive")}</span>
              </li>
            ))}
          </ul>
          {off.length > 0 && (
            <Button
              size="sm"
              icon={RefreshCw}
              className="mt-3"
              loading={busy}
              onClick={async () => {
                setBusy(true);
                try {
                  setList(await api.syncInputLive());
                } catch (e) {
                  handleError(e);
                } finally {
                  setBusy(false);
                }
              }}
            >
              {t("live.fix")}
            </Button>
          )}
        </div>
      </div>
    </Card>
  );
}
