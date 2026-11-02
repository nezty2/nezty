import { create } from "zustand";
import { api } from "@/lib/ipc";
import type { GameServer, NetRegion, RouteReport } from "@/lib/types";

/** Gekozen game en regio blijven bewaard (alleen in deze browseropslag, gaat mis zonder: dan de standaard). */
function load(key: string): string | null {
  try {
    return window.localStorage.getItem(`nezty.netopt.${key}`);
  } catch {
    return null;
  }
}
function save(key: string, value: string | null) {
  try {
    if (value == null) window.localStorage.removeItem(`nezty.netopt.${key}`);
    else window.localStorage.setItem(`nezty.netopt.${key}`, value);
  } catch {
    /* geen opslag: dan alleen voor deze sessie */
  }
}

/** Wat er gemeten wordt: de echte server van de game, of een gekozen datacenter. */
export interface NetTarget {
  host: string;
  kind: "server" | "region";
  region: NetRegion | null;
}

interface NetOptStore {
  gameId: string | null;
  regionId: string | null;
  regions: NetRegion[];
  server: GameServer | null;
  route: RouteReport | null;
  routeHost: string | null;
  routeLoading: boolean;
  setGame: (id: string | null) => void;
  setRegion: (id: string | null) => void;
  setServer: (s: GameServer | null) => void;
  loadRegions: () => Promise<void>;
  runRoute: (host: string) => Promise<RouteReport | null>;
}

export const useNetOpt = create<NetOptStore>()((set, get) => ({
  gameId: load("game"),
  regionId: load("region"),
  regions: [],
  server: null,
  route: null,
  routeHost: null,
  routeLoading: false,

  setGame(id) {
    save("game", id);
    set({ gameId: id, server: null });
  },
  setRegion(id) {
    save("region", id);
    set({ regionId: id });
  },
  setServer(s) {
    set({ server: s });
  },
  async loadRegions() {
    if (get().regions.length) return;
    const regions = await api.gamenetRegions();
    set({ regions });
  },
  async runRoute(host) {
    set({ routeLoading: true, routeHost: host });
    try {
      const route = await api.gamenetRoute(host);
      // Intussen een ander doel gekozen: dan deze uitkomst niet meer tonen.
      if (get().routeHost !== host) return null;
      set({ route });
      return route;
    } finally {
      if (get().routeHost === host) set({ routeLoading: false });
    }
  },
}));

/** Het meetdoel bij de huidige keuze: de echte server als die gevonden is, anders het gekozen datacenter. */
export function targetOf(server: GameServer | null, regions: NetRegion[], regionId: string | null, fallbackRegion: string): NetTarget | null {
  if (server?.status === "found" && server.ip && !server.ip.includes(":")) return { host: server.ip, kind: "server", region: null };
  const region = regions.find((r) => r.id === regionId) ?? regions.find((r) => r.id === fallbackRegion) ?? regions[0];
  return region ? { host: region.host, kind: "region", region } : null;
}
