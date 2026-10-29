import type { Lang } from "./types";
import { LOCALES } from "@/i18n";

const cache = new Map<string, Intl.NumberFormat>();
function nf(lang: Lang, digits: number): Intl.NumberFormat {
  const k = `${lang}:${digits}`;
  let f = cache.get(k);
  if (!f) {
    f = new Intl.NumberFormat(LOCALES[lang], { maximumFractionDigits: digits });
    cache.set(k, f);
  }
  return f;
}

/** Eenheden per taal: in het Frans Go/Mo/Ko (octet). */
const UNITS: Record<Lang, { gb: string; mb: string; kb: string; b: string }> = {
  nl: { gb: "GB", mb: "MB", kb: "kB", b: "B" },
  en: { gb: "GB", mb: "MB", kb: "kB", b: "B" },
  fr: { gb: "Go", mb: "Mo", kb: "Ko", b: "o" },
};

export function formatMb(lang: Lang, mb: number | null | undefined): string {
  if (mb == null) return "-";
  const u = UNITS[lang];
  return mb >= 1024 ? `${nf(lang, 1).format(mb / 1024)} ${u.gb}` : `${nf(lang, 0).format(mb)} ${u.mb}`;
}

export function formatBytes(lang: Lang, bytes: number): string {
  const u = UNITS[lang];
  if (bytes >= 1024 ** 3) return `${nf(lang, 1).format(bytes / 1024 ** 3)} ${u.gb}`;
  if (bytes >= 1024 ** 2) return `${nf(lang, 1).format(bytes / 1024 ** 2)} ${u.mb}`;
  if (bytes >= 1024) return `${nf(lang, 0).format(bytes / 1024)} ${u.kb}`;
  return `${bytes} ${u.b}`;
}

export function formatPct(lang: Lang, v: number | null | undefined, digits = 0): string {
  if (v == null) return "-";
  return `${nf(lang, digits).format(v)}%`;
}

export function formatMs(lang: Lang, v: number | null | undefined): string {
  if (v == null) return "-";
  return `${nf(lang, v < 10 ? 1 : 0).format(v)} ms`;
}

export function formatNumber(lang: Lang, v: number, digits = 0): string {
  return nf(lang, digits).format(v);
}

export function formatTemp(lang: Lang, v: number | null | undefined): string {
  if (v == null) return "-";
  return `${nf(lang, 0).format(v)}°C`;
}

export function formatTime(lang: Lang, ts: number): string {
  return new Intl.DateTimeFormat(LOCALES[lang], { hour: "2-digit", minute: "2-digit", second: "2-digit" }).format(new Date(ts));
}

export function formatDate(lang: Lang, iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return new Intl.DateTimeFormat(LOCALES[lang], { day: "numeric", month: "long", year: "numeric" }).format(d);
}

export function formatDayLong(lang: Lang, ts: number): string {
  return new Intl.DateTimeFormat(LOCALES[lang], { weekday: "long", day: "numeric", month: "long" }).format(new Date(ts));
}

/** "zojuist", "5 min geleden" enz. via Intl.RelativeTimeFormat in de gekozen taal. */
export function timeAgo(lang: Lang, ts: number): string {
  const rtf = new Intl.RelativeTimeFormat(LOCALES[lang], { numeric: "auto" });
  const s = Math.round((ts - Date.now()) / 1000);
  const abs = Math.abs(s);
  if (abs < 60) return rtf.format(Math.round(s), "second");
  if (abs < 3600) return rtf.format(Math.round(s / 60), "minute");
  if (abs < 86400) return rtf.format(Math.round(s / 3600), "hour");
  return rtf.format(Math.round(s / 86400), "day");
}

export function isToday(ts: number): boolean {
  return new Date(ts).toDateString() === new Date().toDateString();
}

export function isYesterday(ts: number): boolean {
  const y = new Date();
  y.setDate(y.getDate() - 1);
  return new Date(ts).toDateString() === y.toDateString();
}
