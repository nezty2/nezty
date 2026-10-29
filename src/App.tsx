import { AnimatePresence, MotionConfig, motion } from "framer-motion";
import { useEffect, useRef } from "react";
import { BoostResultModal } from "@/components/BoostResultModal";
import { Sidebar } from "@/components/Sidebar";
import { Toasts } from "@/components/Toasts";
import { UpdateBanner } from "@/components/UpdateBanner";
import { Spinner } from "@/components/ui";
import { WelcomeModal } from "@/components/WelcomeModal";
import { useT } from "@/i18n/hooks";
import { LOCALES } from "@/i18n";
import { isTauri } from "@/lib/ipc";
import { BackgroundAppsPage } from "@/pages/BackgroundApps";
import { CrosshairPage } from "@/pages/Crosshair";
import { DashboardPage } from "@/pages/Dashboard";
import { GamesPage } from "@/pages/Games";
import { InputPage } from "@/pages/Input";
import { ControllerPage } from "@/pages/Controller";
import { ConsolePage } from "@/pages/Console";
import { LogPage } from "@/pages/Log";
import { NetworkPage } from "@/pages/Network";
import { SettingsPage } from "@/pages/Settings";
import { StartupPage } from "@/pages/Startup";
import { TweaksPage } from "@/pages/Tweaks";
import { useApp } from "@/store/app";

const pages = {
  dashboard: DashboardPage,
  games: GamesPage,
  tweaks: TweaksPage,
  input: InputPage,
  controller: ControllerPage,
  console: ConsolePage,
  crosshair: CrosshairPage,
  background: BackgroundAppsPage,
  network: NetworkPage,
  startup: StartupPage,
  log: LogPage,
  settings: SettingsPage,
};

export default function App() {
  const { t, lang } = useT();
  const init = useApp((s) => s.init);
  const ready = useApp((s) => s.ready);
  const page = useApp((s) => s.page);
  const settings = useApp((s) => s.settings);
  const lastReport = useApp((s) => s.lastReport);
  const showResult = useApp((s) => s.showResult);
  const setShowResult = useApp((s) => s.setShowResult);
  const mainRef = useRef<HTMLElement>(null);

  useEffect(() => {
    void init();
  }, [init]);

  useEffect(() => {
    document.documentElement.dataset.accent = settings?.accent ?? "violet";
  }, [settings?.accent]);

  useEffect(() => {
    document.documentElement.lang = LOCALES[lang].slice(0, 2);
  }, [lang]);

  useEffect(() => {
    mainRef.current?.scrollTo({ top: 0 });
  }, [page]);

  const Page = pages[page];

  return (
    <MotionConfig reducedMotion={settings?.reduceMotion ? "always" : "user"}>
      <div className="flex h-full">
        <Sidebar />
        <main ref={mainRef} className="scroll-thin relative min-w-0 flex-1 overflow-y-auto">
          {!isTauri && <div className="sticky top-0 z-20 border-b border-warn/30 bg-[#2a2416] px-4 py-1 text-center text-[12px] text-warn">{t("common.browserPreview")}</div>}
          {!ready ? (
            <div className="flex h-full items-center justify-center">
              <Spinner className="h-8 w-8" />
            </div>
          ) : (
            <AnimatePresence mode="wait" initial={false}>
              <motion.div
                key={page}
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.12 }}
                className="mx-auto w-full max-w-[1400px] px-6 py-6 md:px-8"
              >
                <UpdateBanner />
                <Page />
              </motion.div>
            </AnimatePresence>
          )}
        </main>
        <Toasts />
        <WelcomeModal />
        <BoostResultModal report={lastReport} open={showResult} onClose={() => setShowResult(false)} />
      </div>
    </MotionConfig>
  );
}
