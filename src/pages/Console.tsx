import { Check, ExternalLink, Info, MapPin, RotateCcw } from "lucide-react";
import { useMemo, useState } from "react";
import { GainBadge } from "@/components/TweakCard";
import { Button, Card, PageHeader, SectionTitle, SegmentedControl, cn } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import type { Gain } from "@/lib/types";
import { useApp } from "@/store/app";

/**
 * Stappenplan voor consoles. nezty kan een console niet zelf aanpassen: dit is uitleg met menupaden,
 * eerlijke winstlabels en een vinkje per stap. Alles is terug te zetten in hetzelfde menu.
 */

type Platform = "ps5" | "ps4" | "xbox_series" | "xbox_one";
type Section = "display" | "performance" | "controller" | "network" | "background" | "maintenance" | "fortnite";

interface Step {
  id: string;
  section: Section;
  gain: Gain;
  platforms: Platform[];
  /** Knop naar een pagina in nezty die bij deze stap helpt. */
  link?: "network" | "controller";
}

const ALL: Platform[] = ["ps5", "ps4", "xbox_series", "xbox_one"];
const XBOX: Platform[] = ["xbox_series", "xbox_one"];
const PLATFORMS: Platform[] = ["ps5", "ps4", "xbox_series", "xbox_one"];
const SECTIONS: Section[] = ["display", "performance", "controller", "network", "background", "maintenance", "fortnite"];

const STEPS: Step[] = [
  // Beeld en tv
  { id: "tv_game_mode", section: "display", gain: "big", platforms: ALL },
  { id: "hz120", section: "display", gain: "big", platforms: ["ps5", "xbox_series"] },
  { id: "vrr", section: "display", gain: "measurable", platforms: ["ps5", "xbox_series", "xbox_one"] },
  { id: "hdmi", section: "display", gain: "measurable", platforms: ["ps5", "xbox_series"] },
  // Prestaties
  { id: "perf_mode", section: "performance", gain: "big", platforms: ["ps5"] },
  { id: "boost_mode", section: "performance", gain: "measurable", platforms: ["ps4"] },
  { id: "fps_boost", section: "performance", gain: "big", platforms: ["xbox_series"] },
  { id: "game_choice", section: "performance", gain: "big", platforms: ALL },
  // Controller en knoppen
  { id: "usb", section: "controller", gain: "small", platforms: ALL },
  { id: "triggers", section: "controller", gain: "measurable", platforms: ["ps5"] },
  { id: "vibration", section: "controller", gain: "small", platforms: ALL },
  { id: "controller_update", section: "controller", gain: "small", platforms: ["ps5", ...XBOX] },
  { id: "deadzone", section: "controller", gain: "measurable", platforms: ALL, link: "controller" },
  { id: "pro_controller", section: "controller", gain: "measurable", platforms: ["ps5", ...XBOX] },
  // Netwerk
  { id: "ethernet", section: "network", gain: "big", platforms: ALL },
  { id: "wifi", section: "network", gain: "measurable", platforms: ALL },
  { id: "nat", section: "network", gain: "measurable", platforms: ALL },
  { id: "network_test", section: "network", gain: "measurable", platforms: ALL, link: "network" },
  { id: "dns", section: "network", gain: "negligible", platforms: ALL },
  { id: "vpn", section: "network", gain: "negligible", platforms: ALL },
  // Downloads en updates
  { id: "downloads", section: "background", gain: "measurable", platforms: ALL },
  { id: "rest_updates", section: "background", gain: "small", platforms: ALL },
  // Onderhoud
  { id: "rebuild", section: "maintenance", gain: "small", platforms: ["ps5", "ps4"] },
  { id: "restart", section: "maintenance", gain: "small", platforms: XBOX },
  // Fortnite
  { id: "fn_120", section: "fortnite", gain: "big", platforms: ["ps5", "xbox_series"] },
  { id: "fn_controller", section: "fortnite", gain: "measurable", platforms: ALL },
];

const STORE_KEY = "nzty.console.v1";

interface Saved {
  platform: Platform;
  done: string[];
}

function loadSaved(): Saved {
  try {
    const v = JSON.parse(localStorage.getItem(STORE_KEY) ?? "null") as Partial<Saved> | null;
    const platform = v?.platform && PLATFORMS.includes(v.platform) ? v.platform : "ps5";
    return { platform, done: Array.isArray(v?.done) ? v.done.filter((x) => typeof x === "string") : [] };
  } catch {
    return { platform: "ps5", done: [] };
  }
}

function persist(s: Saved) {
  try {
    localStorage.setItem(STORE_KEY, JSON.stringify(s));
  } catch {
    /* Zonder opslag werkt de pagina gewoon, alleen de vinkjes blijven niet bewaard. */
  }
}

export function ConsolePage() {
  const { t, td } = useT();
  const setPage = useApp((s) => s.setPage);
  const [saved, setSaved] = useState<Saved>(loadSaved);
  const platform = saved.platform;

  const steps = useMemo(() => STEPS.filter((s) => s.platforms.includes(platform)), [platform]);
  const key = (id: string) => `${platform}:${id}`;
  const doneCount = steps.filter((s) => saved.done.includes(key(s.id))).length;

  const update = (next: Saved) => {
    setSaved(next);
    persist(next);
  };
  const toggle = (id: string) => {
    const k = key(id);
    update({ ...saved, done: saved.done.includes(k) ? saved.done.filter((d) => d !== k) : [...saved.done, k] });
  };
  const resetChecks = () => update({ ...saved, done: saved.done.filter((d) => !d.startsWith(`${platform}:`)) });

  const where = (id: string) => td(`console.steps.${id}.where.${platform}`, undefined, "") || td(`console.steps.${id}.where.all`, undefined, "");

  return (
    <div className="space-y-8">
      <PageHeader title={t("console.title")} subtitle={t("console.subtitle")} />

      <div className="flex items-start gap-3 rounded-lg border border-line bg-panel px-4 py-3 text-[13px] text-ink-2">
        <Info className="mt-0.5 h-4 w-4 shrink-0 text-info" />
        {t("console.honest")}
      </div>

      <Card>
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <div className="text-[13px] text-ink-3">{t("console.platform")}</div>
            <div className="mt-2">
              <SegmentedControl<Platform>
                value={platform}
                onChange={(p) => update({ ...saved, platform: p })}
                options={PLATFORMS.map((p) => ({ value: p, label: t(`console.platforms.${p}`) }))}
              />
            </div>
          </div>
          <div className="min-w-[220px] flex-1 sm:max-w-xs">
            <div className="flex items-center justify-between text-[13px]">
              <span className="text-ink">{t("console.progress", { done: doneCount, total: steps.length })}</span>
              {doneCount > 0 && (
                <button onClick={resetChecks} className="flex items-center gap-1 text-ink-3 hover:text-ink">
                  <RotateCcw className="h-3.5 w-3.5" />
                  {t("console.resetChecks")}
                </button>
              )}
            </div>
            <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-panel-2">
              <div className="h-full rounded-full bg-accent transition-all" style={{ width: `${steps.length ? (doneCount / steps.length) * 100 : 0}%` }} />
            </div>
          </div>
        </div>
        <p className="mt-3 text-[12px] text-ink-3">{t("console.namesNote")}</p>
      </Card>

      {SECTIONS.map((section) => {
        const list = steps.filter((s) => s.section === section);
        if (list.length === 0) return null;
        return (
          <section key={section}>
            <SectionTitle>{t(`console.sections.${section}`)}</SectionTitle>
            <div className="grid gap-4 lg:grid-cols-2">
              {list.map((s) => {
                const done = saved.done.includes(key(s.id));
                return (
                  <Card key={s.id} className={cn("flex gap-4", done && "opacity-75")}>
                    <button
                      onClick={() => toggle(s.id)}
                      aria-pressed={done}
                      aria-label={t("console.markDone")}
                      title={t("console.markDone")}
                      className={cn(
                        "mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-md border transition-colors",
                        done ? "border-ok bg-ok text-[#0f1115]" : "border-line-2 hover:border-accent",
                      )}
                    >
                      {done && <Check className="h-4 w-4" strokeWidth={3} />}
                    </button>
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <h3 className={cn("text-[15px] font-semibold", done && "line-through decoration-ink-3")}>{td(`console.steps.${s.id}.title`)}</h3>
                        <GainBadge gain={s.gain} />
                      </div>
                      <dl className="mt-3 space-y-2 text-[13px] leading-relaxed">
                        <div>
                          <dt className="text-ink-3">{t("tweakCard.what")}</dt>
                          <dd className="text-ink">{td(`console.steps.${s.id}.what`)}</dd>
                        </div>
                        <div>
                          <dt className="text-ink-3">{t("tweakCard.effect")}</dt>
                          <dd className="text-ink">{td(`console.steps.${s.id}.effect`)}</dd>
                        </div>
                      </dl>
                      <div className="mt-3 flex items-start gap-2 rounded-lg bg-panel-2 px-3 py-2.5 text-[12.5px] leading-relaxed text-ink-2">
                        <MapPin className="mt-0.5 h-3.5 w-3.5 shrink-0 text-accent" />
                        <span>
                          <span className="text-ink-3">{t("console.where")} </span>
                          {where(s.id)}
                        </span>
                      </div>
                      {s.link && (
                        <Button size="sm" className="mt-3" icon={ExternalLink} onClick={() => setPage(s.link!)}>
                          {s.link === "network" ? t("console.openNetwork") : t("console.openController")}
                        </Button>
                      )}
                    </div>
                  </Card>
                );
              })}
            </div>
          </section>
        );
      })}

      <section>
        <SectionTitle>{t("console.reversalTitle")}</SectionTitle>
        <Card>
          <p className="text-[13px] leading-relaxed text-ink">{t("console.reversal")}</p>
        </Card>
      </section>
    </div>
  );
}
