import { AnimatePresence, motion } from "framer-motion";
import { useEffect, useState, type ReactNode } from "react";
import { useApp } from "@/store/app";
import { cn } from "./ui";

/** Minder beweging aan (Instellingen): dan staan de ringen stil en springen ze direct naar de nieuwe stand. */
function useReduce(): boolean {
  return useApp((s) => s.settings?.reduceMotion ?? false);
}

export type RingTone = "accent" | "ready" | "pending" | "neutral";

const toneColor: Record<RingTone, string> = {
  accent: "var(--accent)",
  ready: "var(--color-ok)",
  pending: "var(--color-warn)",
  neutral: "var(--color-ink-3)",
};

/**
 * Ronde voortgangsring. `value` loopt van 0 tot 1. Met `busy` draait er een zachte lichtboog mee, zodat je ook
 * bij een lange stap (zoals het herstelpunt) ziet dat nezty bezig is.
 */
export function ProgressRing({
  value,
  tone = "accent",
  busy,
  size = 148,
  stroke = 8,
  children,
  className,
}: {
  value: number;
  tone?: RingTone;
  busy?: boolean;
  size?: number;
  stroke?: number;
  children?: ReactNode;
  className?: string;
}) {
  const reduce = useReduce();
  const r = (size - stroke) / 2;
  const c = size / 2;
  const color = toneColor[tone];
  const v = Math.max(0, Math.min(1, value || 0));
  return (
    <div
      className={cn("relative shrink-0", className)}
      style={{ width: size, height: size }}
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(v * 100)}
    >
      {/* Zachte gloed in de kleur van de ring, alleen als er iets te zien is. */}
      <div
        className="pointer-events-none absolute inset-[18%] rounded-full blur-2xl transition-opacity duration-500"
        style={{ background: color, opacity: v > 0 || busy ? 0.16 : 0 }}
      />
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} className="relative -rotate-90">
        <circle cx={c} cy={c} r={r} fill="none" stroke="var(--color-line)" strokeWidth={stroke} />
        <motion.circle
          cx={c}
          cy={c}
          r={r}
          fill="none"
          stroke={color}
          strokeWidth={stroke}
          strokeLinecap="round"
          initial={false}
          animate={{ pathLength: v, opacity: v > 0 ? 1 : 0 }}
          transition={reduce ? { duration: 0 } : { duration: 0.6, ease: [0.22, 1, 0.36, 1] }}
        />
      </svg>
      {busy && !reduce && (
        <motion.svg
          width={size}
          height={size}
          viewBox={`0 0 ${size} ${size}`}
          className="pointer-events-none absolute inset-0"
          animate={{ rotate: 360 }}
          transition={{ repeat: Infinity, duration: 2.4, ease: "linear" }}
        >
          <circle cx={c} cy={c} r={r} fill="none" stroke={color} strokeOpacity={0.35} strokeWidth={stroke} strokeLinecap="round" strokeDasharray={`${r * 0.5} ${r * 10}`} />
        </motion.svg>
      )}
      <div className="absolute inset-0 flex flex-col items-center justify-center text-center">{children}</div>
    </div>
  );
}

/**
 * Laadindicator voor een stuk pagina: een kleine draaiende ring met een korte tekst. Verschijnt pas na een korte
 * vertraging, zodat een snelle lading niet flitst.
 */
export function Loader({ label, className, delay = 180 }: { label?: string; className?: string; delay?: number }) {
  const reduce = useReduce();
  const [show, setShow] = useState(delay <= 0);
  useEffect(() => {
    if (delay <= 0) return;
    const id = window.setTimeout(() => setShow(true), delay);
    return () => window.clearTimeout(id);
  }, [delay]);
  const size = 36;
  const r = 15;
  return (
    <div role="status" aria-live="polite" className={cn("flex min-h-[140px] flex-col items-center justify-center gap-3", className)}>
      <AnimatePresence>
        {show && (
          <motion.div className="flex flex-col items-center gap-3" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.25 }}>
            <motion.svg
              width={size}
              height={size}
              viewBox={`0 0 ${size} ${size}`}
              animate={reduce ? undefined : { rotate: 360 }}
              transition={{ repeat: Infinity, duration: 1.3, ease: "linear" }}
            >
              <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="var(--color-line)" strokeWidth={3} />
              <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="var(--accent)" strokeWidth={3} strokeLinecap="round" strokeDasharray={`${r * 1.6} ${r * 10}`} />
            </motion.svg>
            {label && <div className="text-[13px] text-ink-3">{label}</div>}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
