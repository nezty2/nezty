import { AppWindow, RefreshCw, ShieldCheck, Sparkles, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useAppIcons } from "@/components/useAppIcons";
import { Badge, Button, Card, Modal, PageHeader, Row, Skeleton, Toggle, cn } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import { api } from "@/lib/ipc";
import { formatMb, formatPct } from "@/lib/format";
import type { ProcessInfo } from "@/lib/types";
import { useApp } from "@/store/app";

function AppIcon({ src }: { src: string | null | undefined }) {
  return src ? (
    <img src={src} alt="" className="h-7 w-7 shrink-0" draggable={false} />
  ) : (
    <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-panel-2 text-ink-3">
      <AppWindow className="h-4 w-4" />
    </span>
  );
}

export function BackgroundAppsPage() {
  const { t, lang } = useT();
  const settings = useApp((s) => s.settings);
  const appInfo = useApp((s) => s.appInfo);
  const updateSettings = useApp((s) => s.updateSettings);
  const handleError = useApp((s) => s.handleError);
  const toast = useApp((s) => s.toast);
  const refreshJournal = useApp((s) => s.refreshJournal);
  const [procs, setProcs] = useState<ProcessInfo[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [closing, setClosing] = useState(false);
  const [storeBg, setStoreBg] = useState<boolean | null>(null);
  const icons = useAppIcons((procs ?? []).map((p) => p.exePath));

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setProcs(await api.listProcesses());
    } catch (e) {
      handleError(e);
    } finally {
      setLoading(false);
    }
  }, [handleError]);

  useEffect(() => {
    void load();
    api.getStoreBackground().then(setStoreBg).catch(() => setStoreBg(null));
    const id = setInterval(() => {
      api.listProcesses().then(setProcs).catch(() => undefined);
    }, 6000);
    return () => clearInterval(id);
  }, [load]);

  const unneeded = useMemo(() => (procs ?? []).filter((p) => p.unneeded), [procs]);
  const unneededMb = unneeded.reduce((a, b) => a + b.memoryMb, 0);
  const closeList = settings?.closeDuringGaming ?? [];
  const inBoost = (name: string) => closeList.some((n) => n.toLowerCase() === name.toLowerCase());
  const plentyRam = (appInfo?.totalMemoryMb ?? 0) >= 30_000;

  const toggleBoost = (name: string, v: boolean) => {
    const next = v ? [...closeList.filter((n) => n.toLowerCase() !== name.toLowerCase()), name] : closeList.filter((n) => n.toLowerCase() !== name.toLowerCase());
    void updateSettings({ closeDuringGaming: next });
  };

  const closeOne = async (p: ProcessInfo) => {
    setBusy(p.name);
    try {
      const r = await api.killProcess(p.pid);
      toast({ level: "success", title: t("notice.success"), message: t("background.closed", { count: r.closed.length || 1, size: formatMb(lang, r.freedMb) }) });
      setProcs((cur) => (cur ?? []).filter((x) => x.name !== p.name));
      void refreshJournal();
    } catch (e) {
      handleError(e);
    } finally {
      setBusy(null);
    }
  };

  const closeAll = async () => {
    setClosing(true);
    try {
      const names = unneeded.filter((p) => picked.has(p.name)).map((p) => p.name);
      const r = await api.closeApps(names);
      toast({
        level: "success",
        title: t("notice.success"),
        message: t("background.closed", { count: r.closed.length, size: formatMb(lang, r.freedMb) }),
        action: r.closed.length ? { label: t("background.restartClosed"), onClick: () => void api.restartClosedApps().catch(handleError) } : undefined,
      });
      setConfirm(false);
      void load();
      void refreshJournal();
    } catch (e) {
      handleError(e);
    } finally {
      setClosing(false);
    }
  };

  return (
    <div className="space-y-6">
      <PageHeader
        title={t("background.title")}
        subtitle={t("background.subtitle")}
        actions={
          <Button icon={RefreshCw} onClick={() => void load()} loading={loading}>
            {t("common.refresh")}
          </Button>
        }
      />

      {/* De grote knop: in één keer alles dicht wat niet nodig is */}
      <Card className="flex flex-wrap items-center gap-5">
        <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-accent-soft text-accent">
          <Sparkles className="h-5 w-5" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="text-[15px] font-semibold">{unneeded.length ? t("background.closeAll") : t("background.closeAllNone")}</div>
          <div className="mt-0.5 text-[13px] text-ink-2">
            {unneeded.length ? t("background.closeAllGain", { size: formatMb(lang, unneededMb) }) : t("background.protectedNote")}
          </div>
          {plentyRam && unneeded.length > 0 && <div className="mt-1 text-[12px] text-ink-3">{t("background.plentyRam", { total: formatMb(lang, appInfo?.totalMemoryMb ?? 0) })}</div>}
        </div>
        <Button
          variant="primary"
          size="lg"
          disabled={unneeded.length === 0}
          onClick={() => {
            setPicked(new Set(unneeded.map((p) => p.name)));
            setConfirm(true);
          }}
        >
          {t("background.closeAll")}
        </Button>
      </Card>

      <Card className="!p-0">
        {procs === null ? (
          <div className="space-y-2 p-4">
            {Array.from({ length: 6 }).map((_, i) => (
              <Skeleton key={i} className="h-12" />
            ))}
          </div>
        ) : procs.length === 0 ? (
          <div className="p-6 text-center text-sm text-ink-3">{t("background.empty")}</div>
        ) : (
          <>
            <div className="grid grid-cols-[minmax(0,1fr)_96px_80px_110px_96px] items-center gap-3 border-b border-line px-5 py-2.5 text-[12px] text-ink-3">
              <span>{t("background.colApp")}</span>
              <span className="text-right">{t("background.colMemory")}</span>
              <span className="text-right">{t("background.colCpu")}</span>
              <span className="text-center">{t("background.colBoost")}</span>
              <span />
            </div>
            <ul className="divide-y divide-line">
              {procs.map((p) => (
                <li key={p.name} className="grid grid-cols-[minmax(0,1fr)_96px_80px_110px_96px] items-center gap-3 px-5 py-2.5 hover:bg-panel-2/60">
                  <div className="flex min-w-0 items-center gap-3">
                    <AppIcon src={p.exePath ? icons[p.exePath] : null} />
                    <div className="min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="truncate text-sm font-medium" title={p.name}>
                          {p.title}
                        </span>
                        {p.unneeded && <Badge variant="accent">{t("background.unneeded")}</Badge>}
                        {p.category === "game" && <Badge variant="ok">{t("background.game")}</Badge>}
                      </div>
                      <div className="truncate text-[12px] text-ink-3">
                        {t(`category.${p.category}`)}
                        {p.instances > 1 ? ` · ${t("background.instances", { count: p.instances })}` : ""}
                      </div>
                    </div>
                  </div>
                  <span className={cn("text-right text-sm tnum", p.memoryMb > 1000 && "font-semibold")}>{formatMb(lang, p.memoryMb)}</span>
                  <span className={cn("text-right text-sm tnum text-ink-2", p.cpuPct >= 10 && "font-semibold text-warn")}>{formatPct(lang, p.cpuPct, 1)}</span>
                  <span className="flex justify-center">
                    {p.category !== "game" && <Toggle checked={inBoost(p.name)} onChange={(v) => toggleBoost(p.name, v)} label={`${p.title}: ${t("background.colBoost")}`} />}
                  </span>
                  <span className="flex justify-end">
                    {p.category !== "game" && (
                      <Button size="sm" variant="ghost" icon={X} loading={busy === p.name} onClick={() => void closeOne(p)}>
                        {t("background.close")}
                      </Button>
                    )}
                  </span>
                </li>
              ))}
            </ul>
          </>
        )}
      </Card>

      <p className="flex items-center gap-2 text-[12px] text-ink-3">
        <ShieldCheck className="h-4 w-4 text-ok" />
        {t("background.protectedNote")}
      </p>

      {storeBg !== null && (
        <Card className="!py-1">
          <Row title={t("background.storeTitle")} desc={t("background.storeDesc")}>
            <Toggle
              checked={!storeBg}
              onChange={async (v) => {
                try {
                  await api.setStoreBackground(!v);
                  setStoreBg(!v);
                  void refreshJournal();
                } catch (e) {
                  handleError(e);
                }
              }}
              label={t("background.storeTitle")}
            />
          </Row>
        </Card>
      )}

      <Modal
        open={confirm}
        onClose={() => setConfirm(false)}
        width="max-w-md"
        title={t("background.confirmTitle")}
        subtitle={t("background.confirmBody")}
        footer={
          <>
            <Button variant="ghost" onClick={() => setConfirm(false)}>
              {t("common.cancel")}
            </Button>
            <Button variant="primary" loading={closing} disabled={picked.size === 0} onClick={() => void closeAll()}>
              {t("background.confirm")}
            </Button>
          </>
        }
      >
        <ul className="space-y-1.5">
          {unneeded.map((p) => (
            <li key={p.name}>
              <label className="flex cursor-pointer items-center gap-3 rounded-lg border border-line bg-panel-2 px-3 py-2">
                <input
                  type="checkbox"
                  className="h-4 w-4"
                  checked={picked.has(p.name)}
                  onChange={(e) => {
                    const next = new Set(picked);
                    if (e.target.checked) next.add(p.name);
                    else next.delete(p.name);
                    setPicked(next);
                  }}
                />
                <AppIcon src={p.exePath ? icons[p.exePath] : null} />
                <span className="min-w-0 flex-1 truncate text-sm">{p.title}</span>
                <span className="text-[13px] tnum text-ink-2">{formatMb(lang, p.memoryMb)}</span>
              </label>
            </li>
          ))}
        </ul>
      </Modal>
    </div>
  );
}
