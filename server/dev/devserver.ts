// Lokale licentieserver om te testen, zonder Supabase. Zelfde logica als de echte server.
//
//   node server/dev/devserver.ts --keys <secrets.json> [--data <data.json>] [--port 8787]
//
// <secrets.json> maak je met: npm run licentie:sleutels-maken -- --bestand <secrets.json>

import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { handle, type Env } from "../supabase/functions/_shared/core.ts";
import { importSigningKey } from "../supabase/functions/_shared/crypto.ts";
import { memoryStore } from "./memory_store.ts";

const arg = (name: string, fallback: string | null = null) => {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 && process.argv[i + 1] ? process.argv[i + 1] : fallback;
};

const keysFile = arg("keys");
if (!keysFile) {
  console.error("Gebruik: node server/dev/devserver.ts --keys <secrets.json> [--data <data.json>] [--port 8787]");
  process.exit(1);
}
const secrets = JSON.parse(readFileSync(keysFile, "utf8"));
const env: Env = {
  signingKey: await importSigningKey(secrets.LICENSE_SIGNING_KEY),
  keyPepper: secrets.LICENSE_KEY_PEPPER,
  adminKeySha256: secrets.ADMIN_KEY_SHA256,
};
const store = memoryStore(arg("data"));
const port = Number(arg("port", "8787"));

createServer(async (req, res) => {
  const chunks: Buffer[] = [];
  for await (const c of req) chunks.push(c as Buffer);
  let body: Record<string, unknown> = {};
  try {
    body = chunks.length ? JSON.parse(Buffer.concat(chunks).toString("utf8")) : {};
  } catch {
    res.writeHead(400, { "content-type": "application/json" }).end(JSON.stringify({ ok: false, error: "bad_request" }));
    return;
  }
  const path = (req.url ?? "/").split("?")[0].replace(/^.*\/license/, "") || "/";
  const r = await handle(path, body, { get: (n) => (req.headers[n.toLowerCase()] as string | undefined) ?? null }, store, env);
  res.writeHead(r.status, { "content-type": "application/json" }).end(JSON.stringify(r.body));
}).listen(port, "127.0.0.1", () => console.log(`licentieserver (test) op http://127.0.0.1:${port}/license`));
