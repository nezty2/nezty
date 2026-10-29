import {
  Crosshair,
  Joystick,
  LayoutDashboard,
  Laptop, Layers,
  Mouse,
  Power,
  ScrollText,
  Settings,
  SlidersHorizontal,
  Wifi,
  Zap,
  type LucideIcon,
} from "lucide-react";
import { motion } from "framer-motion";
import { useT } from "@/i18n/hooks";
import type { TKey } from "@/i18n";
import { useApp, type PageId } from "@/store/app";
import { LicenseBlock } from "./license/LicenseBlock";
import { cn } from "./ui";
import { UpdateButton } from "./UpdateButton";

const items: { id: PageId; key: TKey; icon: LucideIcon }[] = [
  { id: "dashboard", key: "nav.dashboard", icon: LayoutDashboard },
  { id: "tweaks", key: "nav.tweaks", icon: SlidersHorizontal },
  { id: "laptop", key: "nav.laptop", icon: Laptop },
  { id: "input", key: "nav.input", icon: Mouse },
  { id: "controller", key: "nav.controller", icon: Joystick },
  { id: "crosshair", key: "nav.crosshair", icon: Crosshair },
  { id: "background", key: "nav.background", icon: Layers },
  { id: "network", key: "nav.network", icon: Wifi },
  { id: "startup", key: "nav.startup", icon: Power },
  { id: "log", key: "nav.log", icon: ScrollText },
];

function NavButton({ label, icon: Icon, active, dot, onClick }: { id: PageId; label: string; icon: LucideIcon; active: boolean; dot?: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      title={label}
      className={cn(
        "relative flex h-10 w-full items-center gap-3 rounded-lg px-3 text-sm font-medium transition-colors",
        active ? "text-ink" : "text-ink-2 hover:bg-panel-2 hover:text-ink",
      )}
    >
      {active && (
        <motion.span
          layoutId="nav-active"
          className="absolute inset-0 rounded-lg bg-accent-soft ring-1 ring-accent/30"
          transition={{ type: "spring", stiffness: 520, damping: 40 }}
        />
      )}
      <Icon className={cn("relative h-[18px] w-[18px] shrink-0", active && "text-accent")} />
      <span className="relative hidden truncate min-[1120px]:inline">{label}</span>
      {dot && <span className="relative ml-auto hidden h-2 w-2 rounded-full bg-ok min-[1120px]:block" />}
    </button>
  );
}

export function Sidebar() {
  const { t } = useT();
  const page = useApp((s) => s.page);
  const setPage = useApp((s) => s.setPage);
  const appInfo = useApp((s) => s.appInfo);
  const boost = useApp((s) => s.boost);

  return (
    <aside className="glass relative z-10 flex h-full w-[68px] shrink-0 flex-col border-r border-line min-[1120px]:w-[224px]">
      <div className="flex h-16 items-center gap-3 px-4 min-[1120px]:px-5">
        <div className="btn-primary flex h-9 w-9 shrink-0 items-center justify-center rounded-lg">
          <Zap className="h-5 w-5 text-white" fill="currentColor" />
        </div>
        <div className="hidden min-w-0 min-[1120px]:block">
          <div className="text-[15px] font-semibold leading-tight">nezty</div>
          <div className="text-[12px] text-ink-3">v{appInfo?.version ?? ""}</div>
        </div>
      </div>

      <LicenseBlock />

      <nav className="scroll-thin flex flex-1 flex-col gap-0.5 overflow-y-auto px-2.5 py-2">
        {items.map(({ id, key, icon }) => (
          <NavButton key={id} id={id} label={t(key)} icon={icon} active={page === id} dot={id === "dashboard" && boost.active} onClick={() => setPage(id)} />
        ))}
      </nav>

      <div className="space-y-0.5 border-t border-line p-2.5">
        {boost.active && (
          <button
            onClick={() => setPage("dashboard")}
            className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left text-[13px] font-medium text-ok hover:bg-panel-2"
            title={t("sidebar.boostActive")}
          >
            <Zap className="h-4 w-4 shrink-0" fill="currentColor" />
            <span className="hidden truncate min-[1120px]:inline">{t("sidebar.boostActive")}</span>
          </button>
        )}
        <NavButton id="settings" label={t("nav.settings")} icon={Settings} active={page === "settings"} onClick={() => setPage("settings")} />
        <UpdateButton />
      </div>
    </aside>
  );
}
