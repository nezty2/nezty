import { useEffect, useState } from "react";
import { api, fileUrl } from "@/lib/ipc";
import { useApp } from "@/store/app";

/** Achtergrond achter de hele app: de eigen afbeelding (vervaagd en gedimd) of een zachte gloed in de accentkleur. */
export function AppBackground() {
  const image = useApp((s) => s.settings?.backgroundImage ?? null);
  const dim = useApp((s) => s.settings?.backgroundDim ?? 55);
  const blur = useApp((s) => s.settings?.backgroundBlur ?? 6);
  const [url, setUrl] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    if (!image) {
      setUrl(null);
      return;
    }
    api
      .getBackgroundFile()
      .then((p) => (p ? fileUrl(p) : null))
      .then((u) => alive && setUrl(u))
      .catch(() => alive && setUrl(null));
    return () => {
      alive = false;
    };
  }, [image]);

  useEffect(() => {
    document.documentElement.dataset.bg = url ? "on" : "off";
  }, [url]);

  return (
    <div aria-hidden className="pointer-events-none fixed inset-0 overflow-hidden">
      {url ? (
        <>
          <img src={url} alt="" className="absolute inset-0 h-full w-full object-cover" style={{ filter: `blur(${blur}px)`, transform: "scale(1.06)" }} />
          <div className="absolute inset-0" style={{ background: `rgb(0 0 0 / ${Math.min(90, dim) / 100})` }} />
        </>
      ) : (
        <>
          <div className="glow glow-a" />
          <div className="glow glow-b" />
        </>
      )}
    </div>
  );
}
