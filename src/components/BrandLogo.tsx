import { cn } from "./ui";

interface BrandLogoProps {
  className?: string;
  size?: number;
  glow?: boolean;
}

/**
 * Het officiële Hyper-N merklogo van nezty tweaks.
 * 100% strakke vector met dynamische verlichting en diepte.
 */
export function BrandLogo({ className, size = 32, glow = true }: BrandLogoProps) {
  return (
    <div
      className={cn("relative inline-flex items-center justify-center shrink-0 select-none", className)}
      style={{ width: size, height: size }}
    >
      {glow && (
        <div
          className="absolute inset-0 rounded-2xl bg-accent/30 blur-md pointer-events-none"
          style={{ transform: "scale(1.15)" }}
        />
      )}
      <svg
        viewBox="0 0 1024 1024"
        width="100%"
        height="100%"
        className="relative z-10 w-full h-full drop-shadow-md"
      >
        <defs>
          <linearGradient id="bl_bg" x1="0.5" y1="0" x2="0.5" y2="1">
            <stop offset="0%" stopColor="#151229" />
            <stop offset="100%" stopColor="#080612" />
          </linearGradient>

          <linearGradient id="bl_top_rim" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="#ffffff" stopOpacity="0.25" />
            <stop offset="100%" stopColor="#ffffff" stopOpacity="0.02" />
          </linearGradient>

          <linearGradient id="bl_accent_rim" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0%" stopColor="#a855f7" stopOpacity="0.5" />
            <stop offset="50%" stopColor="#6366f1" stopOpacity="0.2" />
            <stop offset="100%" stopColor="#38bdf8" stopOpacity="0.4" />
          </linearGradient>

          <linearGradient id="bl_pillar_left" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="#818cf8" />
            <stop offset="45%" stopColor="#4f46e5" />
            <stop offset="100%" stopColor="#312e81" />
          </linearGradient>

          <linearGradient id="bl_pillar_right" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="#c084fc" />
            <stop offset="50%" stopColor="#9333ea" />
            <stop offset="100%" stopColor="#581c87" />
          </linearGradient>

          <linearGradient id="bl_diagonal_beam" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0%" stopColor="#ffffff" />
            <stop offset="25%" stopColor="#67e8f9" />
            <stop offset="65%" stopColor="#c084fc" />
            <stop offset="100%" stopColor="#9333ea" />
          </linearGradient>
        </defs>

        <rect x="48" y="48" width="928" height="928" rx="240" fill="url(#bl_bg)" />
        <rect x="48" y="48" width="928" height="928" rx="240" fill="none" stroke="url(#bl_top_rim)" strokeWidth="8" />
        <rect x="52" y="52" width="920" height="920" rx="236" fill="none" stroke="url(#bl_accent_rim)" strokeWidth="10" />

        <path
          d="M250 240 C250 196 284 164 326 164 C368 164 402 196 402 240 V784 C402 828 368 860 326 860 C284 860 250 828 250 784 Z"
          fill="url(#bl_pillar_left)"
        />
        <ellipse cx="326" cy="205" rx="55" ry="25" fill="#ffffff" opacity="0.25" />

        <path
          d="M622 240 C622 196 656 164 698 164 C740 164 774 196 774 240 V784 C774 828 740 860 698 860 C656 860 622 828 622 784 Z"
          fill="url(#bl_pillar_right)"
        />
        <ellipse cx="698" cy="205" rx="55" ry="25" fill="#ffffff" opacity="0.25" />

        <path d="M336 172 L770 736 L688 852 L254 288 Z" fill="url(#bl_diagonal_beam)" />
        <path d="M336 172 L770 736" stroke="#ffffff" strokeWidth="10" strokeLinecap="round" opacity="0.55" />
      </svg>
    </div>
  );
}
