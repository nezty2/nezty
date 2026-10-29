// De licentielogica. De server bepaalt altijd of een sleutel geldig is en hoeveel tijd er over is;
// de app krijgt alleen een ondertekend token met die uitkomst.

import { hashPassword, hmacHex, randomToken, safeEqual, sha256Hex, signToken, verifyPassword } from "./crypto.ts";
import { newKey, normalize } from "./keys.ts";
import type { LicenseRow, LicenseType, SessionRow, Store, UserRow } from "./store.ts";

export interface Env {
  signingKey: CryptoKey;
  /** Geheim om sleutels te hashen (HMAC). */
  keyPepper: string;
  /** SHA-256 (hex) van de beheerderssleutel. De sleutel zelf staat alleen op de pc van de eigenaar. */
  adminKeySha256: string;
  now?: () => number;
}

export interface Reply {
  status: number;
  body: Record<string, unknown>;
}

const MINUTE = 60_000;
const DAY = 24 * 60 * MINUTE;
const DURATION: Record<LicenseType, number> = { month: 30 * 24 * 60, demo: 120 };
/** Zonder internet doorwerken: maandlicentie 24 uur, demo alleen online (kort token, vaak verversen). */
const OFFLINE_MS: Record<LicenseType, number> = { month: DAY, demo: 5 * MINUTE };
const SESSION_MS = 90 * DAY;
const MAX_FAILED = 10;
const LOCK_MS = 15 * MINUTE;

export type LicenseStatus = "unused" | "active" | "expired" | "revoked";

export function statusOf(l: LicenseRow, now: number): LicenseStatus {
  if (l.revoked_at) return "revoked";
  if (!l.activated_at || !l.expires_at) return "unused";
  return now >= Date.parse(l.expires_at) ? "expired" : "active";
}

const ok = (body: Record<string, unknown> = {}): Reply => ({ status: 200, body: { ok: true, ...body } });
const fail = (error: string, status = 400, extra: Record<string, unknown> = {}): Reply => ({ status, body: { ok: false, error, ...extra } });
const iso = (ms: number) => new Date(ms).toISOString();

function str(v: unknown, max = 200): string | null {
  return typeof v === "string" && v.length > 0 && v.length <= max ? v : null;
}

const USERNAME = /^[A-Za-z0-9_.-]{3,24}$/;
const DEVICE = /^[a-f0-9]{32,64}$/;

async function keyHash(env: Env, normalized: string) {
  return await hmacHex(env.keyPepper, normalized);
}

async function settings(store: Store) {
  const s = await store.getSettings();
  const contact = (s.contact ?? {}) as Record<string, unknown>;
  return {
    demoEnabled: s.demo_enabled === true,
    contact: {
      discord: typeof contact.discord === "string" ? contact.discord : "",
      email: typeof contact.email === "string" ? contact.email : "",
      website: typeof contact.website === "string" ? contact.website : "",
      note: typeof contact.note === "string" ? contact.note : "",
    },
  };
}

/** Token voor de app: wat de server over deze licentie zegt, ondertekend. */
async function tokenFor(env: Env, now: number, l: LicenseRow, user: UserRow | null, device: string) {
  const status = statusOf(l, now);
  const payload = {
    v: 1,
    u: user?.id ?? null,
    n: user?.username ?? null,
    l: l.id,
    t: l.type,
    s: status,
    a: l.activated_at ? Date.parse(l.activated_at) : null,
    e: l.expires_at ? Date.parse(l.expires_at) : null,
    h: l.key_hint,
    d: device,
    i: now,
    c: now + OFFLINE_MS[l.type],
  };
  return await signToken(env.signingKey, payload);
}

async function newSession(store: Store, now: number, l: LicenseRow, userId: string | null, device: string) {
  const secret = randomToken(32);
  const row: SessionRow = {
    id: crypto.randomUUID(),
    token_hash: await sha256Hex(secret),
    user_id: userId,
    license_id: l.id,
    device_id: device,
    created_at: iso(now),
    last_seen_at: iso(now),
    expires_at: iso(now + SESSION_MS),
  };
  await store.insertSession(row);
  return secret;
}

/** Eén apparaat per sleutel. Na "reset-apparaat" koppelt de eerstvolgende login het nieuwe apparaat. */
async function checkDevice(store: Store, l: LicenseRow, device: string): Promise<boolean> {
  if (!l.device_id) {
    await store.updateLicense(l.id, { device_id: device });
    l.device_id = device;
    return true;
  }
  return l.device_id === device;
}

// ---------- Voor de app ----------

async function register(store: Store, env: Env, now: number, b: Record<string, unknown>): Promise<Reply> {
  const username = str(b.username, 24);
  const password = str(b.password, 200);
  const device = str(b.device, 64);
  const email = b.email == null || b.email === "" ? null : str(b.email, 200);
  if (!username || !USERNAME.test(username)) return fail("username_invalid");
  if (!password || password.length < 8) return fail("password_weak");
  if (!device || !DEVICE.test(device)) return fail("bad_request");
  if (b.email && !email) return fail("email_invalid");
  const key = normalize(String(b.key ?? ""));
  if (!key) return fail("key_format");

  const l = await store.licenseByHash(await keyHash(env, key));
  if (!l) return fail("key_unknown", 404);
  const status = statusOf(l, now);
  if (status === "revoked") return fail("key_revoked");
  if (l.user_id) return fail("key_in_use");
  if (await store.userByName(username.toLowerCase())) return fail("username_taken");

  const user: UserRow = {
    id: crypto.randomUUID(),
    username,
    username_lower: username.toLowerCase(),
    password_hash: await hashPassword(password),
    email,
    created_at: iso(now),
    last_login_at: iso(now),
    failed_logins: 0,
    locked_until: null,
  };
  if (!(await store.insertUser(user))) return fail("username_taken");

  // De tijd gaat lopen bij het activeren, niet bij het maken van de sleutel.
  const patch: Partial<LicenseRow> = { user_id: user.id, device_id: device };
  if (status === "unused") {
    patch.activated_at = iso(now);
    patch.expires_at = iso(now + l.duration_minutes * MINUTE);
  }
  if (!(await store.claimLicense(l.id, patch))) return fail("key_in_use");
  Object.assign(l, patch);

  const session = await newSession(store, now, l, user.id, device);
  return ok({ session, token: await tokenFor(env, now, l, user, device) });
}

async function login(store: Store, env: Env, now: number, b: Record<string, unknown>): Promise<Reply> {
  const username = str(b.username, 24);
  const password = str(b.password, 200);
  const device = str(b.device, 64);
  if (!username || !password || !device || !DEVICE.test(device)) return fail("login_failed", 401);
  const user = await store.userByName(username.toLowerCase());
  if (!user) {
    // Even lang rekenen als bij een echte gebruiker, zodat de tijd niet verraadt of de naam bestaat.
    await hashPassword(password);
    return fail("login_failed", 401);
  }
  if (user.locked_until && Date.parse(user.locked_until) > now) return fail("too_many_attempts", 429);
  if (!(await verifyPassword(password, user.password_hash))) {
    const failed = user.failed_logins + 1;
    await store.updateUser(user.id, failed >= MAX_FAILED ? { failed_logins: 0, locked_until: iso(now + LOCK_MS) } : { failed_logins: failed });
    return fail("login_failed", 401);
  }
  const l = await store.licenseByUser(user.id);
  if (!l) return fail("no_license", 404);
  if (!(await checkDevice(store, l, device))) return fail("device_mismatch", 403);
  await store.updateUser(user.id, { failed_logins: 0, locked_until: null, last_login_at: iso(now) });
  // Eén sessie per gebruiker: een nieuwe login sluit de oude.
  await store.deleteSessionsWhere("user_id", user.id);
  const session = await newSession(store, now, l, user.id, device);
  return ok({ session, token: await tokenFor(env, now, l, user, device) });
}

async function refresh(store: Store, env: Env, now: number, b: Record<string, unknown>): Promise<Reply> {
  const secret = str(b.session, 100);
  const device = str(b.device, 64);
  if (!secret || !device) return fail("session_invalid", 401);
  const s = await store.sessionByHash(await sha256Hex(secret));
  if (!s || Date.parse(s.expires_at) <= now) return fail("session_invalid", 401);
  if (s.device_id !== device) return fail("device_mismatch", 403);
  const l = await store.licenseById(s.license_id);
  if (!l) return fail("session_invalid", 401);
  if (l.device_id && l.device_id !== device) return fail("device_mismatch", 403);
  const user = s.user_id ? await store.userById(s.user_id) : null;
  await store.updateSession(s.id, { last_seen_at: iso(now), expires_at: iso(now + SESSION_MS) });
  return ok({ token: await tokenFor(env, now, l, user, device) });
}

async function logout(store: Store, b: Record<string, unknown>): Promise<Reply> {
  const secret = str(b.session, 100);
  if (secret) {
    const s = await store.sessionByHash(await sha256Hex(secret));
    if (s) await store.deleteSession(s.id);
  }
  return ok();
}

async function demo(store: Store, env: Env, now: number, b: Record<string, unknown>): Promise<Reply> {
  const device = str(b.device, 64);
  if (!device || !DEVICE.test(device)) return fail("bad_request");
  if (!(await settings(store)).demoEnabled) return fail("demo_disabled", 403);
  if (await store.demoDeviceUsed(device)) return fail("demo_used", 403);
  await store.markDemoDevice(device);
  const l: LicenseRow = {
    id: crypto.randomUUID(),
    key_hash: `demo:${randomToken(16)}`,
    key_hint: "DEMO",
    type: "demo",
    duration_minutes: DURATION.demo,
    created_at: iso(now),
    activated_at: iso(now),
    expires_at: iso(now + DURATION.demo * MINUTE),
    revoked_at: null,
    user_id: null,
    device_id: device,
    note: "demo-knop",
  };
  await store.insertLicense(l);
  const session = await newSession(store, now, l, null, device);
  return ok({ session, token: await tokenFor(env, now, l, null, device) });
}

// ---------- Alleen voor de eigenaar (beheerderssleutel) ----------

async function findLicense(store: Store, env: Env, b: Record<string, unknown>): Promise<LicenseRow | Reply> {
  if (typeof b.key === "string" && b.key) {
    const key = normalize(b.key);
    if (!key) return fail("key_format");
    const l = await store.licenseByHash(await keyHash(env, key));
    return l ?? fail("key_unknown", 404);
  }
  if (typeof b.id === "string" && b.id.length >= 6) {
    const list = await store.licensesByIdPrefix(b.id.toLowerCase());
    if (list.length === 1) return list[0];
    return fail(list.length === 0 ? "key_unknown" : "ambiguous", list.length === 0 ? 404 : 400);
  }
  return fail("bad_request");
}

function describe(l: LicenseRow, users: Map<string, UserRow>, now: number) {
  return {
    id: l.id,
    hint: l.key_hint,
    type: l.type,
    status: statusOf(l, now),
    username: l.user_id ? (users.get(l.user_id)?.username ?? null) : null,
    device: l.device_id ? l.device_id.slice(0, 8) : null,
    createdAt: l.created_at,
    activatedAt: l.activated_at,
    expiresAt: l.expires_at,
    durationDays: Math.round((l.duration_minutes / 1440) * 100) / 100,
    durationMinutes: l.duration_minutes,
    note: l.note,
  };
}

async function adminCreate(store: Store, env: Env, now: number, b: Record<string, unknown>): Promise<Reply> {
  const type = b.type === "demo" ? "demo" : b.type === "month" ? "month" : null;
  if (!type) return fail("bad_request");
  const count = Math.min(100, Math.max(1, Math.floor(Number(b.count ?? 1)) || 1));
  const note = str(b.note, 200);
  const keys: string[] = [];
  for (let i = 0; i < count; i++) {
    const key = newKey();
    const norm = normalize(key)!;
    await store.insertLicense({
      id: crypto.randomUUID(),
      key_hash: await keyHash(env, norm),
      key_hint: norm.slice(-4),
      type,
      duration_minutes: DURATION[type],
      created_at: iso(now),
      activated_at: null,
      expires_at: null,
      revoked_at: null,
      user_id: null,
      device_id: null,
      note,
    });
    keys.push(key);
  }
  return ok({ keys, type });
}

async function adminExtend(store: Store, env: Env, now: number, b: Record<string, unknown>): Promise<Reply> {
  const found = await findLicense(store, env, b);
  if ("status" in found && "body" in found) return found;
  const l = found as LicenseRow;
  const days = Number(b.days);
  if (!Number.isFinite(days) || days <= 0 || days > 3650) return fail("bad_request");
  const add = Math.round(days * 1440);
  const st = statusOf(l, now);
  const patch: Partial<LicenseRow> = { revoked_at: null };
  if (st === "unused") {
    patch.duration_minutes = l.duration_minutes + add;
  } else {
    // Nog actief: achter de huidige einddatum. Al verlopen: vanaf nu.
    const from = st === "active" && l.expires_at ? Date.parse(l.expires_at) : now;
    patch.expires_at = iso(from + add * MINUTE);
  }
  await store.updateLicense(l.id, patch);
  const users = new Map((await store.listUsers()).map((u) => [u.id, u]));
  return ok({ license: describe({ ...l, ...patch }, users, now) });
}

async function adminSimple(store: Store, env: Env, now: number, b: Record<string, unknown>, action: "revoke" | "reset-device" | "info") {
  const found = await findLicense(store, env, b);
  if ("status" in found && "body" in found) return found;
  const l = found as LicenseRow;
  if (action === "revoke") {
    l.revoked_at = iso(now);
    await store.updateLicense(l.id, { revoked_at: l.revoked_at });
  } else if (action === "reset-device") {
    l.device_id = null;
    await store.updateLicense(l.id, { device_id: null });
    await store.deleteSessionsWhere("license_id", l.id);
  }
  const users = new Map((await store.listUsers()).map((u) => [u.id, u]));
  return ok({ license: describe(l, users, now) });
}

async function adminList(store: Store, now: number): Promise<Reply> {
  const users = new Map((await store.listUsers()).map((u) => [u.id, u]));
  const list = (await store.listLicenses()).sort((a, b) => b.created_at.localeCompare(a.created_at));
  return ok({ licenses: list.map((l) => describe(l, users, now)) });
}

async function adminResetPassword(store: Store, b: Record<string, unknown>): Promise<Reply> {
  const username = str(b.username, 24);
  if (!username) return fail("bad_request");
  const user = await store.userByName(username.toLowerCase());
  if (!user) return fail("user_unknown", 404);
  // Geen wachtwoord meegegeven: een tijdelijk wachtwoord dat de eigenaar aan de klant doorgeeft.
  const password = str(b.password, 200) ?? randomToken(9);
  if (password.length < 8) return fail("password_weak");
  await store.updateUser(user.id, { password_hash: await hashPassword(password), failed_logins: 0, locked_until: null });
  await store.deleteSessionsWhere("user_id", user.id);
  return ok({ username: user.username, password });
}

async function adminSettings(store: Store, b: Record<string, unknown>): Promise<Reply> {
  if (typeof b.demoEnabled === "boolean") await store.setSetting("demo_enabled", b.demoEnabled);
  if (b.contact && typeof b.contact === "object") {
    const cur = (await settings(store)).contact as Record<string, string>;
    const next = { ...cur };
    for (const k of ["discord", "email", "website", "note"]) {
      const v = (b.contact as Record<string, unknown>)[k];
      if (typeof v === "string") next[k] = v.slice(0, 300);
    }
    await store.setSetting("contact", next);
  }
  return ok(await settings(store));
}

// ---------- Ingang ----------

export async function handle(path: string, body: Record<string, unknown>, headers: { get(name: string): string | null }, store: Store, env: Env): Promise<Reply> {
  const now = (env.now ?? Date.now)();
  try {
    switch (path) {
      case "/config":
        return ok(await settings(store));
      case "/register":
        return await register(store, env, now, body);
      case "/login":
        return await login(store, env, now, body);
      case "/refresh":
        return await refresh(store, env, now, body);
      case "/logout":
        return await logout(store, body);
      case "/demo":
        return await demo(store, env, now, body);
    }
    if (path.startsWith("/admin/")) {
      const given = headers.get("x-admin-key") ?? "";
      if (!given || !env.adminKeySha256 || !safeEqual(await sha256Hex(given), env.adminKeySha256.toLowerCase())) return fail("unauthorized", 401);
      switch (path) {
        case "/admin/create":
          return await adminCreate(store, env, now, body);
        case "/admin/extend":
          return await adminExtend(store, env, now, body);
        case "/admin/revoke":
          return await adminSimple(store, env, now, body, "revoke");
        case "/admin/reset-device":
          return await adminSimple(store, env, now, body, "reset-device");
        case "/admin/info":
          return await adminSimple(store, env, now, body, "info");
        case "/admin/list":
          return await adminList(store, now);
        case "/admin/reset-password":
          return await adminResetPassword(store, body);
        case "/admin/settings":
          return await adminSettings(store, body);
      }
    }
    return fail("not_found", 404);
  } catch (e) {
    console.error("license error", path, e);
    return fail("server_error", 500);
  }
}
