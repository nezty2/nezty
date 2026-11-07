// Runs the desktop tests against the actual license handlers in memory.
import { createServer } from "node:http";
import { randomUUID } from "node:crypto";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { memoryStore } from "../dev/memory_store.ts";
import { handle } from "../supabase/functions/_shared/core.ts";
import { newSigningKeyPair, importSigningKey, sha256Hex } from "../supabase/functions/_shared/crypto.ts";

const pair = await newSigningKeyPair();
const admin = randomUUID();
const store = memoryStore(null);
let clock = Date.now();
const env = { signingKey: await importSigningKey(pair.privatePkcs8), keyPepper: randomUUID(), adminKeySha256: await sha256Hex(admin), now: () => clock };
const server = createServer(async (req, res) => {
  try {
    const chunks = [];
    for await (const chunk of req) chunks.push(chunk);
    const body = JSON.parse(Buffer.concat(chunks).toString() || "{}");
    const path = req.url!;
    if (path === "/fixture/clock" && body.admin === admin) {
      clock += 31 * 86_400_000;
      res.end(JSON.stringify({ ok: true }));
      return;
    }
    const reply = await handle(path, body, { get: name => req.headers[name] as string ?? null }, store, env);
    res.writeHead(reply.status, { "content-type": "application/json" }).end(JSON.stringify(reply.body));
  } catch {
    res.writeHead(500).end();
  }
});
server.listen(0, "127.0.0.1");
await once(server, "listening");
try {
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("Missing test server port");
  const runner = spawn("pwsh", ["-NoProfile", "-File", "scripts/test-herstel.ps1", "-LicenseFixture"], {
    stdio: "inherit",
    env: { ...process.env, NEZTY_LICENSE_URL: `http://127.0.0.1:${address.port}`, NEZTY_LICENSE_PUBKEY: pair.publicRaw,
      NEZTY_FIXTURE_ADMIN: admin },
  });
  const [code] = await once(runner, "exit");
  process.exitCode = code ?? 1;
} finally { server.close(); }
