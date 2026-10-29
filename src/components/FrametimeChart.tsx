import { useState } from "react";
import { useT } from "@/i18n/hooks";

/**
 * Frametijd over de meting (ms, langste frame per tijdvak). Lager en vlakker is beter, pieken zijn
 * haperingen. Twee grafieken naast elkaar krijgen dezelfde schaal (`max`), zodat je ze kunt vergelijken.
 */
export function FrametimeChart({ title, data, max, median, seconds }: { title: string; data: number[]; max: number; median: number; seconds: number }) {
  const { t, lang } = useT();
  const f1 = (v: number) => v.toLocaleString(lang, { minimumFractionDigits: 1, maximumFractionDigits: 1 });
  const [hover, setHover] = useState<number | null>(null);
  const W = 480;
  const H = 120;
  const pad = { l: 34, r: 8, t: 8, b: 18 };
  const iw = W - pad.l - pad.r;
  const ih = H - pad.t - pad.b;
  const x = (i: number) => pad.l + (i / Math.max(1, data.length - 1)) * iw;
  const y = (v: number) => pad.t + ih - (Math.min(v, max) / max) * ih;
  const points = data.map((v, i) => `${x(i).toFixed(1)},${y(v).toFixed(1)}`).join(" ");
  const ticks = [0, Math.round(max / 2), Math.round(max)];

  return (
    <div className="min-w-0">
      <div className="mb-1 flex items-baseline justify-between gap-2">
        <span className="text-[13px] font-medium text-ink">{title}</span>
        <span className="text-[12px] tnum text-ink-3">
          {hover != null ? t("measure.chartHover", { s: ((hover / Math.max(1, data.length - 1)) * seconds).toFixed(0), ms: f1(data[hover]) }) : t("measure.chartMedian", { ms: f1(median) })}
        </span>
      </div>
      <svg
        viewBox={`0 0 ${W} ${H}`}
        className="h-auto w-full"
        role="img"
        aria-label={title}
        onMouseLeave={() => setHover(null)}
        onMouseMove={(e) => {
          const r = e.currentTarget.getBoundingClientRect();
          const px = ((e.clientX - r.left) / r.width) * W;
          const i = Math.round(((px - pad.l) / iw) * (data.length - 1));
          setHover(i >= 0 && i < data.length ? i : null);
        }}
      >
        {ticks.map((tv) => (
          <g key={tv}>
            <line x1={pad.l} x2={W - pad.r} y1={y(tv)} y2={y(tv)} stroke="var(--color-line)" strokeWidth={1} />
            <text x={pad.l - 6} y={y(tv) + 3} textAnchor="end" fontSize={10} fill="var(--color-ink-3)">
              {tv}
            </text>
          </g>
        ))}
        <line x1={pad.l} x2={W - pad.r} y1={y(median)} y2={y(median)} stroke="var(--color-ink-3)" strokeWidth={1} strokeDasharray="3 3" />
        <polyline points={points} fill="none" stroke="var(--color-accent)" strokeWidth={2} strokeLinejoin="round" strokeLinecap="round" />
        {hover != null && (
          <g>
            <line x1={x(hover)} x2={x(hover)} y1={pad.t} y2={pad.t + ih} stroke="var(--color-ink-3)" strokeWidth={1} />
            <circle cx={x(hover)} cy={y(data[hover])} r={4} fill="var(--color-accent)" stroke="var(--color-panel)" strokeWidth={2} />
          </g>
        )}
        <text x={pad.l} y={H - 4} fontSize={10} fill="var(--color-ink-3)">
          0 s
        </text>
        <text x={W - pad.r} y={H - 4} textAnchor="end" fontSize={10} fill="var(--color-ink-3)">
          {seconds} s
        </text>
      </svg>
    </div>
  );
}
