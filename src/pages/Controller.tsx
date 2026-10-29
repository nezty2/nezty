import { AlertTriangle, BatteryLow, Bluetooth, Cable, CheckCircle2, ExternalLink, Gauge, Info, Joystick, RefreshCw, Wifi, Zap } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { TweakCard } from "@/components/TweakCard";
import { Badge, Button, Card, PageHeader, SectionTitle, Skeleton, cn } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import { formatMs } from "@/lib/format";
import { api } from "@/lib/ipc";
import type { ControllerInfo, ControllerRate, TweakInfo } from "@/lib/types";
import { useApp } from "@/store/app";

/** Windows-instellingen die ook voor controllers helpen. Geen eigen tweaks: dit zijn dezelfde, gedeelde instellingen. */
const CONTROLLER_TWEAKS = ["game_mode", "game_bar_dvr_off", "windowed_opt", "power_plan", "hags", "usb_suspend_off"];
const XBOX_ACCESSORIES = "https://apps.microsoft.com/detail/9nblggh30xj3";
const STANDARD_RATES = [60, 125, 250, 500, 1000];

function snap(hz: number): number {
  return STANDARD_RATES.find((r) => Math.abs(r - hz) / r < 0.15) ?? Math.round(hz / 5) * 5;
}

/** Aangesloten Xbox-compatibele controllers, met verbinding en batterij. */
function Devices({ list, loading, onRefresh }: { list: ControllerInfo[]; loading: boolean; onRefresh: () => void }) {
  const { t } = useT();
  return (
    <section>
      <SectionTitle
        right={
          <Button size="sm" variant="ghost" icon={RefreshCw} loading={loading} onClick={onRefresh}>
            {t("controller.refresh")}
          </Button>
        }
      >
        {t("controller.devices")}
      </SectionTitle>
      {loading && list.length === 0 ? (
        <Skeleton className="h-20" />
      ) : list.length === 0 ? (
        <Card>
          <p className="text-[13px] text-ink">{t("controller.none")}</p>
          <p className="mt-1 text-[12px] text-ink-3">{t("controller.noneNote")}</p>
        </Card>
      ) : (
        <div className="grid gap-4 lg:grid-cols-2">
          {list.map((c) => (
            <Card key={c.id}>
              <div className="flex items-center gap-3">
                <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-accent-soft text-accent">
                  <Joystick className="h-5 w-5" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="text-[15px] font-semibold">
                    {t(`controller.models.${c.model}`)}
                    {c.slot != null && list.filter((x) => x.kind === "xbox").length > 1 && <span className="ml-1.5 text-ink-3">{c.slot + 1}</span>}
                  </div>
                  <div className="mt-1 flex flex-wrap gap-1.5">
                    {c.wired === true && (
                      <Badge variant="ok" icon={Cable}>
                        {t("controller.wired")}
                      </Badge>
                    )}
                    {c.wired === false && (
                      <Badge variant="info" icon={Wifi}>
                        {t("controller.wireless")}
                      </Badge>
                    )}
                    {c.wired === null && <Badge>{t("controller.unknownConnection")}</Badge>}
                    {c.batteryPercent != null && (
                      <Badge variant={c.batteryPercent <= 20 ? "warn" : "neutral"} icon={BatteryLow}>
                        {t("controller.batteryPercent", { pct: c.batteryPercent })}
                      </Badge>
                    )}
                    {c.charging && <Badge variant="ok">{t("controller.charging")}</Badge>}
                    {c.battery && (
                      <Badge variant={c.battery === "empty" || c.battery === "low" ? "warn" : "neutral"} icon={BatteryLow}>
                        {t(`controller.battery.${c.battery}`)}
                      </Badge>
                    )}
                  </div>
                </div>
              </div>
              {c.wired === false && <p className="mt-3 text-[13px] text-ink-2">{t("controller.wirelessAdvice")}</p>}
              {(c.battery === "empty" || c.battery === "low" || (c.batteryPercent != null && c.batteryPercent <= 20 && !c.charging)) && <p className="mt-2 text-[13px] text-warn">{t("controller.lowBattery")}</p>}
            </Card>
          ))}
        </div>
      )}
    </section>
  );
}

/** Meet hoe vaak de controller een nieuwe stand doorgeeft (via de app, alleen uitlezen). */
function RateTester({ list, onResult }: { list: ControllerInfo[]; onResult: (r: ControllerRate) => void }) {
  const { t, lang } = useT();
  const handleError = useApp((s) => s.handleError);
  const [slot, setSlot] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<ControllerRate | null>(null);

  useEffect(() => {
    if (slot == null || !list.some((c) => c.id === slot)) setSlot(list[0]?.id ?? null);
  }, [list, slot]);

  const measure = async () => {
    if (slot == null) return;
    setBusy(true);
    setResult(null);
    try {
      const r = await api.measureControllerRate(slot, 4);
      setResult(r);
      onResult(r);
    } catch (e) {
      handleError(e);
    } finally {
      setBusy(false);
    }
  };

  const hz = result?.hz != null ? snap(result.hz) : null;
  const message =
    result == null
      ? null
      : !result.connected
        ? t("controller.rateDisconnected")
        : hz == null
          ? t("controller.rateNoMove")
          : hz < 100
            ? t("controller.rateLow", { hz })
            : hz < 250
              ? t("controller.rateOk", { hz })
              : t("controller.rateGood", { hz });

  return (
    <Card>
      <div className="flex items-start justify-between gap-4">
        <div>
          <h3 className="flex items-center gap-2 text-[15px] font-semibold">
            <Gauge className="h-4 w-4 text-accent" />
            {t("controller.rateTitle")}
          </h3>
          <p className="mt-1 text-[13px] text-ink-2">{t("controller.rateDesc")}</p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          {list.length > 1 &&
            list.map((c, i) => (
              <button
                key={c.id}
                title={t(`controller.models.${c.model}`)}
                onClick={() => setSlot(c.id)}
                className={cn("h-8 min-w-8 rounded-md border px-2 text-[13px] font-semibold", slot === c.id ? "border-accent text-accent" : "border-line-2 text-ink-2")}
              >
                {c.kind === "playstation" ? "PS" : "Xbox"} {list.filter((x) => x.kind === c.kind).length > 1 ? i + 1 : ""}
              </button>
            ))}
          <Button size="sm" variant="primary" loading={busy} disabled={slot == null} onClick={() => void measure()}>
            {t("controller.rateButton")}
          </Button>
        </div>
      </div>
      <div className="mt-4 grid gap-4 md:grid-cols-[minmax(0,1fr)_220px]">
        <div className="flex min-h-24 items-center rounded-lg border border-dashed border-line-2 bg-panel-2 px-4 py-3 text-[13px] leading-relaxed">
          {busy ? (
            <span className="text-ink-2">{t("controller.rateMeasuring")}</span>
          ) : message ? (
            <span className="text-ink">
              {message}
              {hz != null && <span className="mt-1 block text-ink-2">{t("controller.rateWait", { ms: formatMs(lang, 1000 / hz / 2) })}</span>}
            </span>
          ) : (
            <Joystick className="mx-auto h-6 w-6 text-ink-3" />
          )}
        </div>
        <div className="card-inset flex flex-col justify-center p-4 text-center">
          <div className={cn("text-[34px] font-semibold tnum", hz == null ? "text-ink-3" : hz < 100 ? "text-warn" : "text-ok")}>{hz == null ? "-" : t("controller.rateResult", { hz })}</div>
        </div>
      </div>
      <p className="mt-2 text-[12px] text-ink-3">{t("controller.rateNote")}</p>
    </Card>
  );
}

type Axes = [number, number, number, number];

/** Live stand van de sticks (Gamepad API, werkt ook voor PlayStation) en een meting van de rust-afwijking. */
function Sticks() {
  const { t } = useT();
  const [name, setName] = useState<string | null>(null);
  const [axes, setAxes] = useState<Axes>([0, 0, 0, 0]);
  const [measuring, setMeasuring] = useState(false);
  const [drift, setDrift] = useState<{ left: number; right: number; lt: number; rt: number } | null>(null);
  const peak = useRef<{ left: number; right: number; lt: number; rt: number } | null>(null);

  useEffect(() => {
    let raf = 0;
    const loop = () => {
      const pad = (navigator.getGamepads?.() ?? []).find((p) => p && p.connected) ?? null;
      if (pad) {
        setName(pad.id.replace(/\s*\(.*\)\s*$/, "").trim() || pad.id);
        const a: Axes = [pad.axes[0] ?? 0, pad.axes[1] ?? 0, pad.axes[2] ?? 0, pad.axes[3] ?? 0];
        setAxes(a);
        if (peak.current) {
          peak.current.left = Math.max(peak.current.left, Math.hypot(a[0], a[1]));
          peak.current.right = Math.max(peak.current.right, Math.hypot(a[2], a[3]));
          peak.current.lt = Math.max(peak.current.lt, pad.buttons[6]?.value ?? 0);
          peak.current.rt = Math.max(peak.current.rt, pad.buttons[7]?.value ?? 0);
        }
      } else {
        setName(null);
      }
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, []);

  const measure = useCallback(() => {
    setDrift(null);
    setMeasuring(true);
    peak.current = { left: 0, right: 0, lt: 0, rt: 0 };
    setTimeout(() => {
      const p = peak.current ?? { left: 0, right: 0, lt: 0, rt: 0 };
      peak.current = null;
      setDrift({ left: Math.round(p.left * 100), right: Math.round(p.right * 100), lt: Math.round(p.lt * 100), rt: Math.round(p.rt * 100) });
      setMeasuring(false);
    }, 2000);
  }, []);

  const dz = drift ? Math.min(30, Math.max(drift.left, drift.right) + 2) : null;
  const tdz = drift && Math.max(drift.lt, drift.rt) > 2 ? Math.min(30, Math.max(drift.lt, drift.rt) + 2) : null;

  const Stick = ({ x, y, label }: { x: number; y: number; label: string }) => (
    <div className="flex flex-col items-center gap-2">
      <div className="relative h-28 w-28 rounded-full border border-line-2 bg-panel-2">
        <div className="absolute left-1/2 top-0 h-full w-px -translate-x-1/2 bg-line" />
        <div className="absolute left-0 top-1/2 h-px w-full -translate-y-1/2 bg-line" />
        <div
          className="absolute h-3.5 w-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-accent"
          style={{ left: `${50 + x * 46}%`, top: `${50 + y * 46}%` }}
        />
      </div>
      <span className="text-[12px] text-ink-3">{label}</span>
    </div>
  );

  return (
    <Card>
      <div className="flex items-start justify-between gap-4">
        <div>
          <h3 className="flex items-center gap-2 text-[15px] font-semibold">
            <Joystick className="h-4 w-4 text-accent" />
            {t("controller.sticksTitle")}
          </h3>
          <p className="mt-1 text-[13px] text-ink-2">{name ? t("controller.sticksDesc") : t("controller.sticksPress")}</p>
        </div>
        <Button size="sm" variant="primary" loading={measuring} disabled={!name} onClick={measure}>
          {measuring ? t("controller.driftMeasuring") : t("controller.driftButton")}
        </Button>
      </div>
      {name && (
        <>
          <div className="mt-1 text-[12px] text-ink-3">{name}</div>
          <div className="mt-4 flex flex-wrap items-center gap-10">
            <Stick x={axes[0]} y={axes[1]} label={t("controller.sticksLeft")} />
            <Stick x={axes[2]} y={axes[3]} label={t("controller.sticksRight")} />
            {drift && dz != null && (
              <div className="min-w-0 flex-1 space-y-2 text-[13px] leading-relaxed">
                <p className="text-ink">{t("controller.driftResult", { left: drift.left, right: drift.right })}</p>
                <p className="text-ink">{t("controller.driftAdvice", { dz })}</p>
                {Math.max(drift.left, drift.right) > 10 && <p className="text-warn">{t("controller.driftHigh")}</p>}
                <p className="text-ink-2">{t("controller.driftTriggers", { left: drift.lt, right: drift.rt })}</p>
                {tdz != null && <p className="text-ink">{t("controller.driftTriggerAdvice", { dz: tdz })}</p>}
              </div>
            )}
          </div>
        </>
      )}
    </Card>
  );
}

const XBOX_LABELS = ["A", "B", "X", "Y", "LB", "RB", "LT", "RT", "View", "Menu", "LS", "RS", "↑", "↓", "←", "→", "Xbox"];
const PS_LABELS = ["✕", "○", "□", "△", "L1", "R1", "L2", "R2", "Create", "Options", "L3", "R3", "↑", "↓", "←", "→", "PS", "Touchpad"];
const BUTTON_GROUPS = [[0, 1, 2, 3], [12, 13, 14, 15], [4, 5, 10, 11], [8, 9, 16, 17]];

/** Laat elke knop oplichten als je hem indrukt, en hoe ver de trekkers ingedrukt zijn (Gamepad API). */
function ButtonTester() {
  const { t } = useT();
  const [pad, setPad] = useState<{ id: string; pressed: boolean[]; lt: number; rt: number; count: number } | null>(null);

  useEffect(() => {
    let raf = 0;
    const loop = () => {
      const p = (navigator.getGamepads?.() ?? []).find((g) => g && g.connected) ?? null;
      if (p) {
        const pressed = p.buttons.map((b) => b.pressed);
        setPad({ id: p.id, pressed, lt: p.buttons[6]?.value ?? 0, rt: p.buttons[7]?.value ?? 0, count: p.buttons.length });
      } else {
        setPad(null);
      }
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, []);

  const ps = pad ? /054c|dualsense|dualshock|wireless controller/i.test(pad.id) : false;
  const labels = ps ? PS_LABELS : XBOX_LABELS;

  const Trigger = ({ label, value }: { label: string; value: number }) => (
    <div className="flex items-center gap-3">
      <span className="w-14 text-[12px] font-semibold text-ink-2">{label}</span>
      <div className="h-2.5 flex-1 overflow-hidden rounded-full bg-panel-2">
        <div className="h-full rounded-full bg-accent" style={{ width: `${Math.round(value * 100)}%` }} />
      </div>
      <span className="w-12 text-right text-[12px] tnum text-ink">{Math.round(value * 100)}%</span>
    </div>
  );

  return (
    <Card>
      <h3 className="flex items-center gap-2 text-[15px] font-semibold">
        <Zap className="h-4 w-4 text-accent" />
        {t("controller.buttonsTitle")}
      </h3>
      <p className="mt-1 text-[13px] text-ink-2">{pad ? t("controller.buttonsDesc") : t("controller.sticksPress")}</p>
      {pad && (
        <div className="mt-4 space-y-4">
          <div className="flex flex-wrap gap-4">
            {BUTTON_GROUPS.map((group, gi) => (
              <div key={gi} className="flex gap-1.5">
                {group
                  .filter((i) => i < pad.count && labels[i])
                  .map((i) => (
                    <span
                      key={i}
                      className={cn(
                        "flex h-9 min-w-9 items-center justify-center rounded-md border px-2 text-[13px] font-semibold transition-colors",
                        pad.pressed[i] ? "border-accent bg-accent text-white" : "border-line-2 bg-panel-2 text-ink-2",
                      )}
                    >
                      {labels[i]}
                    </span>
                  ))}
              </div>
            ))}
          </div>
          <div className="space-y-2">
            <Trigger label={labels[6]} value={pad.lt} />
            <Trigger label={labels[7]} value={pad.rt} />
          </div>
        </div>
      )}
    </Card>
  );
}

type CheckLevel = "ok" | "warn" | "info";

/** Wat je knopdrukken nog vertraagt, met live controles. */
function FastCheck({ list, rate, tweaks }: { list: ControllerInfo[]; rate: ControllerRate | null; tweaks: TweakInfo[] }) {
  const { t, td } = useT();
  const rows: { level: CheckLevel; text: string }[] = [];

  if (list.some((c) => c.wired === true)) rows.push({ level: "ok", text: t("controller.fastWired") });
  else if (list.some((c) => c.wired === false)) rows.push({ level: "warn", text: t("controller.fastWireless") });
  else rows.push({ level: "info", text: t("controller.fastNoController") });

  const hz = rate?.hz != null ? snap(rate.hz) : null;
  if (hz == null) rows.push({ level: "info", text: t("controller.fastRateUnknown") });
  else if (hz >= 250) rows.push({ level: "ok", text: t("controller.fastRateGood", { hz }) });
  else if (hz >= 100) rows.push({ level: "info", text: t("controller.fastRateOk", { hz }) });
  else rows.push({ level: "warn", text: t("controller.fastRateLow", { hz }) });

  for (const id of ["game_bar_dvr_off", "usb_suspend_off", "game_mode", "windowed_opt"]) {
    const tw = tweaks.find((x) => x.id === id);
    if (!tw || tw.state === "unknown" || tw.state === "unavailable") continue;
    const name = td(`tweak.${id}.name`);
    rows.push(tw.state === "on" ? { level: "ok", text: t("controller.fastTweakOn", { name }) } : { level: "warn", text: t("controller.fastTweakOff", { name }) });
  }
  rows.push({ level: "info", text: t("controller.fastFps") });

  const icon = { ok: CheckCircle2, warn: AlertTriangle, info: Info };
  const color = { ok: "text-ok", warn: "text-warn", info: "text-info" };

  return (
    <Card>
      <h3 className="flex items-center gap-2 text-[15px] font-semibold">
        <Zap className="h-4 w-4 text-accent" />
        {t("controller.fastTitle")}
      </h3>
      <p className="mt-1 text-[13px] text-ink-2">{t("controller.fastDesc")}</p>
      <ul className="mt-4 space-y-2.5 text-[13px] leading-relaxed">
        {rows.map((r, i) => {
          const Icon = icon[r.level];
          return (
            <li key={i} className="flex gap-2.5">
              <Icon className={cn("mt-0.5 h-4 w-4 shrink-0", color[r.level])} />
              <span className="text-ink">{r.text}</span>
            </li>
          );
        })}
      </ul>
    </Card>
  );
}

export function ControllerPage() {
  const { t, td } = useT();
  const tweaks = useApp((s) => s.tweaks);
  const tweaksLoading = useApp((s) => s.tweaksLoading);
  const setTweak = useApp((s) => s.setTweak);
  const appInfo = useApp((s) => s.appInfo);
  const toast = useApp((s) => s.toast);
  const handleError = useApp((s) => s.handleError);
  const [busy, setBusy] = useState<string | null>(null);
  const [list, setList] = useState<ControllerInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [lastRate, setLastRate] = useState<ControllerRate | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      setList(await api.listControllers());
    } catch (e) {
      handleError(e);
    } finally {
      setLoading(false);
    }
  }, [handleError]);

  useEffect(() => {
    void refresh();
    const onPad = () => void refresh();
    window.addEventListener("gamepadconnected", onPad);
    window.addEventListener("gamepaddisconnected", onPad);
    return () => {
      window.removeEventListener("gamepadconnected", onPad);
      window.removeEventListener("gamepaddisconnected", onPad);
    };
  }, [refresh]);

  const shown = CONTROLLER_TWEAKS.map((id) => tweaks.find((tw) => tw.id === id)).filter(Boolean) as TweakInfo[];

  const onToggle = async (tw: TweakInfo, enabled: boolean, option: string | null) => {
    setBusy(tw.id);
    const info = await setTweak(tw.id, enabled, option);
    setBusy(null);
    if (info) {
      const name = td(`tweak.${tw.id}.name`);
      toast({ level: "success", title: t(`timing.${tw.timing}`), message: enabled ? t("tweakCard.applied", { name }) : t("tweakCard.disabled", { name }) });
    }
  };

  const tips = [
    t("controller.tipWired"),
    t("controller.tipPlaystation"),
    t("controller.tipFirmware"),
    t("controller.tipPsSteam"),
    t("controller.tipSteam"),
    t("controller.tipDeadzone"),
    t("controller.tipTriggers"),
    t("controller.tipPaddles"),
    t("controller.tipPsApp"),
    t("controller.tipInputPage"),
  ];

  return (
    <div className="space-y-8">
      <PageHeader title={t("controller.title")} subtitle={t("controller.subtitle")} />

      <div className="flex items-start gap-3 rounded-lg border border-line bg-panel px-4 py-3 text-[13px] text-ink-2">
        <Info className="mt-0.5 h-4 w-4 shrink-0 text-info" />
        {t("controller.honest")}
      </div>

      <Devices list={list} loading={loading} onRefresh={() => void refresh()} />

      <FastCheck list={list} rate={lastRate} tweaks={tweaks} />

      <RateTester list={list} onResult={setLastRate} />

      <ButtonTester />

      <Sticks />

      <section>
        <SectionTitle sub={t("controller.settingsDesc")}>{t("controller.settingsTitle")}</SectionTitle>
        {tweaksLoading && shown.length === 0 ? (
          <Skeleton className="h-48" />
        ) : (
          <div className="grid gap-4 lg:grid-cols-2">
            {shown.map((tw) => (
              <TweakCard key={tw.id} tweak={tw} elevated={appInfo?.isElevated ?? false} busy={busy === tw.id} onToggle={(v, opt) => void onToggle(tw, v, opt)} />
            ))}
          </div>
        )}
      </section>

      <section>
        <SectionTitle>{t("controller.tipsTitle")}</SectionTitle>
        <Card>
          <ul className="space-y-2.5 text-[13px] leading-relaxed">
            {tips.map((tip, i) => (
              <li key={i} className="flex gap-2.5">
                <span className="mt-2 h-1.5 w-1.5 shrink-0 rounded-full bg-accent" />
                {tip}
              </li>
            ))}
          </ul>
          <div className="mt-4 flex flex-wrap gap-2">
            <Button size="sm" icon={ExternalLink} onClick={() => api.openUrl(XBOX_ACCESSORIES).catch(handleError)}>
              {t("controller.tipFirmwareButton")}
            </Button>
            <Button size="sm" icon={Bluetooth} onClick={() => api.openUrl("ms-settings:bluetooth").catch(handleError)}>
              {t("controller.bluetoothButton")}
            </Button>
          </div>
        </Card>
      </section>
    </div>
  );
}
