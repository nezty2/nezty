// Licentie in de frontend: resterende tijd, kleur en een snelle controle op typfouten in de sleutel.
// Of een sleutel echt geldig is, bepaalt alleen de server.

import type { LicenseStatus } from "./types";

const ALPHABET = "23456789ABCDEFGHJKLMNPQRSTUVWXYZ";

function checkChar(body: string): string {
  let factor = 2;
  let sum = 0;
  for (let i = body.length - 1; i >= 0; i--) {
    let addend = factor * ALPHABET.indexOf(body[i]);
    factor = factor === 2 ? 1 : 2;
    addend = Math.floor(addend / 32) + (addend % 32);
    sum += addend;
  }
  return ALPHABET[(32 - (sum % 32)) % 32];
}

/** Tekens zonder voorvoegsel en streepjes, of null als het geen geldige sleutel kan zijn. */
export function keyChars(input: string): string | null {
  let s = input.toUpperCase().replace(/[\s-]/g, "");
  if (s.startsWith("NZT") && s.length === 19) s = s.slice(3);
  if (s.length !== 16 || [...s].some((c) => !ALPHABET.includes(c))) return null;
  return checkChar(s.slice(0, 15)) === s[15] ? s : null;
}

/** Netjes opmaken tijdens het typen: NZT-XXXX-XXXX-XXXX-XXXX. */
export function formatKeyInput(input: string): string {
  let s = input.toUpperCase().replace(/[^0-9A-Z]/g, "");
  if (s.startsWith("NZT")) s = s.slice(3);
  s = s.slice(0, 16);
  const groups = s.match(/.{1,4}/g) ?? [];
  return groups.length ? `NZT-${groups.join("-")}` : "";
}

export type LicenseTone = "normal" | "warn" | "bad";

export const LOCKED_MODES = ["expired", "revoked", "offline"];

export function isLocked(l: LicenseStatus | null): boolean {
  return !!l && LOCKED_MODES.includes(l.mode);
}

/** Resterende tijd in ms (servertijd). */
export function remainingMs(l: LicenseStatus, serverNow: number): number {
  return l.expiresAt == null ? 0 : Math.max(0, l.expiresAt - serverNow);
}

export function progress(l: LicenseStatus, serverNow: number): number {
  if (l.expiresAt == null || l.activatedAt == null) return 0;
  const total = l.expiresAt - l.activatedAt;
  return total > 0 ? Math.min(1, Math.max(0, remainingMs(l, serverNow) / total)) : 0;
}

export function tone(l: LicenseStatus, serverNow: number): LicenseTone {
  if (isLocked(l)) return "bad";
  const left = remainingMs(l, serverNow);
  const limit = l.kind === "demo" ? 15 * 60_000 : 3 * 86_400_000;
  return left < limit ? "warn" : "normal";
}

/** Demo-klok: 1:42:10. */
export function clock(ms: number): string {
  const s = Math.floor(ms / 1000);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  return `${h}:${String(m).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
}

/** Dagen, uren en minuten voor de tekst "Nog 23 dagen en 4 uur". */
export function parts(ms: number): { days: number; hours: number; minutes: number } {
  const m = Math.floor(ms / 60_000);
  return { days: Math.floor(m / 1440), hours: Math.floor((m % 1440) / 60), minutes: m % 60 };
}

/** Momenten waarop we waarschuwen: 3 dagen en 1 dag (maand), 15 minuten (demo). */
export function warnThresholds(l: LicenseStatus): number[] {
  return l.kind === "demo" ? [15 * 60_000] : [3 * 86_400_000, 86_400_000];
}
