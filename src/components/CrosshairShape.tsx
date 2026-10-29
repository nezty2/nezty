import type { CrosshairConfig } from "@/lib/types";

/**
 * Tekent de crosshair als SVG, gecentreerd in een vak van `box` pixels. Wordt gebruikt in de
 * overlay én in het voorbeeld, zodat je precies ziet wat je krijgt. `scale` schaalt alles mee
 * (voor de kleine presetvoorbeelden), de verschuiving blijft dan in dezelfde verhouding.
 */
export function CrosshairShape({ cfg, box = 200, scale = 1 }: { cfg: CrosshairConfig; box?: number; scale?: number }) {
  const c = box / 2;
  const s = scale;
  const t = Math.max(1, cfg.thickness) * s;
  const len = Math.max(1, cfg.size) * s;
  const gap = Math.max(0, cfg.gap) * s;
  const ot = cfg.outline ? Math.max(1, cfg.outlineThickness) * s : 0;
  const dotSize = Math.max(1, cfg.dotSize) * s;
  const rotation = (cfg.rotation || 0) + (cfg.shape === "x" ? 45 : 0);
  const rotated = rotation % 90 !== 0;
  const dx = (cfg.offsetX || 0) * s;
  const dy = (cfg.offsetY || 0) * s;

  const hasArms = cfg.shape === "cross" || cfg.shape === "cross_dot" || cfg.shape === "t" || cfg.shape === "x";
  const showTop = cfg.shape === "t" ? false : cfg.showTop;
  const rects: { x: number; y: number; w: number; h: number }[] = [];
  if (hasArms) {
    if (cfg.showLeft) rects.push({ x: c - gap - len, y: c - t / 2, w: len, h: t });
    if (cfg.showRight) rects.push({ x: c + gap, y: c - t / 2, w: len, h: t });
    if (cfg.showBottom) rects.push({ x: c - t / 2, y: c + gap, w: t, h: len });
    if (showTop) rects.push({ x: c - t / 2, y: c - gap - len, w: t, h: len });
  }

  const showDot = cfg.shape === "dot" || cfg.shape === "cross_dot" || cfg.shape === "circle_dot" || cfg.dot;
  const ring = cfg.shape === "circle" || cfg.shape === "circle_dot";
  const square = cfg.shape === "square";
  const r = gap + len / 2;

  return (
    <svg
      width={box}
      height={box}
      viewBox={`0 0 ${box} ${box}`}
      style={{ opacity: cfg.opacity, display: "block", overflow: "visible" }}
      shapeRendering={rotated ? "geometricPrecision" : "crispEdges"}
    >
      <g transform={`translate(${dx} ${dy}) rotate(${rotation} ${c} ${c})`}>
        {/* Rand eerst, dan de vorm erbovenop */}
        {cfg.outline && rects.map((rc, i) => <rect key={`o${i}`} x={rc.x - ot} y={rc.y - ot} width={rc.w + ot * 2} height={rc.h + ot * 2} fill={cfg.outlineColor} />)}
        {cfg.outline && showDot && <rect x={c - dotSize / 2 - ot} y={c - dotSize / 2 - ot} width={dotSize + ot * 2} height={dotSize + ot * 2} fill={cfg.outlineColor} />}
        {ring && (
          <>
            {cfg.outline && <circle cx={c} cy={c} r={r} fill="none" stroke={cfg.outlineColor} strokeWidth={t + ot * 2} shapeRendering="geometricPrecision" />}
            <circle cx={c} cy={c} r={r} fill="none" stroke={cfg.color} strokeWidth={t} shapeRendering="geometricPrecision" />
          </>
        )}
        {square && (
          <>
            {cfg.outline && <rect x={c - r} y={c - r} width={r * 2} height={r * 2} fill="none" stroke={cfg.outlineColor} strokeWidth={t + ot * 2} />}
            <rect x={c - r} y={c - r} width={r * 2} height={r * 2} fill="none" stroke={cfg.color} strokeWidth={t} />
          </>
        )}
        {rects.map((rc, i) => (
          <rect key={i} x={rc.x} y={rc.y} width={rc.w} height={rc.h} fill={cfg.color} />
        ))}
        {showDot && <rect x={c - dotSize / 2} y={c - dotSize / 2} width={dotSize} height={dotSize} fill={cfg.color} />}
      </g>
    </svg>
  );
}
