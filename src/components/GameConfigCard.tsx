import { CheckCircle2, FileCog, XCircle } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useT } from "@/i18n/hooks";
import type { TKey } from "@/i18n";
import { api } from "@/lib/ipc";
import type { GameConfigCheck } from "@/lib/types";
import { useApp } from "@/store/app";
import { Badge, Button, Card, SectionTitle } from "./ui";

/** Instellingen in het configbestand van een game (alleen games waarvan nezty de sleutels zeker kent). */
export function GameConfigCard({ gameId }: { gameId: string }) {
  const { t } = useT();
  const handleError = useApp((s) => s.handleError);
  const toast = useApp((s) => s.toast);
  const [list, setList] = useState<GameConfigCheck[] | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    api.listGameConfig(gameId).then(setList).catch(() => setList([]));
  }, [gameId]);
  useEffect(load, [load]);

  if (!list || list.length === 0) return null;
  const todo = list.filter((c) => !c.ok).length;

  return (
    <section>
      <SectionTitle sub={t("gameConfig.sub")}>{t("gameConfig.title")}</SectionTitle>
      <Card>
        <ul className="divide-y divide-line">
          {list.map((c) => (
            <li key={c.file + c.id} className="flex items-start gap-3 py-3 first:pt-0">
              {c.ok ? <CheckCircle2 className="h-[18px] w-[18px] shrink-0 text-ok" /> : <XCircle className="h-[18px] w-[18px] shrink-0 text-warn" />}
              <div className="min-w-0 flex-1">
                <div className="text-sm font-medium">{t(`gameConfig.check.${c.id}.name` as TKey)}</div>
                <div className="mt-0.5 text-[13px] text-ink-2">{t(`gameConfig.check.${c.id}.why` as TKey)}</div>
                <div className="mt-0.5 truncate text-[12px] text-ink-3" title={c.file}>
                  {c.file}
                </div>
              </div>
              <div className="shrink-0 text-right text-[13px]">{c.ok ? t("fn.off") : t("fn.on")}</div>
            </li>
          ))}
        </ul>
        <div className="mt-3 flex flex-wrap items-center gap-3 border-t border-line pt-3">
          {todo === 0 ? (
            <Badge variant="ok" icon={CheckCircle2}>
              {t("fn.allGood")}
            </Badge>
          ) : (
            <Button
              variant="primary"
              icon={FileCog}
              loading={busy}
              onClick={async () => {
                setBusy(true);
                try {
                  setList(await api.applyGameConfig(gameId));
                  toast({ level: "success", title: t("notice.success"), message: t("gameConfig.applied") });
                } catch (e) {
                  handleError(e);
                } finally {
                  setBusy(false);
                }
              }}
            >
              {t("gameConfig.apply")}
            </Button>
          )}
          <span className="text-[12px] text-ink-3">{t("gameConfig.note")}</span>
        </div>
      </Card>
    </section>
  );
}
