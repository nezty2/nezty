import { Check, Copy, ExternalLink, Mail, MessageCircle } from "lucide-react";
import { useEffect, useState } from "react";
import { useT } from "@/i18n/hooks";
import { api } from "@/lib/ipc";
import type { LicenseConfig } from "@/lib/types";
import { useApp } from "@/store/app";

/** Geschatte servertijd, die elke `ms` milliseconden opnieuw rendert. */
export function useServerNow(ms = 1000): number {
  const license = useApp((s) => s.license);
  const receivedAt = useApp((s) => s.licenseReceivedAt);
  const [, tick] = useState(0);
  useEffect(() => {
    const id = window.setInterval(() => tick((n) => n + 1), ms);
    return () => window.clearInterval(id);
  }, [ms]);
  return (license?.serverNow ?? Date.now()) + (Date.now() - receivedAt);
}

/** Contactgegevens van de eigenaar (ingesteld op de server), met kopiëren en openen. */
export function ContactInfo({ config, compact }: { config: LicenseConfig | null; compact?: boolean }) {
  const { t } = useT();
  const handleError = useApp((s) => s.handleError);
  const [copied, setCopied] = useState<string | null>(null);
  const c = config?.contact;
  const rows = [
    c?.discord ? { id: "discord", icon: MessageCircle, label: "Discord", value: c.discord, open: null as string | null } : null,
    c?.email ? { id: "email", icon: Mail, label: t("license.contactEmail"), value: c.email, open: `mailto:${c.email}` } : null,
    c?.website ? { id: "website", icon: ExternalLink, label: t("license.contactWebsite"), value: c.website, open: c.website.startsWith("http") ? c.website : `https://${c.website}` } : null,
  ].filter(Boolean) as { id: string; icon: typeof Mail; label: string; value: string; open: string | null }[];

  const copy = async (id: string, value: string) => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(id);
      window.setTimeout(() => setCopied(null), 1500);
    } catch {
      /* klembord niet beschikbaar */
    }
  };

  if (!c || (rows.length === 0 && !c.note)) {
    return <p className="text-[13px] text-ink-3">{config?.reachable === false ? t("license.contactOffline") : t("license.contactNone")}</p>;
  }
  return (
    <div className={compact ? "space-y-1.5" : "space-y-2"}>
      {c.note && <p className="text-[13px] leading-relaxed text-ink-2">{c.note}</p>}
      {rows.map((r) => (
        <div key={r.id} className="card-inset flex items-center gap-3 px-3 py-2">
          <r.icon className="h-4 w-4 shrink-0 text-accent" />
          <div className="min-w-0 flex-1">
            <div className="text-[11px] uppercase tracking-wide text-ink-3">{r.label}</div>
            <div className="selectable truncate text-[13px] font-medium">{r.value}</div>
          </div>
          {r.open && (
            <button type="button" title={t("license.contactOpen")} onClick={() => api.openUrl(r.open!).catch(handleError)} className="rounded-md p-1.5 text-ink-3 hover:bg-panel-2 hover:text-ink">
              <ExternalLink className="h-3.5 w-3.5" />
            </button>
          )}
          <button type="button" title={t("license.contactCopy")} onClick={() => void copy(r.id, r.value)} className="rounded-md p-1.5 text-ink-3 hover:bg-panel-2 hover:text-ink">
            {copied === r.id ? <Check className="h-3.5 w-3.5 text-ok" /> : <Copy className="h-3.5 w-3.5" />}
          </button>
        </div>
      ))}
    </div>
  );
}
