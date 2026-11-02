import { CheckCircle2, ChevronRight, CircleDashed, Gamepad2, Info, Power, RotateCcw } from "lucide-react";
import { useState } from "react";
import { useT } from "@/i18n/hooks";
import { api } from "@/lib/ipc";
import { useApp } from "@/store/app";
import { Button, Modal, cn } from "./ui";

export type StatusKind = "loading" | "restart" | "ready" | "game" | "partial" | "idle";

/** De ene status van de app. Oranje alleen bij "herstart je pc", groen als alles aan staat, anders grijs. */
export function useStatusKind(): StatusKind {
  const restart = useApp((s) => s.restart);
  const applyStatus = useApp((s) => s.applyStatus);
  if (restart?.required) return "restart";
  if (!applyStatus) return "loading";
  if (applyStatus.allActive) return "ready";
  // Alles staat aan, maar een paar instellingen werken pas als de game opnieuw start: grijs, niet groen.
  if (applyStatus.missing.length === 0 && applyStatus.pendingGame.length > 0) return "game";
  return applyStatus.enabled ? "partial" : "idle";
}

/** Knop "Nu herstarten" met een bevestiging vooraf: Windows start echt opnieuw op. */
export function RestartButton({ size = "sm" }: { size?: "sm" | "md" }) {
  const { t } = useT();
  const handleError = useApp((s) => s.handleError);
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const restart = async () => {
    setBusy(true);
    try {
      await api.restartPc();
      setOpen(false);
    } catch (e) {
      handleError(e);
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <Button
        size={size}
        icon={RotateCcw}
        className="border-warn/40 text-warn hover:border-warn/60"
        onClick={(e) => {
          e.stopPropagation();
          setOpen(true);
        }}
        onKeyDown={(e) => e.stopPropagation()}
      >
        {t("status.restartNow")}
      </Button>
      {/* Het venster staat in een portal, maar React-events lopen toch naar de kaart erboven: hier tegenhouden. */}
      <span className="contents" onClick={(e) => e.stopPropagation()} onKeyDown={(e) => e.stopPropagation()}>
        <Modal
          open={open}
          onClose={() => setOpen(false)}
          title={t("status.confirmTitle")}
          width="max-w-md"
          footer={
            <>
              <Button variant="ghost" onClick={() => setOpen(false)}>
                {t("common.cancel")}
              </Button>
              <Button variant="primary" icon={Power} loading={busy} onClick={() => void restart()}>
                {t("status.confirmYes")}
              </Button>
            </>
          }
        >
          <p className="text-sm leading-relaxed text-ink-2">{t("status.confirmText")}</p>
        </Modal>
      </span>
    </>
  );
}

function useStatusText(kind: StatusKind) {
  const { t, td } = useT();
  const restart = useApp((s) => s.restart);
  const applyStatus = useApp((s) => s.applyStatus);
  switch (kind) {
    case "restart": {
      const names = (restart?.tweaks ?? []).map((id) => td(`tweak.${id}.name`, undefined, id)).join(", ");
      return { title: t("status.restart"), sub: names ? t("status.restartSub", { names }) : t("status.restartSubGeneric") };
    }
    case "ready":
      return { title: t("status.ready"), sub: t("status.readySub") };
    case "game":
      return { title: t("status.game"), sub: t("status.gameSub") };
    case "partial":
      return { title: t("status.partial"), sub: t("status.partialSub", { active: applyStatus?.active ?? 0, total: applyStatus?.total ?? 0 }) };
    case "idle":
      return { title: t("status.idle"), sub: t("status.idleSub") };
    default:
      return { title: t("status.checking"), sub: "" };
  }
}

const iconOf = { restart: Power, ready: CheckCircle2, game: Gamepad2, partial: CircleDashed, idle: CircleDashed, loading: CircleDashed };
const toneOf: Record<StatusKind, string> = {
  restart: "bg-warn/12 text-warn",
  ready: "bg-ok/12 text-ok",
  game: "bg-panel-2 text-ink-3",
  partial: "bg-panel-2 text-ink-3",
  idle: "bg-panel-2 text-ink-3",
  loading: "bg-panel-2 text-ink-3",
};

/** Statusbalk bovenaan de tweakspagina. */
export function StatusBar() {
  const { t } = useT();
  const kind = useStatusKind();
  const restart = useApp((s) => s.restart);
  const { title, sub } = useStatusText(kind);
  const Icon = iconOf[kind];
  return (
    <div className={cn("card flex flex-wrap items-center gap-4 px-5 py-4", kind === "restart" && "border-warn/40", kind === "ready" && "border-ok/30")}>
      <span className={cn("flex h-10 w-10 shrink-0 items-center justify-center rounded-xl", toneOf[kind])}>
        <Icon className={cn("h-5 w-5", kind === "loading" && "opacity-60")} />
      </span>
      <div className="min-w-0 flex-1">
        <div className={cn("text-[15px] font-semibold", kind === "restart" && "text-warn", kind === "ready" && "text-ok")}>{title}</div>
        {sub && <div className="mt-0.5 text-[13px] text-ink-2">{sub}</div>}
        {kind === "restart" && restart?.fastStartup && (
          <div className="mt-1.5 flex items-start gap-1.5 text-[12px] text-ink-3">
            <Info className="mt-0.5 h-3.5 w-3.5 shrink-0" />
            {t("status.fastStartup")}
          </div>
        )}
      </div>
      {kind === "restart" && <RestartButton size="md" />}
    </div>
  );
}

/** Compacte versie voor het dashboard. Klik gaat naar Tweaks. */
export function StatusCompact() {
  const { t } = useT();
  const kind = useStatusKind();
  const setPage = useApp((s) => s.setPage);
  const { title, sub } = useStatusText(kind);
  const Icon = iconOf[kind];
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={() => setPage("tweaks")}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") setPage("tweaks");
      }}
      className={cn("card flex w-full items-center gap-3 p-4 text-left transition-colors hover:border-line-2", kind === "restart" && "border-warn/40", kind === "ready" && "border-ok/30")}
    >
      <span className={cn("flex h-9 w-9 shrink-0 items-center justify-center rounded-lg", toneOf[kind])}>
        <Icon className="h-4 w-4" />
      </span>
      <div className="min-w-0 flex-1">
        <div className={cn("text-sm font-semibold", kind === "restart" && "text-warn", kind === "ready" && "text-ok")}>{title}</div>
        <div className="truncate text-[13px] text-ink-3">{kind === "idle" || kind === "partial" ? t("status.openTweaks") : sub}</div>
      </div>
      {kind === "restart" ? <RestartButton /> : <ChevronRight className="h-4 w-4 text-ink-3" />}
    </div>
  );
}
