import logoSvg from "./logo.svg";
import { cn } from "./ui";

interface BrandLogoProps {
  className?: string;
  size?: number;
  glow?: boolean;
}

/**
 * Het officiële Hyper-N merklogo van nezty tweaks.
 * Laadt via native img tag voor gegarandeerde weergave van alle gradients en schaduwen.
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
      <img
        src={logoSvg}
        alt="nezty"
        width={size}
        height={size}
        className="relative z-10 h-full w-full rounded-lg object-contain drop-shadow-md pointer-events-none"
      />
    </div>
  );
}
