import { useEffect, useState } from "react";
import { fileUrl } from "@/lib/ipc";
import type { Game } from "@/lib/types";
import { useAppIcons } from "./useAppIcons";
import { cn } from "./ui";

function initials(name: string): string {
  const words = name.replace(/[^\p{L}\p{N} ]/gu, " ").split(/\s+/).filter(Boolean);
  if (words.length === 0) return "?";
  if (words.length === 1) return words[0].slice(0, 2).toUpperCase();
  return (words[0][0] + words[1][0]).toUpperCase();
}

/** Programma's waarvan het pictogram niets over de game zegt (Minecraft Java draait via Java). */
function genericExe(path: string | null): boolean {
  if (!path) return true;
  const file = path.split(/[\\/]/).pop()?.toLowerCase() ?? "";
  return file === "javaw.exe" || file === "java.exe";
}

/** Laadt een lokaal bestand via het asset-protocol. */
function useFileSrc(path: string | null | undefined): string | null {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    if (!path) {
      setSrc(null);
      return;
    }
    let alive = true;
    void fileUrl(path).then((u) => alive && setSrc(u));
    return () => {
      alive = false;
    };
  }, [path]);
  return src;
}

/** Echte afbeelding van de game, of een rustige placeholder met het pictogram van de game. */
export function GameCover({ game, className }: { game: Game; className?: string }) {
  const [coverFailed, setCoverFailed] = useState(false);
  const [iconFailed, setIconFailed] = useState(false);
  const coverSrc = useFileSrc(game.cover);
  const ownIconSrc = useFileSrc(game.cover ? null : game.icon);
  const exe = !game.cover && !game.icon && !genericExe(game.exePath) ? game.exePath : null;
  const icons = useAppIcons(exe ? [exe] : []);
  const exeIcon = exe ? icons[exe] : null;

  useEffect(() => setCoverFailed(false), [game.cover]);
  useEffect(() => setIconFailed(false), [game.icon]);

  if (coverSrc && !coverFailed) {
    return (
      <img src={coverSrc} alt="" onError={() => setCoverFailed(true)} className={cn("h-full w-full object-cover", className)} draggable={false} />
    );
  }

  // Neutrale achtergrond: geen kleurverloop per game, alleen het paneel met een zacht licht in het midden.
  return (
    <div
      className={cn("flex h-full w-full items-center justify-center bg-panel-2", className)}
      style={{ backgroundImage: "radial-gradient(circle at 50% 45%, rgba(255,255,255,0.06), transparent 65%)" }}
    >
      {ownIconSrc && !iconFailed ? (
        <img
          src={ownIconSrc}
          alt=""
          onError={() => setIconFailed(true)}
          className="h-[46%] max-h-24 w-auto object-contain drop-shadow-[0_4px_10px_rgba(0,0,0,0.45)]"
          draggable={false}
        />
      ) : exeIcon ? (
        <img src={exeIcon} alt="" className="h-12 w-12 rounded-lg" draggable={false} />
      ) : (
        <span className="text-2xl font-bold tracking-tight text-ink-2">{initials(game.name)}</span>
      )}
    </div>
  );
}
