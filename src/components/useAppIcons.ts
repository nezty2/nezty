import { useEffect, useState } from "react";
import { api } from "@/lib/ipc";

// Gedeelde cache, zodat elk pictogram maar één keer wordt opgehaald.
const cache = new Map<string, string | null>();
const pending = new Set<string>();
const subscribers = new Set<() => void>();

async function load(paths: string[]) {
  const todo = paths.filter((p) => !cache.has(p) && !pending.has(p));
  if (todo.length === 0) return;
  todo.forEach((p) => pending.add(p));
  try {
    const res = await api.getIcons(todo);
    for (const p of todo) cache.set(p, res[p] ?? null);
  } catch {
    for (const p of todo) cache.set(p, null);
  } finally {
    todo.forEach((p) => pending.delete(p));
    subscribers.forEach((s) => s());
  }
}

/** Pictogrammen (PNG-data-URL) van programma's, opgehaald via de backend. */
export function useAppIcons(paths: (string | null | undefined)[]): Record<string, string | null> {
  const list = paths.filter((p): p is string => !!p);
  const key = list.join("|");
  const [, force] = useState(0);
  useEffect(() => {
    const sub = () => force((n) => n + 1);
    subscribers.add(sub);
    void load(list);
    return () => {
      subscribers.delete(sub);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  const out: Record<string, string | null> = {};
  for (const p of list) out[p] = cache.get(p) ?? null;
  return out;
}
