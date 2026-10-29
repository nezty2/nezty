// nezty licentiegenerator. Alleen voor de eigenaar; dit zit NIET in de app.
//
// Nodig (omgevingsvariabelen op je eigen pc, nooit in de repo):
//   NEZTY_LICENSE_URL   bv. https://<project>.supabase.co/functions/v1/license
//   NEZTY_ADMIN_KEY     je beheerderssleutel
//
// Voorbeelden (vanuit de map nzty-broncode):
//   npm run licentie:maak -- --type maand
//   npm run licentie:maak -- --type demo
//   npm run licentie:maak -- --type maand --aantal 5
//   npm run licentie:verleng -- --sleutel nezty-xxxx-xxxx-xxx --dagen 30
//   npm run licentie:intrekken -- --sleutel nezty-...
//   npm run licentie:reset-apparaat -- --sleutel nezty-...
//   npm run licentie:lijst
//   npm run licentie:info -- --sleutel nezty-...
//   npm run gebruiker:reset-wachtwoord -- --gebruiker naam
//   npm run licentie:instellingen -- --demo aan --discord jouwnaam --email jij@voorbeeld.nl
//   npm run licentie:sleutels-maken

import { writeFileSync } from "node:fs";
import { newSigningKeyPair, randomToken, sha256Hex } from "../../server/supabase/functions/_shared/crypto.ts";
import { normalize } from "../../server/supabase/functions/_shared/keys.ts";

const [, , command, ...rest] = process.argv;

function opt(name: string): string | null {
  const i = rest.indexOf(`--${name}`);
  return i >= 0 && rest[i + 1] && !rest[i + 1].startsWith("--") ? rest[i + 1] : null;
}

function stop(msg: string): never {
  console.error(`\n  ${msg}\n`);
  process.exit(1);
}

const STATUS: Record<string, string> = { unused: "niet gebruikt", active: "actief", expired: "verlopen", revoked: "ingetrokken" };
const TYPE: Record<string, string> = { month: "maand", demo: "demo" };
const ERRORS: Record<string, string> = {
  unauthorized: "De server accepteert je beheerderssleutel niet. Controleer NEZTY_ADMIN_KEY.",
  key_format: "Dit is geen geldige sleutel. Controleer of je hem goed hebt overgetypt.",
  key_unknown: "Deze sleutel bestaat niet.",
  ambiguous: "Meer dan één licentie begint met dit id. Geef meer tekens.",
  user_unknown: "Deze gebruiker bestaat niet.",
  password_weak: "Het wachtwoord moet minstens 8 tekens hebben.",
  bad_request: "Er ontbreekt iets in de opdracht.",
  not_found: "De server kent deze opdracht niet. Is de serverfunctie bijgewerkt?",
  server_error: "Er ging iets mis op de server. Kijk in de logs van Supabase.",
};

function span(l: Record<string, any>): string {
  const m = Number(l.durationMinutes ?? l.durationDays * 1440);
  return m < 1440 ? `${Math.round(m / 60)} uur na activeren` : `${Math.round(m / 1440)} dagen na activeren`;
}

function date(iso: string | null): string {
  if (!iso) return "-";
  return new Date(iso).toLocaleString("nl-NL", { day: "2-digit", month: "2-digit", year: "numeric", hour: "2-digit", minute: "2-digit" });
}

async function api(path: string, body: Record<string, unknown> = {}) {
  const url = process.env.NEZTY_LICENSE_URL;
  const admin = process.env.NEZTY_ADMIN_KEY;
  if (!url) stop("Zet eerst NEZTY_LICENSE_URL (het adres van je licentieserver).");
  if (!admin) stop("Zet eerst NEZTY_ADMIN_KEY (je beheerderssleutel).");
  let res: Response;
  try {
    res = await fetch(`${url.replace(/\/$/, "")}${path}`, { method: "POST", headers: { "content-type": "application/json", "x-admin-key": admin }, body: JSON.stringify(body) });
  } catch (e) {
    stop(`Kan de server niet bereiken (${(e as Error).message}).`);
  }
  const data = (await res.json().catch(() => ({ ok: false, error: "server_error" }))) as Record<string, any>;
  if (!data.ok) stop(ERRORS[data.error] ?? `Fout van de server: ${data.error}`);
  return data;
}

function target(): Record<string, string> {
  const key = opt("sleutel");
  const id = opt("id");
  if (key) {
    if (!normalize(key)) stop(ERRORS.key_format);
    return { key };
  }
  if (id) return { id };
  stop("Geef --sleutel nezty-xxxx-xxxx-xxx (of --id uit de lijst).");
}

function showLicense(l: Record<string, any>) {
  const rows: [string, string][] = [
    ["Sleutel", `...${l.hint}`],
    ["Id", l.id.slice(0, 8)],
    ["Type", TYPE[l.type] ?? l.type],
    ["Status", STATUS[l.status] ?? l.status],
    ["Gebruiker", l.username ?? "-"],
    ["Apparaat", l.device ?? "-"],
    ["Gemaakt", date(l.createdAt)],
    ["Geactiveerd", date(l.activatedAt)],
    ["Einddatum", l.expiresAt ? date(l.expiresAt) : span(l)],
    ["Notitie", l.note ?? "-"],
  ];
  console.log("");
  for (const [k, v] of rows) console.log(`  ${k.padEnd(12)} ${v}`);
  console.log("");
}

function box(lines: string[]) {
  const w = Math.max(...lines.map((l) => l.length)) + 4;
  console.log(`\n  ╔${"═".repeat(w)}╗`);
  for (const l of lines) console.log(`  ║  ${l.padEnd(w - 2)}║`);
  console.log(`  ╚${"═".repeat(w)}╝\n`);
}

async function main() {
  switch (command) {
    case "maak": {
      const t = opt("type");
      const type = t === "maand" ? "month" : t === "demo" ? "demo" : null;
      if (!type) stop("Kies --type maand of --type demo.");
      const count = Number(opt("aantal") ?? "1");
      if (!Number.isInteger(count) || count < 1 || count > 100) stop("--aantal moet tussen 1 en 100 liggen.");
      const r = await api("/admin/create", { type, count, note: opt("notitie") });
      console.log(`\n  ${r.keys.length} ${type === "month" ? "maandlicentie(s) (30 dagen)" : "demolicentie(s) (2 uur)"} gemaakt. De tijd start pas als de klant de sleutel activeert.`);
      box(r.keys);
      console.log("  Kopieer de sleutel nu: hij wordt nergens bewaard en is later niet meer op te vragen.\n");
      break;
    }
    case "verleng": {
      const days = Number(opt("dagen"));
      if (!Number.isFinite(days) || days <= 0) stop("Geef --dagen, bijvoorbeeld --dagen 30.");
      const r = await api("/admin/extend", { ...target(), days });
      console.log(`\n  Verlengd met ${days} dagen. De klant ziet de nieuwe einddatum bij de volgende controle (binnen 10 minuten).`);
      showLicense(r.license);
      break;
    }
    case "intrekken": {
      const r = await api("/admin/revoke", target());
      console.log("\n  Ingetrokken. De app vergrendelt bij de volgende controle (binnen 10 minuten, offline uiterlijk na 24 uur).");
      showLicense(r.license);
      break;
    }
    case "reset-apparaat": {
      const r = await api("/admin/reset-device", target());
      console.log("\n  Apparaat losgekoppeld. De klant kan nu op zijn nieuwe pc inloggen; dat apparaat wordt dan gekoppeld.");
      showLicense(r.license);
      break;
    }
    case "info": {
      showLicense((await api("/admin/info", target())).license);
      break;
    }
    case "lijst": {
      const r = await api("/admin/list");
      const list = r.licenses as Record<string, any>[];
      if (list.length === 0) {
        console.log("\n  Nog geen licenties. Maak er een met: npm run licentie:maak -- --type maand\n");
        break;
      }
      const head = ["Id", "Sleutel", "Type", "Status", "Gebruiker", "Apparaat", "Geactiveerd", "Einddatum"];
      const rows = list.map((l) => [
        l.id.slice(0, 8),
        `...${l.hint}`,
        TYPE[l.type] ?? l.type,
        STATUS[l.status] ?? l.status,
        l.username ?? "-",
        l.device ?? "-",
        date(l.activatedAt),
        l.expiresAt ? date(l.expiresAt) : span(l),
      ]);
      const widths = head.map((h, i) => Math.max(h.length, ...rows.map((r) => String(r[i]).length)));
      const line = (cells: string[]) => "  " + cells.map((c, i) => String(c).padEnd(widths[i])).join("  ");
      console.log("");
      console.log(line(head));
      console.log("  " + widths.map((w) => "─".repeat(w)).join("  "));
      for (const r of rows) console.log(line(r));
      const count = (s: string) => list.filter((l) => l.status === s).length;
      console.log(`\n  Totaal ${list.length}: ${count("active")} actief, ${count("unused")} niet gebruikt, ${count("expired")} verlopen, ${count("revoked")} ingetrokken.\n`);
      break;
    }
    case "reset-wachtwoord": {
      const user = opt("gebruiker");
      if (!user) stop("Geef --gebruiker <naam>.");
      const r = await api("/admin/reset-password", { username: user, password: opt("wachtwoord") });
      console.log(`\n  Nieuw wachtwoord voor ${r.username}. Geef het aan de klant; die is overal uitgelogd.`);
      box([r.password]);
      break;
    }
    case "instellingen": {
      const body: Record<string, unknown> = {};
      const demo = opt("demo");
      if (demo) {
        if (demo !== "aan" && demo !== "uit") stop("--demo aan of --demo uit");
        body.demoEnabled = demo === "aan";
      }
      const contact: Record<string, string> = {};
      for (const [flag, key] of [["discord", "discord"], ["email", "email"], ["website", "website"], ["tekst", "note"]]) {
        const v = opt(flag);
        if (v !== null) contact[key] = v === "-" ? "" : v;
      }
      if (Object.keys(contact).length) body.contact = contact;
      const r = await api("/admin/settings", body);
      console.log(`\n  Demo-knop in de app: ${r.demoEnabled ? "aan" : "uit"}`);
      console.log(`  Discord: ${r.contact.discord || "-"}`);
      console.log(`  E-mail:  ${r.contact.email || "-"}`);
      console.log(`  Website: ${r.contact.website || "-"}`);
      console.log(`  Tekst:   ${r.contact.note || "-"}\n`);
      break;
    }
    case "sleutels-maken": {
      // Lokaal: maakt alle geheimen in één keer. Niets hiervan gaat naar de server of de repo.
      const pair = await newSigningKeyPair();
      const adminKey = `nzadm_${randomToken(32)}`;
      const secrets = {
        LICENSE_SIGNING_KEY: pair.privatePkcs8,
        LICENSE_KEY_PEPPER: randomToken(32),
        ADMIN_KEY_SHA256: await sha256Hex(adminKey),
        NEZTY_ADMIN_KEY: adminKey,
        NEZTY_LICENSE_PUBKEY: pair.publicRaw,
      };
      const file = opt("bestand");
      if (file) {
        writeFileSync(file, JSON.stringify(secrets, null, 2));
        console.log(`\n  Geheimen opgeslagen in ${file}. Bewaar dit bestand alleen op je eigen pc.\n`);
        break;
      }
      console.log("\n  Nieuwe geheimen. Dit wordt maar één keer getoond.\n");
      console.log("  1. Voor de server (supabase secrets set ...):");
      console.log(`     LICENSE_SIGNING_KEY=${secrets.LICENSE_SIGNING_KEY}`);
      console.log(`     LICENSE_KEY_PEPPER=${secrets.LICENSE_KEY_PEPPER}`);
      console.log(`     ADMIN_KEY_SHA256=${secrets.ADMIN_KEY_SHA256}\n`);
      console.log("  2. Alleen op je eigen pc (omgevingsvariabele voor deze generator):");
      console.log(`     NEZTY_ADMIN_KEY=${secrets.NEZTY_ADMIN_KEY}\n`);
      console.log("  3. Publieke sleutel voor de app (niet geheim, in scripts/license.json):");
      console.log(`     ${secrets.NEZTY_LICENSE_PUBKEY}\n`);
      console.log("  Let op: maak je later nieuwe geheimen, dan werken bestaande sleutels en de app niet meer met elkaar.\n");
      break;
    }
    default:
      console.log(`
  nezty licentiegenerator

  npm run licentie:maak -- --type maand|demo [--aantal 5] [--notitie "klant X"]
  npm run licentie:verleng -- --sleutel nezty-... --dagen 30
  npm run licentie:intrekken -- --sleutel nezty-...
  npm run licentie:reset-apparaat -- --sleutel nezty-...
  npm run licentie:lijst
  npm run licentie:info -- --sleutel nezty-...
  npm run gebruiker:reset-wachtwoord -- --gebruiker naam [--wachtwoord nieuw]
  npm run licentie:instellingen -- [--demo aan|uit] [--discord x] [--email x] [--website x] [--tekst x]
  npm run licentie:sleutels-maken

  In plaats van --sleutel mag ook --id (de eerste tekens uit de lijst).
`);
  }
}

await main();
