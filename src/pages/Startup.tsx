import { AppWindow, Lock, Power, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useAppIcons } from "@/components/useAppIcons";
import { Badge, Button, Card, EmptyState, PageHeader, SearchInput, Skeleton, Toggle, cn } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import { api } from "@/lib/ipc";
import type { StartupItem } from "@/lib/types";
import { useApp } from "@/store/app";

export function StartupPage() {
  const { t } = useT();
  const appInfo = useApp((s) => s.appInfo);
  const handleError = useApp((s) => s.handleError);
  const refreshJournal = useApp((s) => s.refreshJournal);
  const [items, setItems] = useState<StartupItem[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [search, setSearch] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const elevated = appInfo?.isElevated ?? false;
  const icons = useAppIcons((items ?? []).map((i) => i.exePath));

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setItems(await api.listStartupItems());
    } catch (e) {
      handleError(e);
    } finally {
      setLoading(false);
    }
  }, [handleError]);

  useEffect(() => {
    void load();
  }, [load]);

  const filtered = useMemo(() => (items ?? []).filter((i) => i.name.toLowerCase().includes(search.toLowerCase())), [items, search]);
  const enabledCount = (items ?? []).filter((i) => i.enabled).length;

  const toggle = async (item: StartupItem, v: boolean) => {
    setBusy(item.id);
    try {
      const updated = await api.setStartupItem(item.id, v);
      setItems((cur) => (cur ?? []).map((i) => (i.id === item.id ? updated : i)));
      void refreshJournal();
    } catch (e) {
      handleError(e);
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="space-y-6">
      <PageHeader
        title={t("startup.title")}
        subtitle={t("startup.subtitle", { on: enabledCount, total: items?.length ?? 0 })}
        actions={
          <>
            <SearchInput value={search} onChange={setSearch} placeholder={t("startup.search")} className="w-56" />
            <Button icon={RefreshCw} onClick={() => void load()} loading={loading}>
              {t("common.refresh")}
            </Button>
          </>
        }
      />
      <Card className="!p-0">
        {items === null ? (
          <div className="space-y-2 p-4">
            {Array.from({ length: 5 }).map((_, i) => (
              <Skeleton key={i} className="h-12" />
            ))}
          </div>
        ) : filtered.length === 0 ? (
          <div className="p-6">
            <EmptyState icon={Power} title={t("startup.empty")} />
          </div>
        ) : (
          <ul className="divide-y divide-line">
            {filtered.map((item) => {
              const locked = item.requiresAdmin && !elevated;
              const icon = item.exePath ? icons[item.exePath] : null;
              return (
                <li key={item.id} className={cn("grid grid-cols-[minmax(0,1fr)_auto] items-center gap-6 px-5 py-3", !item.enabled && "opacity-70")}>
                  <div className="flex min-w-0 items-center gap-3">
                    {icon ? (
                      <img src={icon} alt="" className="h-7 w-7 shrink-0" />
                    ) : (
                      <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-panel-2 text-ink-3">
                        <AppWindow className="h-4 w-4" />
                      </span>
                    )}
                    <div className="min-w-0">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="truncate text-sm font-medium">{item.name}</span>
                        <Badge variant="neutral">{t(`startup.source.${item.source}`)}</Badge>
                        {locked && (
                          <Badge variant="warn" icon={Lock} title={t("startup.adminHint")}>
                            {t("tweakCard.adminNeeded")}
                          </Badge>
                        )}
                      </div>
                      <div className="selectable mt-0.5 truncate font-mono text-[11px] text-ink-3" title={item.command}>
                        {item.command}
                      </div>
                    </div>
                  </div>
                  <Toggle checked={item.enabled} loading={busy === item.id} disabled={locked} onChange={(v) => void toggle(item, v)} label={item.name} />
                </li>
              );
            })}
          </ul>
        )}
      </Card>
    </div>
  );
}
