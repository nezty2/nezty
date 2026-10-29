import { ArrowRight, Download, History, Undo2 } from "lucide-react";
import { useMemo, useState } from "react";
import { Badge, Button, Card, Chip, EmptyState, Modal, PageHeader } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import { api } from "@/lib/ipc";
import { formatDayLong, formatTime, isToday, isYesterday } from "@/lib/format";
import type { JournalEntry } from "@/lib/types";
import { useApp } from "@/store/app";

type Filter = "all" | "active" | "reverted";

export function LogPage() {
  const { t, td, lang } = useT();
  const journal = useApp((s) => s.journal);
  const refreshJournal = useApp((s) => s.refreshJournal);
  const refreshTweaks = useApp((s) => s.refreshTweaks);
  const games = useApp((s) => s.games);
  const handleError = useApp((s) => s.handleError);
  const toast = useApp((s) => s.toast);
  const [filter, setFilter] = useState<Filter>("all");
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);

  const active = journal.filter((e) => !e.reverted && e.revertable);
  const filtered = useMemo(() => journal.filter((e) => (filter === "all" ? true : filter === "active" ? !e.reverted && e.revertable : e.reverted)), [journal, filter]);
  const groups = useMemo(() => {
    const map = new Map<string, JournalEntry[]>();
    for (const e of filtered) {
      const day = new Date(e.ts).toDateString();
      map.set(day, [...(map.get(day) ?? []), e]);
    }
    return Array.from(map.values());
  }, [filtered]);

  const dayLabel = (ts: number) => (isToday(ts) ? t("log.today") : isYesterday(ts) ? t("log.yesterday") : formatDayLong(lang, ts));
  const value = (v: string | null) => {
    if (!v) return "-";
    if (v.startsWith("@")) return td(`log.values.${v.slice(1)}`, undefined, v);
    return v;
  };
  const gameName = (id: string | null) => (id ? (games.find((g) => g.id === id)?.name ?? id) : null);

  const revertAll = async () => {
    setBusy("all");
    try {
      const r = await api.revertAll();
      toast({ level: r.failed.length ? "warning" : "success", title: t("notice.success"), message: t("log.revertedAll", { count: r.reverted }) });
      setConfirm(false);
      void refreshJournal();
      void refreshTweaks();
    } catch (e) {
      handleError(e);
    } finally {
      setBusy(null);
    }
  };

  const revertOne = async (e: JournalEntry) => {
    setBusy(e.id);
    try {
      const r = await api.revertEntry(e.id);
      if (r.failed.length) toast({ level: "error", title: t("notice.error"), message: t("msg.revert_failed", { target: e.target }), detail: r.failed[0].error });
      else toast({ level: "success", title: t("notice.success"), message: t("log.revertedOne") });
      void refreshJournal();
      void refreshTweaks();
    } catch (err) {
      handleError(err);
    } finally {
      setBusy(null);
    }
  };

  const exportLog = async () => {
    setExporting(true);
    try {
      const path = await api.exportJournal();
      toast({ level: "success", title: t("notice.success"), message: t("log.exported"), action: { label: t("log.openFolder"), onClick: () => void api.openPath(path.replace(/[\\/][^\\/]+$/, "")) } });
    } catch (e) {
      handleError(e);
    } finally {
      setExporting(false);
    }
  };

  return (
    <div className="space-y-6">
      <PageHeader
        title={t("log.title")}
        subtitle={t("log.subtitle", { count: active.length })}
        actions={
          <>
            <Button icon={Download} onClick={() => void exportLog()} loading={exporting}>
              {t("log.export")}
            </Button>
            <Button variant="danger" icon={Undo2} onClick={() => setConfirm(true)} disabled={active.length === 0}>
              {t("log.revertAll", { count: active.length })}
            </Button>
          </>
        }
      />

      <div className="flex gap-1.5">
        <Chip active={filter === "all"} onClick={() => setFilter("all")}>
          {t("log.filterAll", { count: journal.length })}
        </Chip>
        <Chip active={filter === "active"} onClick={() => setFilter("active")}>
          {t("log.filterActive", { count: active.length })}
        </Chip>
        <Chip active={filter === "reverted"} onClick={() => setFilter("reverted")}>
          {t("log.filterReverted", { count: journal.filter((e) => e.reverted).length })}
        </Chip>
      </div>

      {groups.length === 0 ? (
        <EmptyState icon={History} title={t("log.empty")} description={t("log.emptyDesc")} />
      ) : (
        groups.map((entries) => (
          <div key={entries[0].ts}>
            <div className="mb-2 text-[13px] font-semibold text-ink-2">{dayLabel(entries[0].ts)}</div>
            <Card className="!p-0">
              <ul className="divide-y divide-line">
                {entries.map((e) => (
                  <li key={e.id} className={`grid grid-cols-[64px_minmax(0,1fr)_auto] items-start gap-4 px-5 py-3 ${e.reverted ? "opacity-60" : ""}`}>
                    <div className="pt-0.5 font-mono text-[12px] text-ink-3 tnum">{formatTime(lang, e.ts)}</div>
                    <div className="min-w-0">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="text-sm font-medium">{td(`tweak.${e.tweakId}.name`, undefined, e.tweakName)}</span>
                        {e.reverted ? <Badge variant="info">{t("log.reverted")}</Badge> : e.revertable ? <Badge variant="ok">{t("log.active")}</Badge> : <Badge variant="neutral">{t("log.notRevertable")}</Badge>}
                        {e.sessionId && <Badge variant="accent">{t("log.boost")}</Badge>}
                        {e.gameId && <Badge variant="neutral">{gameName(e.gameId)}</Badge>}
                      </div>
                      <div className="selectable mt-1 truncate font-mono text-[11px] text-ink-3" title={e.target}>
                        {e.target}
                      </div>
                      {e.detail ? (
                        <div className="mt-1 text-[13px] text-ink-2">{td(`msg.${e.detail.key}`, e.detail.params)}</div>
                      ) : (
                        (e.oldValue || e.newValue) && (
                          <div className="mt-1.5 flex flex-wrap items-center gap-2 text-[12px]">
                            <span className="selectable rounded bg-panel-2 px-1.5 py-0.5 font-mono text-ink-2">{value(e.oldValue)}</span>
                            <ArrowRight className="h-3 w-3 text-ink-3" />
                            <span className="selectable rounded bg-accent-soft px-1.5 py-0.5 font-mono text-ink">{value(e.newValue)}</span>
                          </div>
                        )
                      )}
                      {e.note && <div className="mt-1 text-[12px] italic text-ink-3">{td(`msg.${e.note}`, undefined, e.note)}</div>}
                    </div>
                    {!e.reverted && e.revertable ? (
                      <Button size="sm" variant="ghost" icon={Undo2} loading={busy === e.id} onClick={() => void revertOne(e)}>
                        {t("log.revert")}
                      </Button>
                    ) : (
                      <span />
                    )}
                  </li>
                ))}
              </ul>
            </Card>
          </div>
        ))
      )}

      <Modal
        open={confirm}
        onClose={() => setConfirm(false)}
        title={t("log.confirmTitle")}
        subtitle={t("log.confirmBody", { count: active.length })}
        width="max-w-md"
        footer={
          <>
            <Button variant="ghost" onClick={() => setConfirm(false)}>
              {t("common.cancel")}
            </Button>
            <Button variant="danger" icon={Undo2} loading={busy === "all"} onClick={() => void revertAll()}>
              {t("log.confirm")}
            </Button>
          </>
        }
      >
        <ul className="scroll-thin max-h-64 space-y-1 overflow-y-auto text-[13px] text-ink-2">
          {active.slice(0, 60).map((e) => (
            <li key={e.id} className="truncate">
              {td(`tweak.${e.tweakId}.name`, undefined, e.tweakName)}: <span className="text-ink-3">{e.target}</span>
            </li>
          ))}
        </ul>
        <p className="mt-3 text-[12px] text-ink-3">{t("log.confirmNote")}</p>
      </Modal>
    </div>
  );
}
