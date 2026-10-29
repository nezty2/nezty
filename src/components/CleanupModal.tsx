import { AlertTriangle, CheckCircle2, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useT } from "@/i18n/hooks";
import { api } from "@/lib/ipc";
import { formatBytes } from "@/lib/format";
import type { CleanupPreview, CleanupResult } from "@/lib/types";
import { useApp } from "@/store/app";
import { Badge, Button, Modal, Spinner, cn } from "./ui";

export function CleanupModal({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { t, td, lang } = useT();
  const [preview, setPreview] = useState<CleanupPreview | null>(null);
  const [loading, setLoading] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<CleanupResult | null>(null);
  const handleError = useApp((s) => s.handleError);
  const refreshJournal = useApp((s) => s.refreshJournal);

  useEffect(() => {
    if (!open) return;
    setResult(null);
    setPreview(null);
    setLoading(true);
    api
      .previewCleanup()
      .then((p) => {
        setPreview(p);
        // Standaard alleen gewone tijdelijke bestanden; shader-caches kies je zelf.
        setSelected(new Set(p.items.filter((i) => !i.shaderCache && !i.needsAdmin).map((i) => i.id)));
      })
      .catch(handleError)
      .finally(() => setLoading(false));
  }, [open, handleError]);

  const selectedBytes = useMemo(() => preview?.items.filter((i) => selected.has(i.id)).reduce((a, b) => a + b.sizeBytes, 0) ?? 0, [preview, selected]);
  const shaderSelected = preview?.items.some((i) => selected.has(i.id) && i.shaderCache) ?? false;

  const run = async () => {
    setRunning(true);
    try {
      setResult(await api.runCleanup(Array.from(selected)));
      void refreshJournal();
    } catch (e) {
      handleError(e);
    } finally {
      setRunning(false);
    }
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={t("cleanup.title")}
      subtitle={t("cleanup.subtitle")}
      footer={
        result ? (
          <Button variant="primary" onClick={onClose}>
            {t("common.done")}
          </Button>
        ) : (
          <>
            <Button variant="ghost" onClick={onClose}>
              {t("common.cancel")}
            </Button>
            <Button variant="primary" icon={Trash2} onClick={() => void run()} loading={running} disabled={selected.size === 0 || loading}>
              {t("cleanup.run", { size: formatBytes(lang, selectedBytes) })}
            </Button>
          </>
        )
      }
    >
      {loading && (
        <div className="flex items-center gap-3 py-8 text-sm text-ink-2">
          <Spinner /> {t("cleanup.scanning")}
        </div>
      )}
      {!loading && preview && !result && (
        <div className="space-y-2">
          {preview.items.length === 0 && <div className="py-6 text-center text-sm text-ink-3">{t("cleanup.nothing")}</div>}
          {preview.items.map((item) => {
            const checked = selected.has(item.id);
            return (
              <label
                key={item.id}
                className={cn(
                  "flex cursor-pointer items-center gap-3 rounded-lg border px-3.5 py-3 transition-colors",
                  checked ? "border-accent bg-accent-soft" : "border-line bg-panel-2 hover:border-line-2",
                  item.needsAdmin && "cursor-not-allowed opacity-60",
                )}
              >
                <input
                  type="checkbox"
                  className="h-4 w-4 shrink-0"
                  checked={checked}
                  disabled={item.needsAdmin}
                  onChange={(e) => {
                    const next = new Set(selected);
                    if (e.target.checked) next.add(item.id);
                    else next.delete(item.id);
                    setSelected(next);
                  }}
                />
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="text-sm font-medium">{td(`cleanup.items.${item.id}`, undefined, item.id)}</span>
                    {item.needsAdmin && <Badge variant="warn">{t("cleanup.adminNote")}</Badge>}
                  </div>
                  <div className="mt-0.5 truncate text-[12px] text-ink-3" title={item.path}>
                    {item.path}
                  </div>
                </div>
                <div className="shrink-0 text-right">
                  <div className="text-sm font-semibold tnum">{formatBytes(lang, item.sizeBytes)}</div>
                  <div className="text-[12px] text-ink-3 tnum">{t("cleanup.files", { count: item.fileCount })}</div>
                </div>
              </label>
            );
          })}
          {shaderSelected && (
            <div className="mt-3 flex items-start gap-2 rounded-lg border border-warn/30 bg-warn/10 p-3 text-[13px] text-warn">
              <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
              <span>{t("cleanup.shaderNote")}</span>
            </div>
          )}
        </div>
      )}
      {result && (
        <div className="flex items-center gap-3 rounded-lg border border-ok/30 bg-ok/10 p-4">
          <CheckCircle2 className="h-6 w-6 shrink-0 text-ok" />
          <div>
            <div className="text-[15px] font-semibold">{t("cleanup.done", { size: formatBytes(lang, result.freedBytes) })}</div>
            <div className="text-[13px] text-ink-2">{t("cleanup.doneSub", { deleted: result.deletedFiles, skipped: result.skippedFiles })}</div>
          </div>
        </div>
      )}
    </Modal>
  );
}
