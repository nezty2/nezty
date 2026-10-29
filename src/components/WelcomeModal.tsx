import { ShieldCheck, Zap } from "lucide-react";
import { useT } from "@/i18n/hooks";
import { LANGS } from "@/i18n";
import { useApp } from "@/store/app";
import { Button, Modal, cn } from "./ui";

/** Eerste start: taal kiezen en uitleg in een paar zinnen. */
export function WelcomeModal() {
  const { t, lang } = useT();
  const settings = useApp((s) => s.settings);
  const updateSettings = useApp((s) => s.updateSettings);
  const open = !!settings && !settings.firstRunDone;

  return (
    <Modal
      open={open}
      onClose={() => void updateSettings({ firstRunDone: true })}
      width="max-w-md"
      title={
        <span className="flex items-center gap-2.5">
          <span className="flex h-8 w-8 items-center justify-center rounded-lg bg-accent">
            <Zap className="h-4 w-4 text-white" fill="currentColor" />
          </span>
          {t("welcome.title")}
        </span>
      }
      footer={
        <Button variant="primary" size="lg" className="w-full" onClick={() => void updateSettings({ firstRunDone: true, language: lang })}>
          {t("welcome.start")}
        </Button>
      }
    >
      <div className="space-y-5">
        <div>
          <div className="mb-2 text-[13px] font-medium text-ink-3">{t("welcome.chooseLanguage")}</div>
          <div className="grid grid-cols-3 gap-2">
            {LANGS.map((l) => (
              <button
                key={l.id}
                onClick={() => void updateSettings({ language: l.id })}
                className={cn(
                  "rounded-lg border px-3 py-2.5 text-sm font-medium transition-colors",
                  lang === l.id ? "border-accent bg-accent-soft text-ink" : "border-line bg-panel-2 text-ink-2 hover:border-line-2",
                )}
              >
                {l.label}
              </button>
            ))}
          </div>
        </div>
        <p className="text-sm leading-relaxed text-ink-2">{t("welcome.body")}</p>
        <p className="flex items-start gap-2 text-[13px] leading-relaxed text-ink-3">
          <ShieldCheck className="mt-0.5 h-4 w-4 shrink-0 text-ok" />
          {t("welcome.safe")}
        </p>
      </div>
    </Modal>
  );
}
