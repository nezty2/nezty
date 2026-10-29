import { AlertCircle, BellOff, CheckCircle2, ChevronDown, ExternalLink, FlaskConical, Gamepad2, HelpCircle, Info, MinusCircle, MonitorCog, Pickaxe, RefreshCw, Sparkles, Trash2, Wrench, type LucideIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { BestTweaks } from "@/components/BestTweaks";
import { CleanupModal } from "@/components/CleanupModal";
import { FortniteSection } from "@/components/FortniteSection";
import { PerGame } from "@/components/PerGame";
import { GainBadge, TweakCard } from "@/components/TweakCard";
import { Button, Card, PageHeader, SectionTitle, Skeleton, cn } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import type { TKey } from "@/i18n";
import { api } from "@/lib/ipc";
import { formatDate, formatMb } from "@/lib/format";
import type { BigWin, TweakInfo } from "@/lib/types";
import { useApp } from "@/store/app";

const statusIcon = { ok: CheckCircle2, issue: AlertCircle, info: HelpCircle, unavailable: MinusCircle };
const statusColor = { ok: "text-ok", issue: "text-accent", info: "text-warn", unavailable: "text-ink-3" };

export type TweaksTab = "recommended" | "fortnite" | "games" | "windows" | "experimental";
const TABS: { id: TweaksTab; icon: LucideIcon }[] = [
  { id: "recommended", icon: Sparkles },
  { id: "fortnite", icon: Pickaxe },
  { id: "games", icon: Gamepad2 },
  { id: "windows", icon: MonitorCog },
  { id: "experimental", icon: FlaskConical },
];
const TAB_KEY = "nezty.tweaks.tab";
/** Controles die alleen op de laptoppagina staan. */
export const LAPTOP_CHECKS = ["laptop_heat", "laptop_gpu_mode", "laptop_wifi", "laptop_display", "vendor_apps"];

export function BigWinCard({ win, onFixed }: { win: BigWin; onFixed: (list: BigWin[]) => void }) {
  const { t, td, tl, lang } = useT();
  const setPage = useApp((s) => s.setPage);
  const handleError = useApp((s) => s.handleError);
  const toast = useApp((s) => s.toast);
  const [busy, setBusy] = useState(false);
  const [howTo, setHowTo] = useState(false);
  const Icon = statusIcon[win.status];
  const base = `bigwin.${win.id}`;
  const params: Record<string, string> = { ...win.params };
  if (params.mb) params.size = formatMb(lang, Number(params.mb));
  if (params.date) params.date = formatDate(lang, params.date);
  if (!params.app) params.app = t("bigwin.brandApp");

  const fix = async () => {
    setBusy(true);
    try {
      const list = await api.fixBigWin(win.id);
      onFixed(list);
      toast({ level: "success", title: t("notice.success"), message: t("bigwin.fixed") });
    } catch (e) {
      handleError(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className={cn("card flex flex-col p-5", win.status === "issue" && "border-accent/50")}>
      <div className="flex items-start gap-3">
        <Icon className={cn("mt-0.5 h-5 w-5 shrink-0", statusColor[win.status])} />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="text-[15px] font-semibold">{td(`${base}.title`)}</h3>
            {win.status === "issue" && <GainBadge gain={win.gain} />}
          </div>
          <p className="mt-1.5 text-[13px] leading-relaxed text-ink-2">{td(`${base}.${win.variant}`, params)}</p>
        </div>
      </div>

      {win.id === "ram_speed" && (win.status === "issue" || win.status === "info") && (
        <div className="mt-3 pl-8">
          <button onClick={() => setHowTo((v) => !v)} className="flex items-center gap-1 text-[12px] font-medium text-ink-3 hover:text-ink-2">
            <ChevronDown className={cn("h-3.5 w-3.5 transition-transform", howTo && "rotate-180")} />
            {t("bigwin.ram_speed.howTo")}
          </button>
          {howTo && (
            <ol className="card-inset mt-2 list-decimal space-y-1 py-2.5 pl-8 pr-3 text-[13px] text-ink-2">
              {tl("bigwin.ram_speed.steps").map((s, i) => (
                <li key={i}>{s}</li>
              ))}
            </ol>
          )}
        </div>
      )}

      <div className="mt-4 flex flex-wrap items-center gap-2 pl-8">
        {win.action === "fix" && win.status === "issue" && (
          <>
            <Button size="sm" variant="primary" icon={Wrench} loading={busy} onClick={() => void fix()}>
              {td(`${base}.fix`, params)}
            </Button>
            {win.id === "refresh_rate" && <span className="text-[12px] text-ink-3">{t("bigwin.refresh_rate.fixNote")}</span>}
          </>
        )}
        {win.action === "link" && win.link && (win.status !== "ok" || win.id === "gpu_driver") && (
          <Button size="sm" variant={win.status === "issue" ? "primary" : "secondary"} icon={ExternalLink} onClick={() => api.openUrl(win.link!).catch(handleError)}>
            {td(`${base}.link`)}
          </Button>
        )}
        {win.action === "open_page" && win.status !== "ok" && (
          <Button size="sm" onClick={() => setPage("background")}>
            {td(`${base}.open`, undefined, t("bigwin.heavy_apps.open"))}
          </Button>
        )}
        <span className={cn("ml-auto text-[12px]", statusColor[win.status])}>{t(`bigwin.status.${win.status}`)}</span>
      </div>
    </div>
  );
}

function BigWins() {
  const { t } = useT();
  const bigWins = useApp((s) => s.bigWins);
  const order = { issue: 0, info: 1, ok: 2, unavailable: 3 };
  const sorted = [...(bigWins ?? [])].filter((w) => !LAPTOP_CHECKS.includes(w.id)).sort((a, b) => order[a.status] - order[b.status]);
  return (
    <section>
      <SectionTitle sub={t("tweaks.bigWinsSub")}>{t("tweaks.bigWins")}</SectionTitle>
      {bigWins === null ? (
        <div className="grid gap-4 lg:grid-cols-2 2xl:grid-cols-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <Skeleton key={i} className="h-36" />
          ))}
        </div>
      ) : (
        <div className="grid gap-4 lg:grid-cols-2 2xl:grid-cols-3">
          {sorted.map((w) => (
            <BigWinCard key={w.id} win={w} onFixed={(list) => useApp.setState({ bigWins: list })} />
          ))}
        </div>
      )}
    </section>
  );
}

function TweakGrid({ list, busy, onToggle, elevated, onCleanup }: { list: TweakInfo[]; busy: string | null; onToggle: (tw: TweakInfo, v: boolean, opt: string | null) => void; elevated: boolean; onCleanup: () => void }) {
  const { t } = useT();
  return (
    <div className="grid gap-4 lg:grid-cols-2 2xl:grid-cols-3">
      {list.map((tw) => (
        <TweakCard
          key={tw.id}
          tweak={tw}
          elevated={elevated}
          busy={busy === tw.id}
          onToggle={(v, opt) => onToggle(tw, v, opt)}
          action={
            tw.kind === "action" ? (
              <Button size="sm" icon={Trash2} onClick={onCleanup}>
                {t("tweaks.cleanupOpen")}
              </Button>
            ) : undefined
          }
        />
      ))}
    </div>
  );
}

function readTab(fromParams: string | undefined): TweaksTab {
  const ids = TABS.map((x) => x.id);
  if (fromParams && (ids as string[]).includes(fromParams)) return fromParams as TweaksTab;
  try {
    const saved = localStorage.getItem(TAB_KEY);
    if (saved && (ids as string[]).includes(saved)) return saved as TweaksTab;
  } catch {
    /* geen opslag beschikbaar */
  }
  return "recommended";
}

export function TweaksPage() {
  const { t, td } = useT();
  const tweaks = useApp((s) => s.tweaks);
  const loading = useApp((s) => s.tweaksLoading);
  const refreshTweaks = useApp((s) => s.refreshTweaks);
  const refreshBigWins = useApp((s) => s.refreshBigWins);
  const refresh = () => Promise.all([refreshTweaks(), refreshBigWins()]);
  const setTweak = useApp((s) => s.setTweak);
  const appInfo = useApp((s) => s.appInfo);
  const pageParams = useApp((s) => s.pageParams);
  const toast = useApp((s) => s.toast);
  const handleError = useApp((s) => s.handleError);
  const [busy, setBusy] = useState<string | null>(null);
  const [cleanupOpen, setCleanupOpen] = useState(false);
  const [tab, setTab] = useState<TweaksTab>(() => readTab(pageParams.tab));
  const elevated = appInfo?.isElevated ?? false;

  // Vanaf het dashboard of een andere pagina: meteen het gevraagde tabblad openen.
  useEffect(() => {
    if (pageParams.tab) setTab(readTab(pageParams.tab));
  }, [pageParams.tab]);
  useEffect(() => {
    try {
      localStorage.setItem(TAB_KEY, tab);
    } catch {
      /* geen opslag beschikbaar */
    }
  }, [tab]);

  const onToggle = async (tw: TweakInfo, enabled: boolean, option: string | null) => {
    setBusy(tw.id);
    const info = await setTweak(tw.id, enabled, option);
    setBusy(null);
    if (info) {
      const name = td(`tweak.${tw.id}.name`);
      const msg = enabled ? t("tweakCard.applied", { name }) : t("tweakCard.disabled", { name });
      toast({ level: "success", title: t(`timing.${tw.timing}`), message: msg });
    }
  };

  const system = tweaks.filter((tw) => tw.category === "system" && !tw.experimental);
  const network = tweaks.filter((tw) => tw.category === "network" && !tw.experimental);
  const experimental = tweaks.filter((tw) => tw.experimental);
  const cleanup = tweaks.filter((tw) => tw.category === "cleanup");
  const grid = (list: TweakInfo[]) => <TweakGrid list={list} busy={busy} onToggle={onToggle} elevated={elevated} onCleanup={() => setCleanupOpen(true)} />;

  return (
    <div className="space-y-6">
      <PageHeader
        title={t("tweaks.title")}
        subtitle={t("tweaks.subtitle")}
        actions={
          <Button icon={RefreshCw} onClick={() => void refresh()} loading={loading}>
            {t("common.refresh")}
          </Button>
        }
      />

      {/* Tabbladen: alles wat met tweaks te maken heeft staat hier, overzichtelijk per onderwerp. */}
      <div className="sticky top-0 z-10 -mx-1 flex flex-wrap gap-1.5 rounded-xl border border-line bg-panel/85 p-1.5 backdrop-blur">
        {TABS.map(({ id, icon: Icon }) => (
          <button
            key={id}
            onClick={() => setTab(id)}
            className={cn(
              "flex items-center gap-2 rounded-lg px-3.5 py-2 text-sm font-medium transition-colors",
              tab === id ? "bg-accent-soft text-ink ring-1 ring-accent/30" : "text-ink-2 hover:bg-panel-2 hover:text-ink",
            )}
          >
            <Icon className={cn("h-4 w-4", tab === id && "text-accent")} />
            {t(`tweaks.tabs.${id}` as TKey)}
            {id === "experimental" && experimental.some((tw) => tw.state === "on") && <span className="h-1.5 w-1.5 rounded-full bg-warn" />}
          </button>
        ))}
      </div>
      <p className="-mt-2 px-1 text-[13px] text-ink-3">{t(`tweaks.tabsSub.${tab}` as TKey)}</p>

      {tab === "recommended" && (
        <div className="space-y-8">
          <BigWins />
          {loading && tweaks.length === 0 ? <Skeleton className="h-48" /> : <BestTweaks />}
        </div>
      )}

      {tab === "fortnite" && <FortniteSection />}

      {tab === "games" && <PerGame />}

      {tab === "windows" && (
        <div className="space-y-8">
          <section>
            <SectionTitle>{t("tweaks.system")}</SectionTitle>
            {loading && tweaks.length === 0 ? <Skeleton className="h-48" /> : grid(system)}
          </section>

          <Card className="flex flex-wrap items-center gap-4">
            <BellOff className="h-5 w-5 text-ink-3" />
            <div className="min-w-0 flex-1">
              <div className="text-sm font-semibold">{t("tweaks.dndTitle")}</div>
              <div className="text-[13px] text-ink-2">{t("tweaks.dndText")}</div>
            </div>
            <Button size="sm" icon={ExternalLink} onClick={() => api.openUrl("ms-settings:notifications").catch(handleError)}>
              {t("tweaks.openSettings")}
            </Button>
          </Card>

          <section>
            <SectionTitle>{t("tweaks.network")}</SectionTitle>
            {grid(network)}
          </section>

          <section>
            <SectionTitle>{t("tweaks.cleanup")}</SectionTitle>
            {grid(cleanup)}
          </section>
        </div>
      )}

      {tab === "experimental" && (
        <section>
          <p className="mb-3 flex items-center gap-2 text-[13px] text-ink-3">
            <Info className="h-4 w-4" />
            {t("tweaks.experimentalNote")}
          </p>
          {grid(experimental)}
        </section>
      )}

      <CleanupModal open={cleanupOpen} onClose={() => setCleanupOpen(false)} />
    </div>
  );
}
