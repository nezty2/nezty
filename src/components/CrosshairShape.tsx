import type { CrosshairConfig } from "@/lib/types";

/**
 * Tekent de crosshair als SVG, gecentreerd in een vak van `box` pixels. Wordt gebruikt in de
 * overlay én in het voorbeeld, zodat je precies ziet wat je krijgt.
 */
export function CrosshairShape({ cfg, box = 200 }: { cfg: CrosshairConfig; box?: number }) {
  const c = box / 2;
  const t = Math.max(1, cfg.thickness);
  const len = Math.max(1, cfg.size);
  const gap = Math.max(0, cfg.gap);
  const ot = cfg.outline ? Math.max(1, cfg.outlineThickness) : 0;
  const rects: { x: number; y: number; w: number; h: number }[] = [];

  const arms = (withTop: boolean) => {
    // links, rechts, onder, (boven)
    rects.push({ x: c - gap - len, y: c - t / 2, w: len, h: t });
    rects.push({ x: c + gap, y: c - t / 2, w: len, h: t });
    rects.push({ x: c - t / 2, y: c + gap, w: t, h: len });
    if (withTop) rects.push({ x: c - t / 2, y: c - gap - len, w: t, h: len });
  };

  const dotSize = Math.max(t, 2);
  const showDot = cfg.shape === "dot" || cfg.shape === "cross_dot" || cfg.dot;
  if (cfg.shape === "cross" || cfg.shape === "cross_dot") arms(true);
  if (cfg.shape === "t") arms(false);

  const r = gap + len / 2;
  return (
    <svg width={box} height={box} viewBox={`0 0 ${box} ${box}`} style={{ opacity: cfg.opacity, display: "block" }} shapeRendering="crispEdges">
      {/* Rand eerst, dan de vorm erbovenop */}
      {cfg.outline &&
        rects.map((rc, i) => (
          <rect key={`o${i}`} x={rc.x - ot} y={rc.y - ot} width={rc.w + ot * 2} height={rc.h + ot * 2} fill={cfg.outlineColor} />
        ))}
      {cfg.outline && showDot && (
        <rect x={c - dotSize / 2 - ot} y={c - dotSize / 2 - ot} width={dotSize + ot * 2} height={dotSize + ot * 2} fill={cfg.outlineColor} />
      )}
      {cfg.shape === "circle" && (
        <>
          {cfg.outline && <circle cx={c} cy={c} r={r} fill="none" stroke={cfg.outlineColor} strokeWidth={t + ot * 2} shapeRendering="geometricPrecision" />}
          <circle cx={c} cy={c} r={r} fill="none" stroke={cfg.color} strokeWidth={t} shapeRendering="geometricPrecision" />
        </>
      )}
      {rects.map((rc, i) => (
        <rect key={i} x={rc.x} y={rc.y} width={rc.w} height={rc.h} fill={cfg.color} />
      ))}
      {showDot && <rect x={c - dotSize / 2} y={c - dotSize / 2} width={dotSize} height={dotSize} fill={cfg.color} />}
    </svg>
  );
}
