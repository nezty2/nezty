// Opslag in Supabase (Postgres) via de service role. Alleen voor de Edge Function (Deno).

import { createClient } from "jsr:@supabase/supabase-js@2";
import type { LicenseRow, SessionRow, Store, UserRow } from "./store.ts";

export function supabaseStore(url: string, serviceKey: string): Store {
  const db = createClient(url, serviceKey, { auth: { persistSession: false, autoRefreshToken: false } });

  const one = async <T>(q: PromiseLike<{ data: T[] | null; error: unknown }>): Promise<T | null> => {
    const { data, error } = await q;
    if (error) throw error;
    return data && data.length > 0 ? data[0] : null;
  };
  const many = async <T>(q: PromiseLike<{ data: T[] | null; error: unknown }>): Promise<T[]> => {
    const { data, error } = await q;
    if (error) throw error;
    return data ?? [];
  };
  const run = async (q: PromiseLike<{ error: unknown }>) => {
    const { error } = await q;
    if (error) throw error;
  };

  return {
    licenseByHash: (hash) => one<LicenseRow>(db.from("licenses").select("*").eq("key_hash", hash).limit(1)),
    licenseById: (id) => one<LicenseRow>(db.from("licenses").select("*").eq("id", id).limit(1)),
    licenseByUser: (userId) => one<LicenseRow>(db.from("licenses").select("*").eq("user_id", userId).limit(1)),
    // Een uuid-kolom kan niet met "like"; daarom op tekst vergelijken via een gefilterde lijst.
    licensesByIdPrefix: async (prefix) => (await many<LicenseRow>(db.from("licenses").select("*"))).filter((l) => l.id.startsWith(prefix)),
    listLicenses: () => many<LicenseRow>(db.from("licenses").select("*")),
    insertLicense: (row) => run(db.from("licenses").insert(row)),
    updateLicense: (id, patch) => run(db.from("licenses").update(patch).eq("id", id)),
    claimLicense: async (id, patch) => {
      const { data, error } = await db.from("licenses").update(patch).eq("id", id).is("user_id", null).select("id");
      if (error) throw error;
      return (data ?? []).length > 0;
    },

    userByName: (lower) => one<UserRow>(db.from("users").select("*").eq("username_lower", lower).limit(1)),
    userById: (id) => one<UserRow>(db.from("users").select("*").eq("id", id).limit(1)),
    listUsers: () => many<UserRow>(db.from("users").select("*")),
    insertUser: async (row) => {
      const { error } = await db.from("users").insert(row);
      if (error) {
        // 23505 = unieke waarde bestaat al (gebruikersnaam bezet).
        if ((error as { code?: string }).code === "23505") return false;
        throw error;
      }
      return true;
    },
    updateUser: (id, patch) => run(db.from("users").update(patch).eq("id", id)),

    sessionByHash: (hash) => one<SessionRow>(db.from("sessions").select("*").eq("token_hash", hash).limit(1)),
    insertSession: (row) => run(db.from("sessions").insert(row)),
    updateSession: (id, patch) => run(db.from("sessions").update(patch).eq("id", id)),
    deleteSession: (id) => run(db.from("sessions").delete().eq("id", id)),
    deleteSessionsWhere: (field, value) => run(db.from("sessions").delete().eq(field, value)),

    demoDeviceUsed: async (deviceId) => (await one(db.from("demo_devices").select("device_id").eq("device_id", deviceId).limit(1))) !== null,
    markDemoDevice: (deviceId) => run(db.from("demo_devices").upsert({ device_id: deviceId })),

    getSettings: async () => {
      const rows = await many<{ key: string; value: unknown }>(db.from("settings").select("*"));
      return Object.fromEntries(rows.map((r) => [r.key, r.value]));
    },
    setSetting: (key, value) => run(db.from("settings").upsert({ key, value })),
  };
}
