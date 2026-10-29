import { useEffect, useRef, useState } from "react";
import { useApp } from "@/store/app";
import { TIMES, TIMES_REDUCED } from "./timeline";

const NAME = "nezty".split("");
/** Aantal lagen achter elke letter: samen geven ze de letter echte dikte als de naam draait. */
const DEPTH = 16;
const LAYER_GAP = 4;

const clamp = (v: number, a = 0, b = 1) => Math.min(b, Math.max(a, v));
const smooth = (a: number, b: number, t: number) => {
  const x = clamp((t - a) / (b - a));
  return x * x * (3 - 2 * x);
};
const easeOutBack = (x: number) => {
  const c1 = 1.25;
  const c3 = c1 + 1;
  return 1 + c3 * Math.pow(x - 1, 3) + c1 * Math.pow(x - 1, 2);
};

/**
 * Opening van nezty: een teller, dan klappen de letters van de naam in 3D naar voren, met echte dikte, een lichtstreep
 * in je accentkleur en een zachte zweving die met je muis meebeweegt. Duurt ongeveer 5 seconden, is altijd over te slaan
 * en blokkeert de app nooit: die laadt ondertussen op de achtergrond.
 */
export function Intro({ onDone }: { onDone: () => void }) {
  const [nameShown, setNameShown] = useState(false);
  const [skipVisible, setSkipVisible] = useState(false);

  const overlayRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const counterRef = useRef<HTMLDivElement>(null);
  const counterBoxRef = useRef<HTMLDivElement>(null);
  const barRef = useRef<HTMLDivElement>(null);
  const wordRef = useRef<HTMLDivElement>(null);
  const lineRef = useRef<HTMLDivElement>(null);
  const flareRef = useRef<HTMLDivElement>(null);
  const taglineRef = useRef<HTMLDivElement>(null);
  const letterRefs = useRef<(HTMLSpanElement | null)[]>([]);
  const skipRef = useRef<() => void>(() => undefined);

  useEffect(() => {
    let raf = 0;
    let last = 0;
    let elapsed = 0;
    let skipAt: number | null = null;
    let finished = false;
    let named = false;
    let skipShown = false;
    let wallStart = 0;
    const mouse = { x: 0, y: 0 };
    const smoothMouse = { x: 0, y: 0 };

    // Alleen nezty's eigen schakelaar "Minder beweging" telt. De Windows-instelling "Animaties" staat bij veel gamers uit
    // (ook door tweaks) en zegt niets over wat je in nezty wilt zien. ?intro=full forceert alles (alleen tijdens ontwikkelen).
    const force = import.meta.env.DEV ? new URLSearchParams(location.search).get("intro") : null;
    const reduce = () => (force ? false : !!useApp.getState().settings?.reduceMotion);
    let reduced = reduce();
    let times: typeof TIMES = reduced ? TIMES_REDUCED : TIMES;

    const finish = () => {
      if (finished) return;
      finished = true;
      cancelAnimationFrame(raf);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("keydown", onKey);
      onDone();
    };
    const requestSkip = () => {
      if (skipAt === null) skipAt = elapsed;
    };
    skipRef.current = requestSkip;
    const onMove = (e: PointerEvent) => {
      mouse.x = (e.clientX / window.innerWidth) * 2 - 1;
      mouse.y = -((e.clientY / window.innerHeight) * 2 - 1);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" || e.key === "Enter" || e.key === " ") requestSkip();
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("keydown", onKey);

    const loop = (now: number) => {
      raf = requestAnimationFrame(loop);
      if (finished) return;
      // Alleen echte, zichtbare tijd telt mee: een venster op de achtergrond pauzeert de opening.
      const dt = Math.min(0.1, (now - last) / 1000);
      last = now;
      elapsed += dt;
      const tm = elapsed;
      // Vangnet: op een heel trage pc duurt de opening nooit langer dan ongeveer 9 seconden echte tijd.
      if (!wallStart) wallStart = now;
      if (now - wallStart > 9000) requestSkip();

      // Settings kunnen later laden dan de opening start: staat "Minder beweging" aan, dan meteen afronden.
      if (!reduced && reduce()) {
        reduced = true;
        requestSkip();
      }

      // Teller en lichtstreep
      const load = smooth(0, times.loadEnd, tm);
      if (counterRef.current) counterRef.current.textContent = String(Math.round(load * 100)).padStart(3, "0");
      if (counterBoxRef.current) counterBoxRef.current.style.opacity = String(1 - smooth(times.loadEnd, times.loadEnd + 0.5, tm));
      if (barRef.current) barRef.current.style.transform = `scaleX(${load})`;
      if (lineRef.current) {
        const grow = smooth(times.loadEnd - 0.5, times.nameAt + 0.5, tm);
        lineRef.current.style.transform = `scaleX(${grow})`;
        lineRef.current.style.opacity = String(grow * (1 - smooth(times.nameAt + 0.6, times.nameAt + 1.6, tm)));
      }
      if (flareRef.current) flareRef.current.style.opacity = String(0.4 + 0.6 * smooth(times.nameAt - 0.2, times.nameAt + 1.2, tm));

      // De letters: elk klapt vanuit de diepte naar voren.
      const instant = times === TIMES_REDUCED;
      NAME.forEach((_, i) => {
        const el = letterRefs.current[i];
        if (!el) return;
        const p = instant ? 1 : clamp((tm - times.nameAt - i * times.letterStagger) / times.letterDuration);
        const e = easeOutBack(p);
        const o = smooth(0, 0.45, p);
        el.style.transform = `translate3d(0, ${(1 - e) * 46}px, ${(1 - e) * -520}px) rotateX(${(1 - e) * -105}deg) rotateY(${(1 - e) * 38}deg)`;
        for (let c = 0; c < el.children.length; c++) {
          const child = el.children[c] as HTMLElement;
          child.style.opacity = String(c === el.children.length - 1 ? o : o * 0.9);
        }
        (el.lastElementChild as HTMLElement).style.filter = p < 1 ? `blur(${(1 - smooth(0, 0.8, p)) * 14}px)` : "none";
      });

      // De hele naam zweeft rustig en volgt je muis.
      smoothMouse.x += (mouse.x - smoothMouse.x) * 0.06;
      smoothMouse.y += (mouse.y - smoothMouse.y) * 0.06;
      if (wordRef.current) {
        const settle = instant ? 0 : smooth(times.nameAt + 0.8, times.nameAt + 2.2, tm);
        const ry = Math.sin(tm * 1.1) * 18 * settle + smoothMouse.x * 16;
        const rx = Math.sin(tm * 0.8) * 3.5 * settle - smoothMouse.y * 9;
        wordRef.current.style.transform = `rotateX(${rx}deg) rotateY(${ry}deg)`;
      }
      if (taglineRef.current) {
        const k = smooth(times.tagline, times.tagline + 0.7, tm);
        taglineRef.current.style.opacity = String(k);
        taglineRef.current.style.transform = `translateY(${(1 - k) * 12}px)`;
      }

      if (!named && tm >= times.nameAt) {
        named = true;
        setNameShown(true);
      }
      if (!skipShown && tm > 0.5) {
        skipShown = true;
        setSkipVisible(true);
      }

      // Het eind: de naam zoomt door het scherm heen en vervaagt naar de app.
      const fade = skipAt !== null ? clamp((tm - skipAt) / 0.3) : smooth(times.fadeStart, times.total, tm);
      if (overlayRef.current) overlayRef.current.style.opacity = String(1 - fade);
      if (contentRef.current) contentRef.current.style.transform = `scale(${1 + 0.12 * fade})`;
      if (fade >= 1) finish();
    };
    raf = requestAnimationFrame((n) => {
      last = n;
      loop(n);
    });

    return () => {
      finished = true;
      cancelAnimationFrame(raf);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("keydown", onKey);
    };
    // De opening start één keer.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div ref={overlayRef} className="fixed inset-0 z-[100] overflow-hidden bg-bg" onPointerDown={() => skipRef.current()} role="status" aria-label="Loading nezty">
      <div ref={contentRef} className="absolute inset-0 will-change-transform">
        {/* Gloed in je accentkleur en fijne korrel */}
        <div
          ref={flareRef}
          className="absolute inset-0"
          style={{ background: "radial-gradient(55% 50% at 50% 46%, color-mix(in srgb, var(--accent) 30%, transparent), transparent 72%), radial-gradient(40% 40% at 85% 90%, color-mix(in srgb, var(--accent) 12%, transparent), transparent 70%)" }}
        />
        <div
          className="absolute inset-0 opacity-[0.07] mix-blend-overlay"
          style={{ backgroundImage: "url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='160' height='160'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='.9' numOctaves='2' stitchTiles='stitch'/></filter><rect width='100%' height='100%' filter='url(%23n)'/></svg>\")" }}
        />

        {/* Lichtstreep waar de naam uit verschijnt */}
        <div
          ref={lineRef}
          className="absolute left-[8%] right-[8%] top-1/2 h-px origin-center opacity-0"
          style={{ background: "linear-gradient(90deg, transparent, var(--accent), transparent)", boxShadow: "0 0 24px 2px var(--accent)" }}
          aria-hidden
        />

        {/* De naam in 3D */}
        <div className="absolute inset-0 flex flex-col items-center justify-center" style={{ perspective: "900px" }} aria-hidden>
          <div ref={wordRef} className="flex" style={{ transformStyle: "preserve-3d" }}>
            {NAME.map((ch, i) => (
              <span
                key={i}
                ref={(el) => void (letterRefs.current[i] = el)}
                className="relative inline-block text-[clamp(64px,13vw,170px)] font-semibold leading-none tracking-[0.03em]"
                style={{ transformStyle: "preserve-3d" }}
              >
                {Array.from({ length: DEPTH }, (_, d) => (
                  <span
                    key={d}
                    className="absolute inset-0 select-none opacity-0"
                    style={{
                      transform: `translateZ(${-(d + 1) * LAYER_GAP}px)`,
                      color: `color-mix(in srgb, var(--accent) ${Math.max(6, 56 - d * 3.4)}%, #05060a)`,
                    }}
                  >
                    {ch}
                  </span>
                ))}
                <span className={`relative inline-block text-ink opacity-0 ${nameShown ? "intro-letter" : ""}`} style={{ animationDelay: `${0.7 + i * 0.1}s` }}>
                  {ch}
                </span>
              </span>
            ))}
          </div>
          <div ref={taglineRef} className="mt-7 text-sm tracking-wide text-ink-2 opacity-0">
            Honest game tweaks, always reversible
          </div>
        </div>

        {/* Teller */}
        <div ref={counterBoxRef} className="absolute bottom-[9%] left-1/2 flex -translate-x-1/2 flex-col items-center gap-3" aria-hidden>
          <div className="flex items-baseline gap-1 text-ink">
            <div ref={counterRef} className="w-[3ch] text-right font-mono text-[34px] font-light tnum tracking-tight">
              000
            </div>
            <span className="text-sm text-ink-3">%</span>
          </div>
          <div className="h-[2px] w-40 overflow-hidden rounded-full bg-line">
            <div ref={barRef} className="h-full origin-left rounded-full bg-accent" style={{ transform: "scaleX(0)" }} />
          </div>
        </div>
      </div>

      <button
        onPointerDown={(e) => e.stopPropagation()}
        onClick={() => skipRef.current()}
        className={`absolute bottom-6 right-6 rounded-lg border border-line bg-panel/60 px-3 py-1.5 text-[13px] text-ink-2 backdrop-blur transition-opacity duration-500 hover:text-ink ${skipVisible ? "opacity-100" : "opacity-0"}`}
      >
        Skip
      </button>
    </div>
  );
}
