// Opslag in het geheugen, met een JSON-bestand erachter. Alleen om lokaal te testen.

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import type { LicenseRow, SessionRow, Store, UserRow } from "../supabase/functions/_shared/store.ts";

interface Data {
  licenses: LicenseRow[];
  users: UserRow[];
  sessions: SessionRow[];
  demo: string[];
  settings: Record<string, unknown>;
}

export function memoryStore(file: string | null): Store {
  const d: Data =
    file && existsSync(file)
      ? JSON.parse(readFileSync(file, "utf8"))
      : { licenses: [], users: [], sessions: [], demo: [], settings: { demo_enabled: false, contact: { discord: "", email: "", website: "", note: "" } } };
  const save = () => {
    if (file) writeFileSync(file, JSON.stringify(d, null, 2));
  };
  const copy = <T>(x: T | undefined): T | null => (x ? structuredClone(x) : null);

  return {
    licenseByHash: async (h) => copy(d.licenses.find((l) => l.key_hash === h)),
    licenseById: async (id) => copy(d.licenses.find((l) => l.id === id)),
    licenseByUser: async (u) => copy(d.licenses.find((l) => l.user_id === u)),
    licensesByIdPrefix: async (p) => d.licenses.filter((l) => l.id.startsWith(p)).map((l) => structuredClone(l)),
    listLicenses: async () => structuredClone(d.licenses),
    insertLicense: async (row) => {
      d.licenses.push(structuredClone(row));
      save();
    },
    updateLicense: async (id, patch) => {
      const l = d.licenses.find((x) => x.id === id);
      if (l) Object.assign(l, patch);
      save();
    },
    claimLicense: async (id, patch) => {
      const l = d.licenses.find((x) => x.id === id);
      if (!l || l.user_id) return false;
      Object.assign(l, patch);
      save();
      return true;
    },
    userByName: async (lower) => copy(d.users.find((u) => u.username_lower === lower)),
    userById: async (id) => copy(d.users.find((u) => u.id === id)),
    listUsers: async () => structuredClone(d.users),
    insertUser: async (row) => {
      if (d.users.some((u) => u.username_lower === row.username_lower)) return false;
      d.users.push(structuredClone(row));
      save();
      return true;
    },
    updateUser: async (id, patch) => {
      const u = d.users.find((x) => x.id === id);
      if (u) Object.assign(u, patch);
      save();
    },
    sessionByHash: async (h) => copy(d.sessions.find((s) => s.token_hash === h)),
    insertSession: async (row) => {
      d.sessions.push(structuredClone(row));
      save();
    },
    updateSession: async (id, patch) => {
      const s = d.sessions.find((x) => x.id === id);
      if (s) Object.assign(s, patch);
      save();
    },
    deleteSession: async (id) => {
      d.sessions = d.sessions.filter((s) => s.id !== id);
      save();
    },
    deleteSessionsWhere: async (field, value) => {
      d.sessions = d.sessions.filter((s) => s[field] !== value);
      save();
    },
    demoDeviceUsed: async (dev) => d.demo.includes(dev),
    markDemoDevice: async (dev) => {
      if (!d.demo.includes(dev)) d.demo.push(dev);
      save();
    },
    getSettings: async () => structuredClone(d.settings),
    setSetting: async (k, v) => {
      d.settings[k] = v;
      save();
    },
  };
}
