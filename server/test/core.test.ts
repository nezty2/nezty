// Tests voor de licentielogica:  node --test server/test/core.test.ts

import assert from "node:assert/strict";
import { test } from "node:test";
import { handle, type Env } from "../supabase/functions/_shared/core.ts";
import { importSigningKey, newSigningKeyPair, sha256Hex, verifyToken } from "../supabase/functions/_shared/crypto.ts";
import { normalize, newKey, ALPHABET } from "../supabase/functions/_shared/keys.ts";
import { memoryStore } from "../dev/memory_store.ts";

const DEV_A = "a".repeat(64);
const DEV_B = "b".repeat(64);
const ADMIN = "beheer-test-sleutel";
const HOUR = 3_600_000;
const DAY = 24 * HOUR;

async function setup() {
  const pair = await newSigningKeyPair();
  let clock = Date.parse("2026-11-01T12:00:00Z");
  const env: Env = { signingKey: await importSigningKey(pair.privatePkcs8), keyPepper: "peper", adminKeySha256: await sha256Hex(ADMIN), now: () => clock };
  const store = memoryStore(null);
  const call = async (path: string, body: Record<string, unknown> = {}, admin = false) =>
    (await handle(path, body, { get: (n) => (admin && n === "x-admin-key" ? ADMIN : null) }, store, env)).body as Record<string, any>;
  const token = async (t: string) => (await verifyToken(pair.publicRaw, t)) as Record<string, any>;
  return { call, token, advance: (ms: number) => (clock += ms), now: () => clock };
}

test("sleutels: formaat, controleteken en typfouten", () => {
  for (let i = 0; i < 200; i++) {
    const k = newKey();
    assert.match(k, /^nezty-[2-9a-hj-np-z]{4}-[2-9a-hj-np-z]{4}-[2-9a-hj-np-z]{3}$/);
    assert.ok(normalize(k));
    assert.ok(normalize(k.toUpperCase().replace(/-/g, " ")), "hoofdletters en spaties mogen");
    assert.ok(normalize(k.replace(/^nezty-/, "")), "zonder nezty ervoor mag ook");
    // Eén teken veranderen moet altijd opvallen.
    const chars = k.replace(/^nezty-/, "").replace(/-/g, "");
    const pos = i % 11;
    const other = ALPHABET[(ALPHABET.indexOf(chars[pos]) + 1 + (i % 30)) % 32];
    const typo = chars.slice(0, pos) + other + chars.slice(pos + 1);
    assert.equal(normalize(typo), null, `typfout op positie ${pos} niet gezien`);
  }
  assert.equal(normalize("nezty-0000-1111-ioi"), null);
  assert.equal(normalize("nezty-2345-6789-abc") === null || normalize("nezty-2345-6789-abd") === null, true, "niet elk rijtje is geldig");
});

test("account maken: tijd start bij activeren, één apparaat, sleutel maar één keer", async () => {
  const { call, token, advance, now } = await setup();
  const made = await call("/admin/create", { type: "month" }, true);
  assert.equal(made.ok, true);
  const key = made.keys[0];
  advance(5 * DAY); // de sleutel ligt even op de plank
  const reg = await call("/register", { username: "Speler_1", password: "geheim123", key, device: DEV_A });
  assert.equal(reg.ok, true, JSON.stringify(reg));
  const t = await token(reg.token);
  assert.equal(t.s, "active");
  assert.equal(t.t, "month");
  assert.equal(t.e - t.a, 30 * DAY, "30 dagen vanaf activeren");
  assert.equal(t.a, now());
  assert.equal(t.c - t.i, DAY, "maximaal 24 uur offline");

  const again = await call("/register", { username: "Iemand", password: "geheim123", key, device: DEV_B });
  assert.equal(again.error, "key_in_use");
  const taken = await call("/register", { username: "speler_1", password: "geheim123", key: (await call("/admin/create", { type: "month" }, true)).keys[0], device: DEV_B });
  assert.equal(taken.error, "username_taken");
  assert.equal((await call("/register", { username: "x", password: "geheim123", key, device: DEV_A })).error, "username_invalid");
  assert.equal((await call("/register", { username: "nieuw", password: "kort", key, device: DEV_A })).error, "password_weak");
  const last = key.slice(-1);
  const typo = key.slice(0, -1) + ALPHABET[(ALPHABET.indexOf(last) + 1) % 32];
  assert.equal((await call("/register", { username: "nieuw", password: "geheim123", key: typo, device: DEV_A })).error, "key_format");
  const unknown = newKey();
  assert.equal((await call("/register", { username: "nieuw", password: "geheim123", key: unknown, device: DEV_A })).error, "key_unknown");
});

test("inloggen, ander apparaat, verkeerde wachtwoorden en verversen", async () => {
  const { call, token } = await setup();
  const key = (await call("/admin/create", { type: "month" }, true)).keys[0];
  await call("/register", { username: "speler", password: "geheim123", key, device: DEV_A });

  const li = await call("/login", { username: "SPELER", password: "geheim123", device: DEV_A });
  assert.equal(li.ok, true);
  assert.equal((await call("/login", { username: "speler", password: "geheim123", device: DEV_B })).error, "device_mismatch");
  assert.equal((await call("/login", { username: "speler", password: "fout", device: DEV_A })).error, "login_failed");
  assert.equal((await call("/login", { username: "bestaatniet", password: "fout", device: DEV_A })).error, "login_failed");

  const r = await call("/refresh", { session: li.session, device: DEV_A });
  assert.equal(r.ok, true);
  assert.equal((await token(r.token)).n, "speler");
  assert.equal((await call("/refresh", { session: li.session, device: DEV_B })).error, "device_mismatch");

  for (let i = 0; i < 10; i++) await call("/login", { username: "speler", password: "fout", device: DEV_A });
  assert.equal((await call("/login", { username: "speler", password: "geheim123", device: DEV_A })).error, "too_many_attempts");

  await call("/logout", { session: li.session });
  assert.equal((await call("/refresh", { session: li.session, device: DEV_A })).error, "session_invalid");
});

test("beheer: verlengen, intrekken, nieuw apparaat, wachtwoord resetten, lijst", async () => {
  const { call, token, advance } = await setup();
  assert.equal((await call("/admin/list", {}, false)).error, "unauthorized");
  const key = (await call("/admin/create", { type: "month", count: 3 }, true)).keys[0];
  const reg = await call("/register", { username: "klant", password: "geheim123", key, device: DEV_A });
  const before = await token(reg.token);

  const ext = await call("/admin/extend", { key, days: 30 }, true);
  assert.equal(ext.ok, true);
  const after = await token((await call("/refresh", { session: reg.session, device: DEV_A })).token);
  assert.equal(after.e - before.e, 30 * DAY, "verlengd achter de oude einddatum");

  advance(61 * DAY);
  const expired = await token((await call("/refresh", { session: reg.session, device: DEV_A })).token);
  assert.equal(expired.s, "expired");
  await call("/admin/extend", { key, days: 30 }, true);
  const renewed = await token((await call("/refresh", { session: reg.session, device: DEV_A })).token);
  assert.equal(renewed.s, "active");
  assert.ok(Math.abs(renewed.e - (renewed.i + 30 * DAY)) < 1000, "verlopen: 30 dagen vanaf nu");

  await call("/admin/revoke", { key }, true);
  assert.equal((await token((await call("/refresh", { session: reg.session, device: DEV_A })).token)).s, "revoked");
  await call("/admin/extend", { key, days: 1 }, true);
  assert.equal((await token((await call("/refresh", { session: reg.session, device: DEV_A })).token)).s, "active", "verlengen heft intrekken op");

  const reset = await call("/admin/reset-device", { key }, true);
  assert.equal(reset.license.device, null);
  assert.equal((await call("/refresh", { session: reg.session, device: DEV_A })).error, "session_invalid", "oude sessies vervallen");
  assert.equal((await call("/login", { username: "klant", password: "geheim123", device: DEV_B })).ok, true, "nieuwe pc mag");
  assert.equal((await call("/login", { username: "klant", password: "geheim123", device: DEV_A })).error, "device_mismatch");

  const pw = await call("/admin/reset-password", { username: "klant" }, true);
  assert.equal(pw.ok, true);
  assert.equal((await call("/login", { username: "klant", password: "geheim123", device: DEV_B })).error, "login_failed");
  assert.equal((await call("/login", { username: "klant", password: pw.password, device: DEV_B })).ok, true);

  const list = await call("/admin/list", {}, true);
  assert.equal(list.licenses.length, 3);
  const mine = list.licenses.find((l: any) => l.username === "klant");
  assert.equal(mine.status, "active");
  assert.equal(mine.hint, normalize(key)!.slice(-4));
  assert.equal(list.licenses.filter((l: any) => l.status === "unused").length, 2);
  const byId = await call("/admin/info", { id: mine.id.slice(0, 8) }, true);
  assert.equal(byId.license.username, "klant");
});

test("demo: 2 uur, alleen online, één keer per apparaat, aan/uit op de server", async () => {
  const { call, token } = await setup();
  assert.equal((await call("/demo", { device: DEV_A })).error, "demo_disabled");
  const cfg = await call("/admin/settings", { demoEnabled: true, contact: { discord: "nezty#1", email: "x@y.z" } }, true);
  assert.equal(cfg.demoEnabled, true);
  assert.equal((await call("/config")).contact.discord, "nezty#1");
  const d = await call("/demo", { device: DEV_A });
  assert.equal(d.ok, true);
  const t = await token(d.token);
  assert.equal(t.t, "demo");
  assert.equal(t.e - t.a, 2 * HOUR);
  assert.equal(t.c - t.i, 5 * 60_000, "demo werkt alleen online: kort token");
  assert.equal((await call("/demo", { device: DEV_A })).error, "demo_used");
  assert.equal((await call("/refresh", { session: d.session, device: DEV_A })).ok, true);
});
