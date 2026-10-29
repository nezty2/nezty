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
