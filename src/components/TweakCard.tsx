import { AnimatePresence, motion } from "framer-motion";
import { Check, ChevronDown, FlaskConical, Laptop, Lock, Monitor, Power, RotateCcw, Sparkles, Zap } from "lucide-react";
import { useState, type ReactNode } from "react";
import { useT } from "@/i18n/hooks";
import type { Gain, Timing, TweakInfo } from "@/lib/types";
import { Badge, SegmentedControl, Toggle, cn, type BadgeVariant } from "./ui";

const timingIcon = { instant: Zap, game_restart: RotateCcw, pc_restart: Power };

export function TimingBadge({ timing }: { timing: Timing }) {
  const { t } = useT();
  return (
    <Badge variant={timing === "instant" ? "ok" : timing === "game_restart" ? "info" : "warn"} icon={timingIcon[timing]}>
      {t(`timing.${timing}`)}
    </Badge>
  );
}

export function GainBadge({ gain }: { gain: Gain }) {
  const { t } = useT();
  const variant: BadgeVariant = gain === "big" ? "accent" : gain === "measurable" ? "info" : "neutral";
  return (
    <Badge variant={variant} className={cn(gain === "big" && "font-semibold", gain === "negligible" && "opacity-75")}>
      {t(`gain.${gain}`)}
    </Badge>
  );
}

/** Statuslabel, concreet: werkt nu, of nog niet (herstart pc of game), of niet beschikbaar. */
export function StateBadge({ tweak }: { tweak: TweakInfo }) {
  const { t } = useT();
  if (tweak.state === "unavailable") return <Badge variant="neutral">{t("tweakCard.unavailable")}</Badge>;
  if (tweak.state !== "on") return null;
  if (tweak.active === "after_pc_restart") return <Badge variant="warn" icon={Power}>{t("tweakCard.pendingPc")}</Badge>;
  if (tweak.active === "after_game_restart") return <Badge variant="warn" icon={RotateCcw}>{t("tweakCard.pendingGame")}</Badge>;
  if (tweak.appliedByApp) return <Badge variant="ok" icon={Sparkles}>{t("tweakCard.activeByApp")}</Badge>;
  return <Badge variant="ok" icon={Check}>{t("tweakCard.activeAlready")}</Badge>;
}

export function TweakCard({
  tweak,
  onToggle,
  busy,
  elevated,
  action,
}: {
  tweak: TweakInfo;
  onToggle: (enabled: boolean, option: string | null) => void;
  busy?: boolean;
  elevated: boolean;
  /** Voor actie-tweaks (opruimen): een knop in plaats van een schakelaar. */
  action?: ReactNode;
}) {
  const { t, td, tl } = useT();
  const [open, setOpen] = useState(false);
  const [why, setWhy] = useState(false);
  const isOn = tweak.state === "on";
  // Past niet bij dit apparaat: grijs en niet te bedienen. Heeft nezty hem eerder zelf aangezet, dan mag hij wel uit.
  const platformBlocked = tweak.platformLocked && !(isOn && tweak.appliedByApp);
  const unavailable = tweak.state === "unavailable";
  const locked = tweak.requiresAdmin && !elevated;
  const base = `tweak.${tweak.id}`;

  return (
    <div className={cn("card flex flex-col p-5", (unavailable || platformBlocked) && "opacity-70", platformBlocked && "grayscale")}>
      {/* Kop: naam links, schakelaar rechts op een vaste plek */}
      <div className="grid grid-cols-[1fr_auto] items-center gap-4">
        <h3 className="text-[15px] font-semibold leading-snug">{td(`${base}.name`, undefined, tweak.id)}</h3>
        {action ?? (
          <Toggle checked={isOn} onChange={(v) => onToggle(v, tweak.currentOption)} loading={busy} disabled={unavailable || locked || platformBlocked} label={td(`${base}.name`)} />
        )}
      </div>

      <div className="mt-2.5 flex flex-wrap gap-1.5">
        {tweak.platform !== "all" && (
          <button type="button" onClick={() => setWhy((v) => !v)} title={t("platform.whyHint")} className="rounded-md">
            <Badge variant={tweak.platformLocked ? "neutral" : "info"} icon={tweak.platformLocked ? Lock : tweak.platform === "laptop_only" ? Laptop : Monitor}>
              {t(`platform.${tweak.platform}` as "platform.pc_only")}
            </Badge>
          </button>
        )}
        {tweak.kind === "toggle" && <TimingBadge timing={tweak.timing} />}
        <GainBadge gain={tweak.gain} />
        <StateBadge tweak={tweak} />
        {tweak.risk === "medium" && <Badge variant="warn">{t("tweakCard.riskMedium")}</Badge>}
        {tweak.experimental && <Badge variant="warn" icon={FlaskConical}>{t("tweakCard.experimental")}</Badge>}
        {locked && <Badge variant="warn" icon={Lock} title={t("tweakCard.adminHint")}>{t("tweakCard.adminNeeded")}</Badge>}
      </div>

      {why && tweak.platform !== "all" && (
        <p className="mt-2.5 flex items-start gap-2 text-[12px] leading-relaxed text-ink-2">
          <Lock className="mt-0.5 h-3.5 w-3.5 shrink-0 text-ink-3" />
          {td(`${base}.platformWhy`, undefined, t(`platform.why.${tweak.platform}` as "platform.why.pc_only"))}
        </p>
      )}

      <dl className="mt-3.5 space-y-2 text-[13px] leading-relaxed">
        <div>
          <dt className="text-ink-3">{t("tweakCard.what")}</dt>
          <dd className="text-ink">{td(`${base}.what`)}</dd>
        </div>
        <div>
          <dt className="text-ink-3">{t("tweakCard.effect")}</dt>
          <dd className="text-ink">{td(`${base}.effect`)}</dd>
        </div>
      </dl>

      {tweak.options && tweak.options.length > 0 && (
        <div className="mt-3.5 flex flex-wrap items-center gap-3">
          <SegmentedControl
            options={tweak.options.map((o) => ({ value: o, label: td(`${base}.opt.${o}`, undefined, o) }))}
            value={tweak.currentOption ?? tweak.options[0]}
            onChange={(v) => onToggle(true, v)}
          />
          {tweak.currentOption === "ultimate" && <span className="text-[12px] text-ink-3">{td(`${base}.opt.ultimate_note`)}</span>}
        </div>
      )}

      {tweak.detail && <p className="mt-3 text-[12px] text-ink-3">{td(`msg.${tweak.detail.key}`, tweak.detail.params)}</p>}
      {isOn && (tweak.active === "after_pc_restart" || tweak.active === "after_game_restart") && (
        <p className="mt-3 flex items-start gap-2 text-[12px] text-warn">
          <Power className="mt-0.5 h-3.5 w-3.5 shrink-0" />
          {tweak.active === "after_pc_restart" ? t("tweakCard.pendingPcHint") : t("tweakCard.pendingGameHint")}
        </p>
      )}

      <div className="mt-auto pt-3">
        <button onClick={() => setOpen((o) => !o)} className="flex items-center gap-1 text-[12px] font-medium text-ink-3 hover:text-ink-2">
          <ChevronDown className={cn("h-3.5 w-3.5 transition-transform", open && "rotate-180")} />
          {open ? t("common.lessInfo") : t("common.moreInfo")}
        </button>
        <AnimatePresence initial={false}>
          {open && (
            <motion.ul
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: "auto", opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              transition={{ duration: 0.15 }}
              className="overflow-hidden"
            >
              <div className="card-inset mt-2 space-y-1 px-3 py-2.5 text-[12px] leading-relaxed text-ink-2">
                {tl(`${base}.tech`).map((d, i) => (
                  <li key={i} className="selectable list-none font-mono text-[11.5px]">
                    {d}
                  </li>
                ))}
                {locked && <li className="list-none text-warn">{t("tweakCard.adminHint")}</li>}
              </div>
            </motion.ul>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}
