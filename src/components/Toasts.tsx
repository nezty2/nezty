import { AnimatePresence, motion } from "framer-motion";
import { AlertTriangle, CheckCircle2, Info, X, XCircle } from "lucide-react";
import { useState } from "react";
import { useT } from "@/i18n/hooks";
import { useApp, type Toast } from "@/store/app";
import { cn } from "./ui";

const styles = {
  info: { icon: Info, cls: "text-info" },
  success: { icon: CheckCircle2, cls: "text-ok" },
  warning: { icon: AlertTriangle, cls: "text-warn" },
  error: { icon: XCircle, cls: "text-bad" },
};

function ToastItem({ t: toast }: { t: Toast }) {
  const { t } = useT();
  const dismiss = useApp((s) => s.dismissToast);
  const [showDetail, setShowDetail] = useState(false);
  const s = styles[toast.level];
  const Icon = s.icon;
  return (
    <motion.div
      layout
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: 8 }}
      transition={{ duration: 0.15 }}
      className="pointer-events-auto flex items-start gap-3 rounded-xl border border-line-2 bg-panel px-4 py-3 shadow-2xl"
    >
      <Icon className={cn("mt-0.5 h-4 w-4 shrink-0", s.cls)} />
      <div className="min-w-0 flex-1">
        <div className="text-sm font-semibold">{toast.title}</div>
        <div className="selectable mt-0.5 text-[13px] leading-relaxed text-ink-2">{toast.message}</div>
        {toast.detail && (
          <button onClick={() => setShowDetail((v) => !v)} className="mt-1 text-[12px] text-ink-3 underline-offset-2 hover:underline">
            {t("msg.details")}
          </button>
        )}
        {showDetail && <div className="selectable mt-1 break-words font-mono text-[11px] text-ink-3">{toast.detail}</div>}
        {toast.action && (
          <button
            onClick={() => {
              toast.action?.onClick();
              dismiss(toast.id);
            }}
            className="mt-2 rounded-md border border-line-2 px-2.5 py-1 text-[13px] font-medium hover:bg-panel-2"
          >
            {toast.action.label}
          </button>
        )}
      </div>
      <button onClick={() => dismiss(toast.id)} className="rounded p-1 text-ink-3 hover:text-ink" aria-label={t("common.close")}>
        <X className="h-3.5 w-3.5" />
      </button>
    </motion.div>
  );
}

export function Toasts() {
  const toasts = useApp((s) => s.toasts);
  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-[60] flex w-[380px] max-w-[calc(100vw-2rem)] flex-col gap-2">
      <AnimatePresence initial={false}>
        {toasts.map((t) => (
          <ToastItem key={t.id} t={t} />
        ))}
      </AnimatePresence>
    </div>
  );
}
