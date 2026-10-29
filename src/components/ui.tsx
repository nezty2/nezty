import { AnimatePresence, motion } from "framer-motion";
import { Loader2, Search, X, type LucideIcon } from "lucide-react";
import { useEffect, useId, type ButtonHTMLAttributes, type ReactNode } from "react";
import { createPortal } from "react-dom";

export function cn(...parts: (string | false | null | undefined)[]): string {
  return parts.filter(Boolean).join(" ");
}

/* ---------- Opmaak ---------- */

export function Card({ children, className, onClick }: { children: ReactNode; className?: string; onClick?: () => void }) {
  return (
    <div className={cn("card p-5", onClick && "transition-colors hover:border-line-2", className)} onClick={onClick}>
      {children}
    </div>
  );
}

export function PageHeader({ title, subtitle, actions }: { title: string; subtitle?: ReactNode; actions?: ReactNode }) {
  return (
    <div className="mb-6 flex flex-wrap items-end justify-between gap-4">
      <div className="min-w-0">
        <h1 className="text-[22px] font-semibold tracking-tight">{title}</h1>
        {subtitle && <p className="mt-1 text-sm text-ink-2">{subtitle}</p>}
      </div>
      {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
    </div>
  );
}

export function SectionTitle({ children, right, sub }: { children: ReactNode; right?: ReactNode; sub?: ReactNode }) {
  return (
    <div className="mb-3 flex items-end justify-between gap-3">
      <div>
        <h2 className="text-[15px] font-semibold">{children}</h2>
        {sub && <p className="mt-0.5 text-[13px] text-ink-3">{sub}</p>}
      </div>
      {right}
    </div>
  );
}

/* ---------- Knoppen ---------- */

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "secondary" | "ghost" | "danger";
  size?: "sm" | "md" | "lg";
  loading?: boolean;
  icon?: LucideIcon;
};

export function Button({ variant = "secondary", size = "md", loading, icon: Icon, children, className, disabled, ...rest }: ButtonProps) {
  const sizes = { sm: "h-8 px-3 text-[13px]", md: "h-9 px-4 text-sm", lg: "h-11 px-5 text-[15px]" }[size];
  const variants = {
    primary: "bg-accent text-white hover:brightness-110",
    secondary: "bg-panel-2 border border-line text-ink hover:border-line-2 hover:bg-[#222733]",
    ghost: "text-ink-2 hover:bg-panel-2 hover:text-ink",
    danger: "bg-panel-2 border border-bad/40 text-bad hover:bg-bad/10",
  }[variant];
  return (
    <button
      className={cn("inline-flex shrink-0 items-center justify-center gap-2 rounded-lg font-medium transition-colors disabled:opacity-50", sizes, variants, className)}
      disabled={disabled || loading}
      {...rest}
    >
      {loading ? <Loader2 className="h-4 w-4 animate-spin" /> : Icon ? <Icon className="h-4 w-4" /> : null}
      {children}
    </button>
  );
}

export function IconButton({ icon: Icon, label, className, ...rest }: ButtonHTMLAttributes<HTMLButtonElement> & { icon: LucideIcon; label: string }) {
  return (
    <button
      aria-label={label}
      title={label}
      className={cn("inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-ink-2 transition-colors hover:bg-panel-2 hover:text-ink disabled:opacity-40", className)}
      {...rest}
    >
      <Icon className="h-4 w-4" />
    </button>
  );
}

/* ---------- Schakelaar: één vaste maat overal ---------- */

export function Toggle({ checked, onChange, disabled, loading, label }: { checked: boolean; onChange: (v: boolean) => void; disabled?: boolean; loading?: boolean; label?: string }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled || loading}
      onClick={(e) => {
        e.stopPropagation();
        onChange(!checked);
      }}
      className={cn(
        "relative inline-flex h-6 w-11 shrink-0 items-center rounded-full border transition-colors",
        checked ? "border-accent bg-accent" : "border-line-2 bg-[#252a35]",
        (disabled || loading) && "opacity-50",
      )}
    >
      <span
        className={cn(
          "absolute left-[2px] flex h-[18px] w-[18px] items-center justify-center rounded-full bg-white transition-transform duration-150",
          checked ? "translate-x-[20px]" : "translate-x-0",
        )}
      >
        {loading && <Loader2 className="h-3 w-3 animate-spin text-[#333]" />}
      </span>
    </button>
  );
}

/** Rij met tekst links en een bediening (bv. schakelaar) rechts, verticaal gecentreerd. */
export function Row({ title, desc, children, className }: { title: ReactNode; desc?: ReactNode; children?: ReactNode; className?: string }) {
  return (
    <div className={cn("flex items-center justify-between gap-6 py-3.5", className)}>
      <div className="min-w-0">
        <div className="text-sm font-medium">{title}</div>
        {desc && <div className="mt-0.5 text-[13px] leading-snug text-ink-3">{desc}</div>}
      </div>
      {children && <div className="flex shrink-0 items-center gap-2">{children}</div>}
    </div>
  );
}

/* ---------- Labels ---------- */

export type BadgeVariant = "neutral" | "accent" | "ok" | "warn" | "bad" | "info";

export function Badge({ variant = "neutral", children, icon: Icon, title, className }: { variant?: BadgeVariant; children: ReactNode; icon?: LucideIcon; title?: string; className?: string }) {
  const styles = {
    neutral: "bg-panel-2 text-ink-2 border-line",
    accent: "bg-accent-soft text-ink border-accent/40",
    ok: "bg-ok/10 text-ok border-ok/30",
    warn: "bg-warn/10 text-warn border-warn/30",
    bad: "bg-bad/10 text-bad border-bad/30",
    info: "bg-info/10 text-info border-info/30",
  }[variant];
  return (
    <span title={title} className={cn("inline-flex items-center gap-1 whitespace-nowrap rounded-md border px-1.5 py-0.5 text-[12px] font-medium leading-4", styles, className)}>
      {Icon && <Icon className="h-3 w-3" />}
      {children}
    </span>
  );
}

export function Chip({ active, onClick, children, icon: Icon }: { active?: boolean; onClick?: () => void; children: ReactNode; icon?: LucideIcon }) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "inline-flex items-center gap-1.5 whitespace-nowrap rounded-lg border px-3 py-1.5 text-[13px] font-medium transition-colors",
        active ? "border-accent bg-accent-soft text-ink" : "border-line bg-panel-2 text-ink-2 hover:border-line-2 hover:text-ink",
      )}
    >
      {Icon && <Icon className="h-3.5 w-3.5" />}
      {children}
    </button>
  );
}

export function SegmentedControl<V extends string>({ options, value, onChange }: { options: { value: V; label: string }[]; value: V; onChange: (v: V) => void }) {
  const id = useId();
  return (
    <div className="inline-flex rounded-lg border border-line bg-panel-2 p-0.5">
      {options.map((o) => (
        <button
          key={o.value}
          onClick={() => onChange(o.value)}
          className={cn("relative rounded-md px-3 py-1 text-[13px] font-medium transition-colors", value === o.value ? "text-ink" : "text-ink-3 hover:text-ink-2")}
        >
          {value === o.value && <motion.span layoutId={`seg-${id}`} className="absolute inset-0 rounded-md bg-[#2a303c]" transition={{ type: "spring", stiffness: 500, damping: 40 }} />}
          <span className="relative">{o.label}</span>
        </button>
      ))}
    </div>
  );
}

/* ---------- Invoer ---------- */

export function SearchInput({ value, onChange, placeholder, className }: { value: string; onChange: (v: string) => void; placeholder?: string; className?: string }) {
  return (
    <div className={cn("relative", className)}>
      <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-ink-3" />
      <input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className="h-9 w-full rounded-lg border border-line bg-panel-2 pl-9 pr-8 text-sm text-ink placeholder:text-ink-3 focus:border-accent focus:outline-none"
      />
      {value && (
        <button onClick={() => onChange("")} className="absolute right-2 top-1/2 -translate-y-1/2 rounded p-1 text-ink-3 hover:text-ink" aria-label="clear">
          <X className="h-3.5 w-3.5" />
        </button>
      )}
    </div>
  );
}

export function TextInput({ value, onChange, placeholder, className, type = "text", mono }: { value: string; onChange: (v: string) => void; placeholder?: string; className?: string; type?: string; mono?: boolean }) {
  return (
    <input
      type={type}
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      className={cn(
        "h-9 w-full rounded-lg border border-line bg-panel-2 px-3 text-sm text-ink placeholder:text-ink-3 focus:border-accent focus:outline-none",
        mono && "font-mono text-[13px]",
        className,
      )}
    />
  );
}

/* ---------- Overig ---------- */

export function Spinner({ className }: { className?: string }) {
  return <Loader2 className={cn("h-5 w-5 animate-spin text-ink-3", className)} />;
}

export function EmptyState({ icon: Icon, title, description, action }: { icon: LucideIcon; title: string; description?: string; action?: ReactNode }) {
  return (
    <div className="flex flex-col items-center justify-center rounded-xl border border-dashed border-line px-6 py-12 text-center">
      <Icon className="mb-3 h-7 w-7 text-ink-3" />
      <div className="text-sm font-medium">{title}</div>
      {description && <div className="mt-1 max-w-md text-[13px] text-ink-3">{description}</div>}
      {action && <div className="mt-4">{action}</div>}
    </div>
  );
}

export function Kv({ k, v, mono }: { k: ReactNode; v: ReactNode; mono?: boolean }) {
  return (
    <div className="flex items-start justify-between gap-4 py-2 text-sm">
      <span className="text-ink-3">{k}</span>
      <span className={cn("selectable text-right text-ink", mono && "font-mono text-[13px]")}>{v}</span>
    </div>
  );
}

export function Skeleton({ className }: { className?: string }) {
  return <div className={cn("animate-pulse rounded-lg bg-panel-2", className)} />;
}

/* ---------- Venster (modal): ondoorzichtig, zodat niets erdoorheen schijnt ---------- */

export function Modal({
  open,
  onClose,
  title,
  subtitle,
  children,
  footer,
  width = "max-w-2xl",
}: {
  open: boolean;
  onClose: () => void;
  title: ReactNode;
  subtitle?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  width?: string;
}) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  return createPortal(
    <AnimatePresence>
      {open && (
        <motion.div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.12 }}
          onMouseDown={(e) => {
            if (e.target === e.currentTarget) onClose();
          }}
        >
          <motion.div
            role="dialog"
            aria-modal="true"
            className={cn("flex max-h-[88vh] w-full flex-col overflow-hidden rounded-xl border border-line-2 bg-panel shadow-2xl", width)}
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 6 }}
            transition={{ duration: 0.15 }}
          >
            <div className="flex items-start justify-between gap-4 border-b border-line px-6 py-4">
              <div className="min-w-0">
                <div className="text-[17px] font-semibold leading-tight">{title}</div>
                {subtitle && <div className="mt-1 text-[13px] text-ink-3">{subtitle}</div>}
              </div>
              <IconButton icon={X} label="close" onClick={onClose} />
            </div>
            <div className="scroll-thin min-h-0 flex-1 overflow-y-auto px-6 py-5">{children}</div>
            {footer && <div className="flex flex-wrap items-center justify-end gap-2 border-t border-line px-6 py-4">{footer}</div>}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>,
    document.body,
  );
}
