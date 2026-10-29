import { AnimatePresence, MotionConfig, motion } from "framer-motion";
import { useEffect, useRef, useState } from "react";
import { BoostResultModal } from "@/components/BoostResultModal";
import { Sidebar } from "@/components/Sidebar";
import { Toasts } from "@/components/Toasts";
import { AppBackground } from "@/components/AppBackground";
import { Splash } from "@/components/Splash";
import { Intro } from "@/intro/Intro";
import { WelcomeModal } from "@/components/WelcomeModal";
import { useT } from "@/i18n/hooks";
import { LOCALES } from "@/i18n";
import { isTauri } from "@/lib/ipc";
import { BackgroundAppsPage } from "@/pages/BackgroundApps";
import { CrosshairPage } from "@/pages/Crosshair";
import { DashboardPage } from "@/pages/Dashboard";
import { InputPage } from "@/pages/Input";
import { ControllerPage } from "@/pages/Controller";
import { LogPage } from "@/pages/Log";
import { NetworkPage } from "@/pages/Network";
import { SettingsPage } from "@/pages/Settings";
import { StartupPage } from "@/pages/Startup";
import { TweaksPage } from "@/pages/Tweaks";
import { useApp } from "@/store/app";

const pages = {
  dashboard: DashboardPage,
  tweaks: TweaksPage,
  input: InputPage,
  controller: ControllerPage,
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

  // Openingsanimatie (elke keer dat de app opent). Daarna, als de app nog niet klaar is, het eenvoudige laadscherm.
  const [intro, setIntro] = useState(true);

  useEffect(() => {
    const root = document.documentElement;
    root.dataset.accent = settings?.accent ?? "violet";
    root.dataset.theme = settings?.theme ?? "midnight";
    root.dataset.motion = settings?.reduceMotion ? "reduce" : "full";
    const custom = settings?.accentCustom;
    if (custom && /^#[0-9a-f]{6}$/i.test(custom)) {
      root.style.setProperty("--accent", custom);
      root.style.setProperty("--accent-soft", `color-mix(in srgb, ${custom} 16%, transparent)`);
    } else {
      root.style.removeProperty("--accent");
      root.style.removeProperty("--accent-soft");
    }
  }, [settings?.accent, settings?.accentCustom, settings?.theme, settings?.reduceMotion]);

  useEffect(() => {
    document.documentElement.lang = LOCALES[lang].slice(0, 2);
  }, [lang]);

  useEffect(() => {
    mainRef.current?.scrollTo({ top: 0 });
  }, [page]);

  const Page = pages[page];

  return (
    <MotionConfig reducedMotion={settings?.reduceMotion ? "always" : "never"}>
      <AppBackground />
      <div className="relative flex h-full">
        <Sidebar />
        <main ref={mainRef} className="scroll-thin relative min-w-0 flex-1 overflow-y-auto">
          {!isTauri && <div className="sticky top-0 z-20 border-b border-warn/30 bg-[#2a2416] px-4 py-1 text-center text-[12px] text-warn">{t("common.browserPreview")}</div>}
          {ready && (
            <AnimatePresence mode="wait" initial={false}>
              <motion.div
                key={page}
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -6 }}
                transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
                className="mx-auto w-full max-w-[1400px] px-6 py-6 md:px-8"
              >
                <div className="stagger">
                  <Page />
                </div>
              </motion.div>
            </AnimatePresence>
          )}
        </main>
        <Toasts />
        <WelcomeModal />
        <BoostResultModal report={lastReport} open={showResult} onClose={() => setShowResult(false)} />
        {intro && <Intro onDone={() => setIntro(false)} />}
        <Splash show={!intro && !ready} />
      </div>
    </MotionConfig>
  );
}
