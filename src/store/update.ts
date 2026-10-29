import { useEffect } from "react";
import { create } from "zustand";
import { checkForUpdate, installUpdate, updateConfigured, type UpdateInfo } from "@/lib/updater";

export type UpdateStatus = "idle" | "checking" | "none" | "available" | "installing" | "error" | "installError" | "unconfigured";

interface UpdateStore {
  status: UpdateStatus;
  info: UpdateInfo | null;
  progress: number | null;
  /** "Later" gekozen: de balk blijft deze sessie weg, in Instellingen blijft de update zichtbaar. */
  dismissed: boolean;
  check: () => Promise<void>;
  install: () => Promise<void>;
  dismiss: () => void;
}

export const useUpdate = create<UpdateStore>()((set, get) => ({
  status: "idle",
  info: null,
  progress: null,
  dismissed: false,

  async check() {
    if (get().status === "checking" || get().status === "installing") return;
    set({ status: "checking" });
    try {
      if (!(await updateConfigured())) {
        set({ status: "unconfigured" });
        return;
      }
      const info = await checkForUpdate();
      set({ info, status: info ? "available" : "none" });
    } catch {
      set({ status: "error" });
    }
  },

  async install() {
    if (!get().info) return;
    set({ status: "installing", progress: null });
    try {
      await installUpdate((pct) => set({ progress: pct }));
    } catch {
      set({ status: "installError" });
    }
  },

  dismiss() {
    set({ dismissed: true });
  },
}));

/**
 * Updates zijn verplicht: zoek meteen bij het openen (ook tijdens de opstartanimatie en op het inlogscherm),
 * daarna elke 30 minuten en als het venster na een kwartier terugkomt. Een gevonden update installeert
 * ForceUpdate direct.
 */
export function useAutoUpdateCheck(ready: boolean) {
  const check = useUpdate((s) => s.check);
  useEffect(() => {
    if (!ready) return;
    let last = 0;
    const run = () => {
      const s = useUpdate.getState().status;
      if (s === "available" || s === "installing" || s === "checking") return;
      last = Date.now();
      void check();
    };
    const first = setTimeout(run, 1000);
    const every = setInterval(run, 30 * 60 * 1000);
    const onVisible = () => {
      if (document.visibilityState === "visible" && Date.now() - last > 15 * 60 * 1000) run();
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      clearTimeout(first);
      clearInterval(every);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [ready, check]);
}
