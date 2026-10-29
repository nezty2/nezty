import { useId, useMemo, useState } from "react";

/** Eén reeks, dunne lijn met lichte vulling. Bij hover verschijnt de waarde op dat punt. */
export function Sparkline({ values, min, max, height = 40, format, className }: { values: (number | null)[]; min?: number; max?: number; height?: number; format?: (v: number) => string; className?: string }) {
  const id = useId();
  const [hover, setHover] = useState<number | null>(null);
  const W = 200;
  const H = height;
  const pad = 2;

  const { line, area, points } = useMemo(() => {
    const nums = values.filter((v): v is number => v != null);
    if (nums.length < 2) return { line: "", area: "", points: [] as { x: number; y: number; v: number | null }[] };
    const lo = min ?? Math.min(...nums);
    const hi0 = max ?? Math.max(...nums);
    const hi = hi0 === lo ? lo + 1 : hi0;
    const n = values.length;
    const pts = values.map((v, i) => {
      const x = pad + (i / Math.max(1, n - 1)) * (W - pad * 2);
      const y = v == null ? H - pad : pad + (1 - (Math.min(hi, Math.max(lo, v)) - lo) / (hi - lo)) * (H - pad * 2);
      return { x, y, v };
    });
    let l = "";
    let started = false;
    for (const p of pts) {
      if (p.v == null) {
        started = false;
        continue;
      }
      l += `${started ? "L" : "M"}${p.x.toFixed(1)},${p.y.toFixed(1)} `;
      started = true;
    }
    const first = pts.find((p) => p.v != null);
    const last = [...pts].reverse().find((p) => p.v != null);
    const a = first && last ? `${l} L${last.x.toFixed(1)},${H} L${first.x.toFixed(1)},${H} Z` : "";
    return { line: l, area: a, points: pts };
  }, [values, min, max, H]);

  const hovered = hover != null ? points[hover] : null;
  return (
    <div className={className} style={{ position: "relative" }}>
      <svg
        viewBox={`0 0 ${W} ${H}`}
        preserveAspectRatio="none"
        className="block h-full w-full"
        onMouseMove={(e) => {
          const rect = e.currentTarget.getBoundingClientRect();
          const ratio = (e.clientX - rect.left) / rect.width;
          setHover(Math.max(0, Math.min(points.length - 1, Math.round(ratio * (points.length - 1)))));
        }}
        onMouseLeave={() => setHover(null)}
      >
        <defs>
          <linearGradient id={`${id}-f`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="var(--accent)" stopOpacity="0.22" />
            <stop offset="100%" stopColor="var(--accent)" stopOpacity="0" />
          </linearGradient>
        </defs>
        {area && <path d={area} fill={`url(#${id}-f)`} />}
        {line && <path d={line} fill="none" stroke="var(--accent)" strokeWidth="1.75" strokeLinejoin="round" strokeLinecap="round" vectorEffect="non-scaling-stroke" />}
        {hovered && hovered.v != null && <line x1={hovered.x} x2={hovered.x} y1={0} y2={H} stroke="#3a404d" strokeWidth="1" vectorEffect="non-scaling-stroke" />}
      </svg>
      {hovered && hovered.v != null && (
        <div
          className="pointer-events-none absolute -top-1 rounded-md border border-line-2 bg-panel px-1.5 py-0.5 text-[11px] text-ink tnum"
          style={{ left: `${(hovered.x / W) * 100}%`, transform: "translate(-50%, -100%)" }}
        >
          {format ? format(hovered.v) : hovered.v}
        </div>
      )}
    </div>
  );
}
