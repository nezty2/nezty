// Updates via de eigen updater van nezty (backend). Een update wordt alleen geïnstalleerd als de
// handtekening klopt met de openbare sleutel in de app. De installer werkt in updatemodus:
// instellingen en logboek blijven staan, en nezty start daarna vanzelf opnieuw.
import { isTauri, listenEvent } from "./ipc";

export interface UpdateInfo {
  version: string;
  notes: string | null;
}

async function invoke<R>(cmd: string): Promise<R> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<R>(cmd);
}

/** Is er een updatebron ingebouwd? In de browser (nepdata) doen we alsof. */
export async function updateConfigured(): Promise<boolean> {
  if (!isTauri) return true;
  return invoke<boolean>("update_configured");
}

/** Zoekt naar een nieuwere versie. Geeft null als je de nieuwste hebt. */
export async function checkForUpdate(): Promise<UpdateInfo | null> {
  if (!isTauri) {
    await new Promise((r) => setTimeout(r, 600));
    // Preview: ?update=1 doet alsof er een nieuwe versie is.
    return new URLSearchParams(location.search).get("update") ? { version: "9.9.9", notes: "Voorbeeld van wat er nieuw is." } : null;
  }
  return invoke<UpdateInfo | null>("check_update");
}

/** Downloadt en installeert de update. nezty sluit daarna en start na de installatie opnieuw. */
export async function installUpdate(onProgress: (pct: number | null) => void): Promise<void> {
  if (!isTauri) {
    // Preview: voortgang laten zien en dan mislukken, zodat ook het foutscherm te zien is.
    for (let p = 0; p <= 100; p += 20) {
      onProgress(p);
      await new Promise((r) => setTimeout(r, 300));
    }
    throw new Error("not available in the browser");
  }
  onProgress(null);
  const off = await listenEvent<number>("update-progress", (pct) => onProgress(pct));
  try {
    await invoke<void>("install_update");
  } finally {
    off();
  }
}
