// Eenvoudige, getypeerde vertalingen. nl.ts is de bron; en.ts en fr.ts moeten exact dezelfde sleutels
// hebben (TypeScript dwingt dat af). Meervoud: "enkelvoud|meervoud" met een {count}-parameter.

import type { Lang } from "@/lib/types";
import { en } from "./en";
import { fr } from "./fr";
import { nl } from "./nl";

/** Maakt van elke tekst `string` (en van tekstlijsten `string[]`), zodat en/fr hetzelfde moeten hebben. */
export type DeepStrings<T> = {
  [K in keyof T]: T[K] extends string ? string : T[K] extends readonly string[] ? readonly string[] : DeepStrings<T[K]>;
};
export type Dict = DeepStrings<typeof nl>;

type Leaves<T, P extends string = ""> = {
  [K in keyof T & string]: T[K] extends string | readonly string[] ? `${P}${K}` : Leaves<T[K], `${P}${K}.`>;
}[keyof T & string];

/** Alle geldige sleutels, bv. "nav.dashboard". */
export type TKey = Leaves<typeof nl>;
export type Params = Record<string, string | number | null | undefined>;

const dicts: Record<Lang, Dict> = { nl, en, fr };

export const LANGS: { id: Lang; label: string }[] = [
  { id: "nl", label: "Nederlands" },
  { id: "en", label: "English" },
  { id: "fr", label: "Français" },
];

export const LOCALES: Record<Lang, string> = { nl: "nl-NL", en: "en-GB", fr: "fr-FR" };

function lookup(dict: unknown, key: string): unknown {
  let cur: unknown = dict;
  for (const part of key.split(".")) {
    if (cur && typeof cur === "object" && part in (cur as Record<string, unknown>)) {
      cur = (cur as Record<string, unknown>)[part];
    } else {
      return undefined;
    }
  }
  return cur;
}

function interpolate(text: string, params?: Params): string {
  let s = text;
  if (params && "count" in params && s.includes("|")) {
    const [one, other] = s.split("|");
    s = Number(params.count) === 1 ? one : other;
  }
  return s.replace(/\{(\w+)\}/g, (_, k: string) => {
    const v = params?.[k];
    return v === undefined || v === null ? `{${k}}` : String(v);
  });
}

/** Vertaal een sleutel. Valt terug op Engels, daarna op de sleutel zelf. */
export function translate(lang: Lang, key: string, params?: Params): string {
  const v = lookup(dicts[lang], key) ?? lookup(dicts.en, key);
  if (typeof v === "string") return interpolate(v, params);
  return key;
}

/** Lijst met teksten (bijvoorbeeld technische details). */
export function translateList(lang: Lang, key: string): string[] {
  const v = lookup(dicts[lang], key) ?? lookup(dicts.en, key);
  return Array.isArray(v) ? (v as string[]) : [];
}

/** Bestaat deze sleutel? */
export function hasKey(lang: Lang, key: string): boolean {
  return lookup(dicts[lang], key) !== undefined;
}

/** Bepaal de taal: eigen keuze, anders de taal van Windows, anders Engels. */
export function resolveLang(setting: Lang | null | undefined, windowsLang?: string | null): Lang {
  if (setting) return setting;
  const candidates = [windowsLang ?? "", ...(typeof navigator !== "undefined" ? navigator.languages ?? [] : [])];
  for (const c of candidates) {
    const two = c.slice(0, 2).toLowerCase();
    if (two === "nl" || two === "en" || two === "fr") return two;
  }
  return "en";
}

