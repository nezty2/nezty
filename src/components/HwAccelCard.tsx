import { Cpu } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useT } from "@/i18n/hooks";
import { api } from "@/lib/ipc";
import type { HwAccelInfo } from "@/lib/types";
import { useApp } from "@/store/app";
import { Badge, Card, SectionTitle, Toggle, cn } from "./ui";

/**
 * Hardwareversnelling van Discord en browsers: uit betekent minder GPU- en VRAM-gebruik op de achtergrond
 * tijdens het gamen. Elke wijziging maakt een back-up en komt in het logboek.
 */
export function HwAccelCard({ refreshTrigger }: { refreshTrigger?: unknown }) {
  const { t } = useT();
  const handleError = useApp((s) => s.handleError);
  const refreshJournal = useApp((s) => s.refreshJournal);
  const toast = useApp((s) => s.toast);
  const [list, setList] = useState<HwAccelInfo[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(() => {
    api.listHwAccel().then(setList).catch(handleError);
  }, [handleError]);

  useEffect(() => {
    load();
  }, [load, refreshTrigger]);

  if (!list || list.length === 0) return null;

  return (
    <section>
      <SectionTitle sub={t("background.hwAccelSub")}>{t("background.hwAccelTitle")}</SectionTitle>
      <Card className="!py-1">
        <ul className="divide-y divide-line">
          {list.map((h) => {
            const off = !h.enabled;
            return (
              <li key={h.id} className="flex items-start gap-3 py-3">
                <Cpu className={cn("mt-0.5 h-4 w-4 shrink-0", off ? "text-ok" : "text-ink-3")} />
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="text-sm font-medium">{h.name}</span>
                    <Badge variant={off ? "ok" : "neutral"}>{off ? t("background.hwAccelBadgeOff") : t("background.hwAccelBadgeOn")}</Badge>
                  </div>
                  <div className="mt-0.5 text-[13px] text-ink-2">{off ? t("background.hwAccelOffDesc") : t("background.hwAccelOnDesc")}</div>
                  {h.running && <div className="mt-0.5 text-[12px] text-warn">{t("background.hwAccelRunning", { app: h.name })}</div>}
                </div>
                <Toggle
                  checked={off}
                  loading={busy === h.id}
                  label={`${h.name}: ${t("background.hwAccelTitle")}`}
                  onChange={async (turnOff) => {
                    setBusy(h.id);
                    try {
                      const next = await api.setHwAccel(h.id, !turnOff);
                      setList(next);
                      toast({
                        level: "success",
                        title: t("notice.success"),
                        message: turnOff ? t("background.hwAccelSavedOff", { app: h.name }) : t("background.hwAccelSavedOn", { app: h.name }),
                      });
                      void refreshJournal();
                    } catch (e) {
                      handleError(e);
                    } finally {
                      setBusy(null);
                    }
                  }}
                />
              </li>
            );
          })}
        </ul>
      </Card>
    </section>
  );
}
