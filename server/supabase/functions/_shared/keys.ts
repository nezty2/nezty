// Licentiesleutels: NZT-XXXX-XXXX-XXXX-XXXX.
// 15 willekeurige tekens plus 1 controleteken (Luhn mod 32). Zonder 0, 1, I en O, zodat niemand
// letters en cijfers verwart. 15 tekens uit 32 = 75 bits: niet te raden.
// Dit bestand wordt gedeeld door de server, de generator en (als kopie in Rust) de app.

export const ALPHABET = "23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
export const PREFIX = "NZT";
const N = ALPHABET.length;

function checkChar(body: string): string {
  let factor = 2;
  let sum = 0;
  for (let i = body.length - 1; i >= 0; i--) {
    let addend = factor * ALPHABET.indexOf(body[i]);
    factor = factor === 2 ? 1 : 2;
    addend = Math.floor(addend / N) + (addend % N);
    sum += addend;
  }
  return ALPHABET[(N - (sum % N)) % N];
}

/** Nieuwe sleutel in leesbare vorm. */
export function newKey(): string {
  const bytes = new Uint8Array(15);
  crypto.getRandomValues(bytes);
  // 256 is deelbaar door 32: modulo geeft geen scheve verdeling.
  const body = Array.from(bytes, (b) => ALPHABET[b % N]).join("");
  return format(body + checkChar(body));
}

export function format(chars16: string): string {
  return `${PREFIX}-${chars16.slice(0, 4)}-${chars16.slice(4, 8)}-${chars16.slice(8, 12)}-${chars16.slice(12, 16)}`;
}

/**
 * Maakt van wat de klant typt de 16 tekens (zonder voorvoegsel en streepjes).
 * Geeft null als het geen geldige sleutel kan zijn (verkeerde lengte, vreemde tekens of typfout).
 */
export function normalize(input: string): string | null {
  let s = input.toUpperCase().replace(/[\s-]/g, "");
  if (s.startsWith(PREFIX) && s.length === PREFIX.length + 16) s = s.slice(PREFIX.length);
  if (s.length !== 16) return null;
  for (const c of s) if (!ALPHABET.includes(c)) return null;
  if (checkChar(s.slice(0, 15)) !== s[15]) return null;
  return s;
}
