// Cryptografie met alleen WebCrypto: werkt in Supabase Edge Functions (Deno) en in Node.

const enc = new TextEncoder();

export function b64url(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

export function b64decode(s: string): Uint8Array {
  const norm = s.replace(/-/g, "+").replace(/_/g, "/");
  const bin = atob(norm + "===".slice((norm.length + 3) % 4));
  return Uint8Array.from(bin, (c) => c.charCodeAt(0));
}

function hex(buf: ArrayBuffer): string {
  return Array.from(new Uint8Array(buf), (b) => b.toString(16).padStart(2, "0")).join("");
}

export function randomToken(bytes = 32): string {
  const b = new Uint8Array(bytes);
  crypto.getRandomValues(b);
  return b64url(b);
}

export async function sha256Hex(text: string): Promise<string> {
  return hex(await crypto.subtle.digest("SHA-256", enc.encode(text)));
}

export async function hmacHex(secret: string, text: string): Promise<string> {
  const key = await crypto.subtle.importKey("raw", enc.encode(secret), { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
  return hex(await crypto.subtle.sign("HMAC", key, enc.encode(text)));
}

/** Vergelijken zonder dat de tijd verraadt hoeveel tekens kloppen. */
export function safeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let d = 0;
  for (let i = 0; i < a.length; i++) d |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return d === 0;
}

// Wachtwoorden: PBKDF2-SHA256 met 600.000 rondes (OWASP-advies) en een eigen zout per wachtwoord.
const ROUNDS = 600_000;

async function pbkdf2(password: string, salt: Uint8Array, rounds: number): Promise<Uint8Array> {
  const base = await crypto.subtle.importKey("raw", enc.encode(password), "PBKDF2", false, ["deriveBits"]);
  const bits = await crypto.subtle.deriveBits({ name: "PBKDF2", hash: "SHA-256", salt, iterations: rounds }, base, 256);
  return new Uint8Array(bits);
}

export async function hashPassword(password: string): Promise<string> {
  const salt = new Uint8Array(16);
  crypto.getRandomValues(salt);
  return `pbkdf2$${ROUNDS}$${b64url(salt)}$${b64url(await pbkdf2(password, salt, ROUNDS))}`;
}

export async function verifyPassword(password: string, stored: string): Promise<boolean> {
  const [alg, rounds, salt, hash] = stored.split("$");
  if (alg !== "pbkdf2" || !rounds || !salt || !hash) return false;
  const got = b64url(await pbkdf2(password, b64decode(salt), Number(rounds)));
  return safeEqual(got, hash);
}

// Ondertekende tokens: Ed25519. De server heeft de privésleutel, de app alleen de publieke sleutel.
export async function importSigningKey(pkcs8Base64: string): Promise<CryptoKey> {
  return await crypto.subtle.importKey("pkcs8", b64decode(pkcs8Base64), { name: "Ed25519" }, false, ["sign"]);
}

export async function signToken(key: CryptoKey, payload: unknown): Promise<string> {
  const body = `v1.${b64url(enc.encode(JSON.stringify(payload)))}`;
  const sig = new Uint8Array(await crypto.subtle.sign("Ed25519", key, enc.encode(body)));
  return `${body}.${b64url(sig)}`;
}

export async function verifyToken(publicRawBase64: string, token: string): Promise<unknown | null> {
  const parts = token.split(".");
  if (parts.length !== 3 || parts[0] !== "v1") return null;
  const key = await crypto.subtle.importKey("raw", b64decode(publicRawBase64), { name: "Ed25519" }, false, ["verify"]);
  const ok = await crypto.subtle.verify("Ed25519", key, b64decode(parts[2]), enc.encode(`${parts[0]}.${parts[1]}`));
  return ok ? JSON.parse(new TextDecoder().decode(b64decode(parts[1]))) : null;
}

/** Nieuw sleutelpaar voor tokens: privé (pkcs8) voor de server, publiek (32 bytes) voor de app. */
export async function newSigningKeyPair(): Promise<{ privatePkcs8: string; publicRaw: string }> {
  const pair = (await crypto.subtle.generateKey({ name: "Ed25519" }, true, ["sign", "verify"])) as CryptoKeyPair;
  const priv = new Uint8Array(await crypto.subtle.exportKey("pkcs8", pair.privateKey));
  const pub = new Uint8Array(await crypto.subtle.exportKey("raw", pair.publicKey));
  return { privatePkcs8: b64url(priv), publicRaw: b64url(pub) };
}
