import { useEffect, useState } from "react";
import { CrosshairShape } from "@/components/CrosshairShape";
import { api, listenEvent } from "@/lib/ipc";
import type { CrosshairConfig } from "@/lib/types";
import { EVENTS } from "@/lib/types";

/** Inhoud van het doorklikbare overlayvenster: alleen de crosshair, in het midden. */
export function CrosshairOverlay() {
  const [cfg, setCfg] = useState<CrosshairConfig | null>(null);

  useEffect(() => {
    document.documentElement.classList.add("overlay");
    void api.getCrosshairConfig().then(setCfg).catch(() => undefined);
    let off: (() => void) | undefined;
    void listenEvent<CrosshairConfig>(EVENTS.crosshairConfig, setCfg).then((u) => (off = u));
    return () => off?.();
  }, []);

  if (!cfg) return null;
  return (
    <div style={{ position: "fixed", inset: 0, display: "flex", alignItems: "center", justifyContent: "center", pointerEvents: "none" }}>
      <CrosshairShape cfg={cfg} box={400} />
    </div>
  );
}
