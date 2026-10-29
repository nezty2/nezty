// Supabase Edge Function "license". Eén functie met routes:
//   /config /register /login /refresh /logout /demo        (de app)
//   /admin/create /admin/extend /admin/revoke /admin/reset-device /admin/info /admin/list
//   /admin/reset-password /admin/settings                   (de generator, met beheerderssleutel)
//
// Geheimen (zet ze met `supabase secrets set`, nooit in de code):
//   LICENSE_SIGNING_KEY  privésleutel voor tokens (pkcs8, base64url)
//   LICENSE_KEY_PEPPER   geheim om sleutels te hashen
//   ADMIN_KEY_SHA256     SHA-256 van je beheerderssleutel
// SUPABASE_URL en SUPABASE_SERVICE_ROLE_KEY zet Supabase zelf.

import { handle, type Env } from "../_shared/core.ts";
import { importSigningKey } from "../_shared/crypto.ts";
import { supabaseStore } from "../_shared/supabase_store.ts";

const need = (name: string) => {
  const v = Deno.env.get(name);
  if (!v) throw new Error(`secret ${name} ontbreekt`);
  return v;
};

const store = supabaseStore(need("SUPABASE_URL"), need("SUPABASE_SERVICE_ROLE_KEY"));
const env: Env = {
  signingKey: await importSigningKey(need("LICENSE_SIGNING_KEY")),
  keyPepper: need("LICENSE_KEY_PEPPER"),
  adminKeySha256: need("ADMIN_KEY_SHA256"),
};

Deno.serve(async (req) => {
  if (req.method !== "POST") return new Response(JSON.stringify({ ok: false, error: "method" }), { status: 405 });
  const url = new URL(req.url);
  // Pad na ".../license", bijvoorbeeld "/login".
  const path = url.pathname.replace(/^.*\/license/, "") || "/";
  let body: Record<string, unknown> = {};
  try {
    const text = await req.text();
    body = text ? JSON.parse(text) : {};
  } catch {
    return new Response(JSON.stringify({ ok: false, error: "bad_request" }), { status: 400 });
  }
  const r = await handle(path, body, req.headers, store, env);
  return new Response(JSON.stringify(r.body), { status: r.status, headers: { "content-type": "application/json" } });
});
