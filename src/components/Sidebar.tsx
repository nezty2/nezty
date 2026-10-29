import {
  Crosshair,
  Gamepad2,
  Joystick,
  LayoutDashboard,
  Layers,
  Mouse,
  Power,
  ScrollText,
  Settings,
  ShieldAlert,
  ShieldCheck,
  SlidersHorizontal,
  Tv,
  Wifi,
  Zap,
  type LucideIcon,
} from "lucide-react";
import { useT } from "@/i18n/hooks";
import type { TKey } from "@/i18n";
import { useApp, type PageId } from "@/store/app";
import { cn } from "./ui";

const items: { id: PageId; key: TKey; icon: LucideIcon }[] = [
  { id: "dashboard", key: "nav.dashboard", icon: LayoutDashboard },
  { id: "games", key: "nav.games", icon: Gamepad2 },
  { id: "tweaks", key: "nav.tweaks", icon: SlidersHorizontal },
  { id: "input", key: "nav.input", icon: Mouse },
  { id: "controller", key: "nav.controller", icon: Joystick },
  { id: "console", key: "nav.console", icon: Tv },
  { id: "crosshair", key: "nav.crosshair", icon: Crosshair },
  { id: "background", key: "nav.background", icon: Layers },
  { id: "network", key: "nav.network", icon: Wifi },
  { id: "startup", key: "nav.startup", icon: Power },
  { id: "log", key: "nav.log", icon: ScrollText },
  { id: "settings", key: "nav.settings", icon: Settings },
];

export function Sidebar() {
  const { t } = useT();
  const page = useApp((s) => s.page);
  const setPage = useApp((s) => s.setPage);
  const appInfo = useApp((s) => s.appInfo);
  const boost = useApp((s) => s.boost);
  const relaunch = useApp((s) => s.relaunchElevated);
  const elevated = appInfo?.isElevated ?? false;

  return (
    <aside className="flex h-full w-[68px] shrink-0 flex-col border-r border-line bg-panel min-[1120px]:w-[224px]">
      <div className="flex h-16 items-center gap-3 px-4 min-[1120px]:px-5">
        <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-accent">
          <Zap className="h-5 w-5 text-white" fill="currentColor" />
        </div>
        <div className="hidden min-w-0 min-[1120px]:block">
          <div className="text-[15px] font-semibold leading-tight">nezty</div>
          <div className="text-[12px] text-ink-3">v{appInfo?.version ?? ""}</div>
        </div>
      </div>

      <nav className="scroll-thin flex flex-1 flex-col gap-0.5 overflow-y-auto px-2.5 py-2">
        {items.map(({ id, key, icon: Icon }) => {
          const active = page === id;
          return (
            <button
              key={id}
              onClick={() => setPage(id)}
              title={t(key)}
              className={cn(
                "flex h-10 items-center gap-3 rounded-lg px-3 text-sm font-medium transition-colors",
                active ? "bg-accent-soft text-ink" : "text-ink-2 hover:bg-panel-2 hover:text-ink",
              )}
            >
              <Icon className={cn("h-[18px] w-[18px] shrink-0", active && "text-accent")} />
              <span className="hidden truncate min-[1120px]:inline">{t(key)}</span>
              {id === "dashboard" && boost.active && <span className="ml-auto hidden h-2 w-2 rounded-full bg-ok min-[1120px]:block" />}
            </button>
          );
        })}
      </nav>

      <div className="space-y-1.5 border-t border-line p-2.5">
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
        {elevated ? (
          <div className="flex items-center gap-2 px-3 py-2 text-[13px] text-ink-3" title={t("sidebar.admin")}>
            <ShieldCheck className="h-4 w-4 shrink-0 text-ok" />
            <span className="hidden truncate min-[1120px]:inline">{t("sidebar.admin")}</span>
          </div>
        ) : (
          <button
            onClick={() => void relaunch()}
            className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left text-[13px] text-warn hover:bg-panel-2"
            title={t("sidebar.limitedHint")}
          >
            <ShieldAlert className="h-4 w-4 shrink-0" />
            <span className="hidden truncate min-[1120px]:inline">{t("sidebar.limited")}</span>
          </button>
        )}
      </div>
    </aside>
  );
}
