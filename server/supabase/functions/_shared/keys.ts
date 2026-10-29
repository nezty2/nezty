// Licentiesleutels: nezty-8273-ctaa-871.
// 10 willekeurige tekens plus 1 controleteken (Luhn mod 32). Kleine letters en cijfers, zonder 0, 1, i en o,
// zodat niemand tekens verwart. 10 tekens uit 32 = 50 bits: ruim duizend biljoen mogelijkheden, niet te raden
// (de server bepaalt of een sleutel bestaat). Hoofdletters en spaties bij het overtypen maken niet uit.
// Dit bestand wordt gedeeld door de server en de generator; de app heeft een kopie in src/lib/license.ts.

export const ALPHABET = "23456789abcdefghjklmnpqrstuvwxyz";
export const PREFIX = "nezty";
const N = ALPHABET.length;
const RANDOM = 10;

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
  const bytes = new Uint8Array(RANDOM);
  crypto.getRandomValues(bytes);
  // 256 is deelbaar door 32: modulo geeft geen scheve verdeling.
  const body = Array.from(bytes, (b) => ALPHABET[b % N]).join("");
  return format(body + checkChar(body));
}

/** 11 tekens naar nezty-xxxx-xxxx-xxx. */
export function format(chars: string): string {
  return `${PREFIX}-${chars.slice(0, 4)}-${chars.slice(4, 8)}-${chars.slice(8, 11)}`;
}

/**
 * Maakt van wat de klant typt de 11 tekens (zonder "nezty" en streepjes).
 * Geeft null als het geen geldige sleutel kan zijn (verkeerde lengte, vreemde tekens of typfout).
 */
export function normalize(input: string): string | null {
  let s = input.toLowerCase().replace(/[\s-]/g, "");
  if (s.startsWith(PREFIX) && s.length === PREFIX.length + RANDOM + 1) s = s.slice(PREFIX.length);
  if (s.length !== RANDOM + 1) return null;
  for (const c of s) if (!ALPHABET.includes(c)) return null;
  if (checkChar(s.slice(0, RANDOM)) !== s[RANDOM]) return null;
  return s;
}
