import { Layers } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useT } from "@/i18n/hooks";
import type { TKey } from "@/i18n";
import { api } from "@/lib/ipc";
import type { OverlayInfo } from "@/lib/types";
import { useApp } from "@/store/app";
import { Button, Card, SectionTitle, Toggle, cn } from "./ui";

/** Overlays die over elke game tekenen: Xbox Game Bar met één klik uit, de rest met uitleg of sluiten. */
export function OverlaysCard() {
  const { t } = useT();
  const handleError = useApp((s) => s.handleError);
  const setTweak = useApp((s) => s.setTweak);
  const [list, setList] = useState<OverlayInfo[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(() => {
    api.listOverlays().then(setList).catch(handleError);
  }, [handleError]);
  useEffect(load, [load]);

  return (
    <section>
      <SectionTitle sub={t("fn.overlaySub")}>{t("fn.overlayTitle")}</SectionTitle>
      <Card className="!py-1">
        <ul className="divide-y divide-line">
          {(list ?? []).map((o) => (
            <li key={o.id} className="flex items-start gap-3 py-3">
              <Layers className={cn("mt-0.5 h-4 w-4 shrink-0", o.active ? "text-warn" : "text-ok")} />
              <div className="min-w-0 flex-1">
                <div className="text-sm font-medium">{t(`fn.overlay.${o.id}.name` as TKey)}</div>
                <div className="mt-0.5 text-[13px] text-ink-2">{o.active ? t(`fn.overlay.${o.id}.how` as TKey) : t("fn.overlayOff")}</div>
              </div>
              {o.fix === "tweak" && (
                <Toggle
                  checked={!o.active}
                  loading={busy === o.id}
                  label={t("fn.overlayToggle")}
                  onChange={async (v) => {
                    setBusy(o.id);
                    await setTweak("game_bar_dvr_off", v);
                    setBusy(null);
                    load();
                  }}
                />
              )}
              {o.fix === "close" && o.active && (
                <Button
                  size="sm"
                  loading={busy === o.id}
                  onClick={async () => {
                    setBusy(o.id);
                    try {
                      setList(await api.closeOverlay(o.id));
                    } catch (e) {
                      handleError(e);
                    } finally {
                      setBusy(null);
                    }
                  }}
                >
                  {t("fn.close")}
                </Button>
              )}
            </li>
          ))}
        </ul>
      </Card>
    </section>
  );
}
