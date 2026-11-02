import { Sparkles } from "lucide-react";
import { useEffect, useState } from "react";
import { useT } from "@/i18n/hooks";
import { useApp } from "@/store/app";
import { Button, Modal } from "./ui";

interface Saved {
  version: string;
  notes: string;
  shown: boolean;
}

/**
 * Na een update: de tekst uit het updatescherm ("Wat is er nieuw") één keer rustig tonen. Het updatescherm zelf
 * verdwijnt meteen bij het bijwerken, dus zo kun je het alsnog lezen. Opgeslagen vlak voor het bijwerken.
 */
export function WhatsNew() {
  const { t } = useT();
  const version = useApp((s) => s.appInfo?.version);
  const [saved, setSaved] = useState<Saved | null>(null);

  useEffect(() => {
    if (!version) return;
    try {
      const raw = window.localStorage.getItem("nezty.whatsNew");
      const s = raw ? (JSON.parse(raw) as Saved) : null;
      if (s && !s.shown && s.version === version && s.notes) setSaved(s);
    } catch {
      /* niets opgeslagen of onleesbaar: niets tonen */
    }
  }, [version]);

  const close = () => {
    if (saved) {
      try {
        window.localStorage.setItem("nezty.whatsNew", JSON.stringify({ ...saved, shown: true }));
      } catch {
        /* dan kan hij de volgende keer nog een keer komen */
      }
    }
    setSaved(null);
  };

  return (
    <Modal
      open={!!saved}
      onClose={close}
      width="max-w-md"
      title={
        <span className="flex items-center gap-2">
          <Sparkles className="h-4 w-4 text-accent" />
          {t("whatsNew.title", { version: saved?.version ?? "" })}
        </span>
      }
      footer={
        <Button variant="primary" onClick={close}>
          {t("whatsNew.close")}
        </Button>
      }
    >
      <p className="whitespace-pre-line text-[13px] leading-relaxed text-ink-2">{saved?.notes}</p>
    </Modal>
  );
}
