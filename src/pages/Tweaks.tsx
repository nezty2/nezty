import { AnimatePresence, motion } from "framer-motion";
import { ChevronDown, RefreshCw, SlidersHorizontal, Trash2 } from "lucide-react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { ACTIVATE_ALL_TWEAKS, ActivateCovers, ActivateHero } from "@/components/ActivateAll";
import { BigWinCard } from "@/components/BigWinCard";
import { CleanupModal } from "@/components/CleanupModal";
import { FortniteSection } from "@/components/FortniteSection";
import { Loader } from "@/components/Loader";
import { OverlaysCard } from "@/components/OverlaysCard";
import { PerGame } from "@/components/PerGame";
import { StatusBar } from "@/components/StatusBar";
import { TweakCard } from "@/components/TweakCard";
import { Button, PageHeader, SectionTitle, cn } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import type { TweakInfo } from "@/lib/types";
import { useApp } from "@/store/app";

const MORE_KEY = "nezty.tweaks.more";
/** Oude sleutel van de tabbladen (tot 0.4.0): wordt opgeruimd. */
const OLD_TAB_KEY = "nezty.tweaks.tab";
/** Controles die nezty niet zelf kan oplossen. De andere zitten in "Alles activeren" of staan op de laptoppagina. */
const MANUAL_CHECKS = ["ram_speed", "gpu_driver", "heavy_apps"];

/** Waar een link naartoe moet. Oude links met een tabblad (van vóór 0.5.0) komen op de juiste plek uit. */
function targetOf(params: Record<string, string>): "more" | "games" | null {
  const s = params.section ?? params.tab;
  if (s === "games" || s === "fortnite" || params.game) return "games";
  if (s === "more" || s === "windows" || s === "experimental") return "more";
  return null;
}

function readMore(): boolean {
  try {
    localStorage.removeItem(OLD_TAB_KEY);
    return localStorage.getItem(MORE_KEY) === "1";
  } catch {
    return false;
  }
}

function Group({ id, title, sub, children }: { id?: string; title: string; sub?: string; children: ReactNode }) {
  return (
    <section id={id} className="scroll-mt-6">
      <SectionTitle sub={sub}>{title}</SectionTitle>
      {children}
    </section>
  );
}

export function TweaksPage() {
  const { t, td } = useT();
  const tweaks = useApp((s) => s.tweaks);
  const loading = useApp((s) => s.tweaksLoading);
  const bigWins = useApp((s) => s.bigWins);
  const device = useApp((s) => s.device);
  const refreshTweaks = useApp((s) => s.refreshTweaks);
  const refreshBigWins = useApp((s) => s.refreshBigWins);
  const setTweak = useApp((s) => s.setTweak);
  const appInfo = useApp((s) => s.appInfo);
  const pageParams = useApp((s) => s.pageParams);
  const toast = useApp((s) => s.toast);
  const reduceMotion = useApp((s) => s.settings?.reduceMotion ?? false);
  const [busy, setBusy] = useState<string | null>(null);
  const [cleanupOpen, setCleanupOpen] = useState(false);
  const [more, setMore] = useState(() => readMore() || targetOf(pageParams) !== null);
  const moreRef = useRef<HTMLDivElement>(null);
  const elevated = appInfo?.isElevated ?? false;

  useEffect(() => {
    try {
      localStorage.setItem(MORE_KEY, more ? "1" : "0");
    } catch {
      /* geen opslag beschikbaar */
    }
  }, [more]);

  // Vanaf het dashboard of een andere pagina: "Meer opties" openen en naar het gevraagde deel scrollen.
  useEffect(() => {
    const target = targetOf(pageParams);
    if (!target) return;
    setMore(true);
    const id = window.setTimeout(() => {
      const el = target === "games" ? document.getElementById("tweaks-games") : moreRef.current;
      el?.scrollIntoView({ behavior: reduceMotion ? "auto" : "smooth", block: "start" });
    }, 260);
    return () => window.clearTimeout(id);
    // Alleen bij een nieuwe link, niet bij elke andere wijziging.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pageParams]);

  const onToggle = async (tw: TweakInfo, enabled: boolean, option: string | null) => {
    setBusy(tw.id);
    const info = await setTweak(tw.id, enabled, option);
    setBusy(null);
    if (info) {
      const name = td(`tweak.${tw.id}.name`);
      const msg = !enabled ? t("tweakCard.disabled", { name }) : info.active === "after_pc_restart" ? t("tweakCard.appliedRestart", { name }) : t("tweakCard.applied", { name });
      toast({ level: "success", title: t("notice.success"), message: msg });
    }
  };

  // Losse tweaks: alles wat niet al in "Alles activeren" zit. Wat niet bij dit apparaat past, staat er niet tussen,
  // behalve als nezty het eerder zelf heeft aangezet (dan moet het wel uit kunnen). Netwerktweaks staan op de
  // netwerkpagina, bij de functies waar ze bij horen.
  const shown = (tw: TweakInfo) => !tw.platformLocked || (tw.state === "on" && tw.appliedByApp);
  const optional = tweaks.filter((tw) => tw.scope === "global" && tw.kind === "toggle" && tw.category !== "network" && !ACTIVATE_ALL_TWEAKS.includes(tw.id) && shown(tw));
  const windows = optional.filter((tw) => tw.category !== "laptop");
  const laptop = optional.filter((tw) => tw.category === "laptop");
  const cleanup = tweaks.filter((tw) => tw.kind === "action");
  const checks = (bigWins ?? []).filter((w) => MANUAL_CHECKS.includes(w.id));
  const isLaptop = device?.effective === "laptop";

  const grid = (list: TweakInfo[]) => (
    <div className="grid gap-4 lg:grid-cols-2 2xl:grid-cols-3">
      {list.map((tw) => (
        <TweakCard
          key={tw.id}
          tweak={tw}
          elevated={elevated}
          busy={busy === tw.id}
          onToggle={(v, opt) => void onToggle(tw, v, opt)}
          action={
            tw.kind === "action" ? (
              <Button size="sm" icon={Trash2} onClick={() => setCleanupOpen(true)}>
                {t("tweaks.cleanupOpen")}
              </Button>
            ) : undefined
          }
        />
      ))}
    </div>
  );

  return (
    <div className="space-y-6">
      <PageHeader
        title={t("tweaks.title")}
        subtitle={t("tweaks.subtitle")}
        actions={
          <Button icon={RefreshCw} onClick={() => void Promise.all([refreshTweaks(), refreshBigWins()])} loading={loading}>
            {t("common.refresh")}
          </Button>
        }
      />

      <StatusBar />
      <ActivateHero />
      <ActivateCovers />

      {/* Meer opties: standaard dicht, zodat het scherm rustig blijft. */}
      <div ref={moreRef} className="scroll-mt-6">
        <button
          onClick={() => setMore((v) => !v)}
          aria-expanded={more}
          className="card flex w-full items-center gap-3 px-5 py-4 text-left transition-colors hover:border-line-2"
        >
          <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-panel-2 text-ink-2">
            <SlidersHorizontal className="h-4 w-4" />
          </span>
          <span className="min-w-0 flex-1">
            <span className="block text-[15px] font-semibold">{t("tweaks.more.title")}</span>
            <span className="block text-[13px] text-ink-3">{t("tweaks.more.sub")}</span>
          </span>
          <ChevronDown className={cn("h-5 w-5 shrink-0 text-ink-3 transition-transform duration-200", more && "rotate-180")} />
        </button>

        <AnimatePresence initial={false}>
          {more && (
            <motion.div
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: "auto", opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              transition={{ duration: 0.28, ease: [0.22, 1, 0.36, 1] }}
              className="overflow-hidden"
            >
              <div className="space-y-8 pt-6">
                {loading && tweaks.length === 0 ? (
                  <Loader label={t("tweaks.loading")} />
                ) : (
                  <>
                    {windows.length > 0 && (
                      <Group title={t("tweaks.more.windows")} sub={t("tweaks.more.windowsSub")}>
                        {grid(windows)}
                      </Group>
                    )}
                    {isLaptop && laptop.length > 0 && (
                      <Group title={t("tweaks.more.laptop")} sub={t("tweaks.more.laptopSub")}>
                        {grid(laptop)}
                      </Group>
                    )}
                  </>
                )}

                <Group id="tweaks-games" title={t("tweaks.more.games")} sub={t("tweaks.more.gamesSub")}>
                  <div className="space-y-6">
                    <FortniteSection />
                    <OverlaysCard />
                    <PerGame />
                  </div>
                </Group>

                {checks.length > 0 && (
                  <Group title={t("tweaks.more.checks")} sub={t("tweaks.more.checksSub")}>
                    <div className="grid gap-4 lg:grid-cols-2 2xl:grid-cols-3">
                      {checks.map((w) => (
                        <BigWinCard key={w.id} win={w} onFixed={(list) => useApp.setState({ bigWins: list })} />
                      ))}
                    </div>
                  </Group>
                )}

                {cleanup.length > 0 && <Group title={t("tweaks.more.cleanup")}>{grid(cleanup)}</Group>}
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      <CleanupModal open={cleanupOpen} onClose={() => setCleanupOpen(false)} />
    </div>
  );
}
