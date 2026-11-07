import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, Card, SectionTitle } from "@/components/ui";
import { useApp } from "@/store/app";

type Account = { id: string; keyHint: string; isOwner: boolean; username: string; status: string; recent: boolean; lastSeen: string | null; connectedAt: string | null; activatedAt: string | null; expiresAt: string | null };
const date = (value: string | null) => value ? new Date(value).toLocaleString("nl-NL", {timeZone:"Europe/Amsterdam",day:"2-digit",month:"2-digit",year:"numeric",hour:"2-digit",minute:"2-digit",second:"2-digit",hour12:false}) : "—";

export function OwnerPanel() {
  const license = useApp(s => s.license);
  return license?.mode === "active" && license.username === "Mero" && license.keyHint === "88sn" &&
    license.licenseId === "d66b2498-0c00-4c78-831d-9cb0abdab366" ? <Accounts /> : null;
}

function Accounts() {
  const [rows,setRows] = useState<Account[] | null>(null);
  const [error,setError] = useState(false);
  const [refresh,setRefresh] = useState(0);
  const [selected,setSelected] = useState<Account | null>(null);
  const [busy,setBusy] = useState(false);
  const [actionError,setActionError] = useState(false);
  useEffect(() => {
    let live = true;
    let timer: ReturnType<typeof setTimeout>;
    async function load() {
      try {
        const data = await invoke<Account[]>("owner_accounts");
        if (live) { setRows(data); setError(false); }
      } catch {
        if (live) { setRows(null); setError(true); }
      } finally {
        if (live) timer = setTimeout(load, 60000);
      }
    }
    void load();
    return () => { live = false; clearTimeout(timer); };
  }, [refresh]);
  async function revoke() {
    if (!selected || busy) return;
    setBusy(true); setActionError(false);
    try {
      await invoke("owner_revoke", {id:selected.id});
      setRows(old => old?.map(row => row.id === selected.id ? {...row,status:"revoked",recent:false} : row) ?? null);
      setSelected(null); setRefresh(n => n + 1);
    } catch { setActionError(true); }
    finally { setBusy(false); }
  }
  return <section>
    <SectionTitle>Verbonden accounts</SectionTitle>
    <Card>
      <div className="flex items-center justify-between gap-3">
        <span>{rows ? `${rows.length} accounts · Recent actief: ${rows.filter(r => r.recent).length}` : error ? "Accounts laden mislukt" : "Laden…"}</span>
        <Button size="sm" disabled={busy} onClick={() => setRefresh(n => n + 1)}>Vernieuwen</Button>
      </div>
      <p className="my-2 text-xs text-ink-3">Nederlandse tijd (Amsterdam), inclusief zomer-/wintertijd. Recent actief = licentiecontrole in de afgelopen 20 minuten.</p>
      <p className="my-2 text-xs text-ink-3">Keys zijn alleen herkenbaar aan de laatste vier tekens; volledige keys worden niet opgeslagen.</p>
      {selected && <div role="alert" className="my-3 rounded-lg border border-line p-3 text-sm">
        <p>Licentie van <strong>{selected.username}</strong> (•••• {selected.keyHint}) intrekken?</p>
        <p className="my-2 text-xs text-ink-3">De server blokkeert deze licentie. De app verwerkt dit bij de volgende licentiecontrole; een bestaand offline token kan tot zijn controlemoment geldig blijven.</p>
        <div className="flex gap-2"><Button size="sm" disabled={busy} onClick={() => void revoke()}>{busy ? "Intrekken…" : "Ja, intrekken"}</Button><Button size="sm" disabled={busy} onClick={() => setSelected(null)}>Annuleren</Button></div>
        {actionError && <p role="alert">Intrekken mislukt. Probeer opnieuw.</p>}
      </div>}
      {rows?.length === 0 && <p>Geen geactiveerde accounts</p>}
      <div className="max-h-96 overflow-auto space-y-3">
        {rows?.map(r => <div key={r.id} className="border-t border-line pt-3 text-sm">
          <div className="flex items-center justify-between gap-2"><strong>{r.username}{r.isOwner ? " · Jij" : ""}</strong><span>{({active:"Actief",expired:"Verlopen",revoked:"Ingetrokken"})[r.status] ?? r.status}</span></div>
          <div className="mt-1">Key: <code>•••• {r.keyHint}</code>{r.recent && <span className="ml-3 text-green-400">Recent actief</span>}</div>
          <div className="text-xs text-ink-3 break-all">Licentie-ID: {r.id}</div>
          <dl className="my-2 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs">
            <dt>Geactiveerd</dt><dd>{date(r.activatedAt)}</dd>
            <dt>Laatst verbonden</dt><dd>{date(r.connectedAt)}</dd>
            <dt>Laatste controle</dt><dd>{date(r.lastSeen)}</dd>
            <dt>Geldig tot</dt><dd>{r.expiresAt ? date(r.expiresAt) : "Onbeperkt"}</dd>
          </dl>
          {!r.isOwner && r.status !== "revoked" && <Button size="sm" disabled={busy} onClick={() => {setSelected(r);setActionError(false);}}>Licentie intrekken</Button>}
        </div>)}
      </div>
    </Card>
  </section>;
}
