import { AlertTriangle, Crosshair, Info, RotateCcw, Save, Trash2 } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { CrosshairShape } from "@/components/CrosshairShape";
import { Button, Card, Chip, PageHeader, Row, SectionTitle, TextInput, Toggle } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import type { TKey } from "@/i18n";
import { api } from "@/lib/ipc";
import type { CrosshairConfig } from "@/lib/types";
import { useApp } from "@/store/app";

const DEFAULT: CrosshairConfig = {
  shape: "cross", color: "#00ff7f", size: 8, thickness: 2, gap: 4, outline: true, outlineColor: "#000000", outlineThickness: 1, opacity: 1, dot: false,
  dotSize: 3, rotation: 0, offsetX: 0, offsetY: 0, showTop: true, showRight: true, showBottom: true, showLeft: true,
};
const COLORS = ["#00ff7f", "#00e5ff", "#ffff00", "#ff2d55", "#ff00ff", "#ff9500", "#ffffff", "#000000"];
const HOTKEYS = ["ctrl+alt+c", "ctrl+shift+x", "alt+f9", "f8", "f10"];
const SHAPES: CrosshairConfig["shape"][] = ["cross", "cross_dot", "dot", "circle", "circle_dot", "t", "x", "square"];
const ARM_SHAPES: CrosshairConfig["shape"][] = ["cross", "cross_dot", "t", "x"];

/** Ingebouwde startpunten. Je past ze daarna zelf aan en bewaart ze eventueel als eigen preset. */
const BUILT_IN: { id: string; label: TKey; patch: Partial<CrosshairConfig> }[] = [
  { id: "classic", label: "crosshair.builtInNames.classic", patch: {} },
  { id: "tight", label: "crosshair.builtInNames.tight", patch: { size: 5, gap: 2, thickness: 2, color: "#00e5ff" } },
  { id: "dot", label: "crosshair.builtInNames.dot", patch: { shape: "dot", dotSize: 4, color: "#ff2d55" } },
  { id: "ring", label: "crosshair.builtInNames.ring", patch: { shape: "circle_dot", size: 6, gap: 4, thickness: 2, dotSize: 2, color: "#ffff00" } },
  { id: "x", label: "crosshair.builtInNames.x", patch: { shape: "x", size: 8, gap: 3, color: "#ff00ff" } },
];

function Slider({ label, value, min, max, step = 1, onChange, suffix = "px" }: { label: string; value: number; min: number; max: number; step?: number; onChange: (v: number) => void; suffix?: string }) {
  return (
    <div className="grid grid-cols-[130px_1fr_56px] items-center gap-3 py-2">
      <span className="text-[13px] text-ink-2">{label}</span>
      <input type="range" min={min} max={max} step={step} value={value} onChange={(e) => onChange(Number(e.target.value))} className="w-full" aria-label={label} />
      <span className="text-right text-[13px] tnum text-ink">
        {suffix === "%" ? Math.round(value * 100) : value}
        {suffix}
      </span>
    </div>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="grid grid-cols-[130px_1fr] items-center gap-3 py-2">
      <span className="text-[13px] text-ink-2">{label}</span>
      <div className="flex flex-wrap items-center gap-2">{children}</div>
    </div>
  );
}

function Group({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="border-t border-line py-3 first:border-t-0 first:pt-0">
      <div className="mb-1 text-[11px] font-semibold uppercase tracking-wide text-ink-3">{title}</div>
      {children}
    </div>
  );
}

function ColorPick({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  return (
    <Field label={label}>
      {COLORS.map((c) => (
        <button
          key={c}
          onClick={() => onChange(c)}
          title={c}
          aria-label={c}
          className="h-6 w-6 rounded-md border-2"
          style={{ background: c, borderColor: value.toLowerCase() === c ? "white" : "var(--color-line)" }}
        />
      ))}
      <input type="color" value={value} onChange={(e) => onChange(e.target.value)} className="h-7 w-10 cursor-pointer rounded border border-line bg-transparent" aria-label={label} />
      <span className="text-[12px] uppercase tnum text-ink-3">{value}</span>
    </Field>
  );
}

export function CrosshairPage() {
  const { t } = useT();
  const settings = useApp((s) => s.settings);
  const updateSettings = useApp((s) => s.updateSettings);
  const handleError = useApp((s) => s.handleError);
  const toast = useApp((s) => s.toast);
  const [cfg, setCfg] = useState<CrosshairConfig>(settings?.crosshair ?? DEFAULT);
  const [presetName, setPresetName] = useState("");
  const [toggling, setToggling] = useState(false);

  // Wijzigingen na een korte pauze opslaan; de overlay past zich dan direct aan.
  useEffect(() => {
    if (!settings || JSON.stringify(settings.crosshair) === JSON.stringify(cfg)) return;
    const id = setTimeout(() => void updateSettings({ crosshair: cfg }), 200);
    return () => clearTimeout(id);
  }, [cfg, settings, updateSettings]);

  if (!settings) return null;
  const set = (patch: Partial<CrosshairConfig>) => setCfg((c) => ({ ...c, ...patch }));

  const setEnabled = async (v: boolean) => {
    setToggling(true);
    try {
      await api.setCrosshairEnabled(v);
      useApp.setState({ settings: { ...settings, crosshairEnabled: v } });
    } catch (e) {
      handleError(e);
    } finally {
      setToggling(false);
    }
  };

  const savePreset = () => {
    const name = presetName.trim();
    if (!name) return;
    const presets = [...settings.crosshairPresets.filter((p) => p.name !== name), { name, config: cfg }];
    void updateSettings({ crosshairPresets: presets });
    setPresetName("");
    toast({ level: "success", title: t("notice.success"), message: t("crosshair.presetSaved", { name }) });
  };

  const hasArms = ARM_SHAPES.includes(cfg.shape);
  const hasSize = cfg.shape !== "dot";
  const hasDotSize = cfg.shape === "dot" || cfg.shape === "cross_dot" || cfg.shape === "circle_dot" || cfg.dot;
  const dotBuiltIn = cfg.shape === "cross_dot" || cfg.shape === "circle_dot";
  const arm = (key: "showTop" | "showRight" | "showBottom" | "showLeft", label: string) => (
    <Chip active={cfg[key]} onClick={() => set({ [key]: !cfg[key] })}>
      {label}
    </Chip>
  );
  const controls: ReactNode = (
    <>
      <Group title={t("crosshair.groupShape")}>
        <Field label={t("crosshair.shape")}>
          {SHAPES.map((sh) => (
            <Chip key={sh} active={cfg.shape === sh} onClick={() => set({ shape: sh })}>
              {t(`crosshair.shapes.${sh}`)}
            </Chip>
          ))}
        </Field>
        {hasArms && cfg.shape !== "t" && (
          <Field label={t("crosshair.arms")}>
            {arm("showTop", t("crosshair.armTop"))}
            {arm("showRight", t("crosshair.armRight"))}
            {arm("showBottom", t("crosshair.armBottom"))}
            {arm("showLeft", t("crosshair.armLeft"))}
          </Field>
        )}
        {cfg.shape !== "t" && cfg.shape !== "dot" && !dotBuiltIn && (
          <Field label={t("crosshair.centerDot")}>
            <Toggle checked={cfg.dot} onChange={(v) => set({ dot: v })} label={t("crosshair.centerDot")} />
          </Field>
        )}
      </Group>
      <Group title={t("crosshair.groupSize")}>
        {hasSize && <Slider label={t("crosshair.size")} value={cfg.size} min={1} max={40} onChange={(v) => set({ size: v })} />}
        {cfg.shape !== "dot" && <Slider label={t("crosshair.thickness")} value={cfg.thickness} min={1} max={8} onChange={(v) => set({ thickness: v })} />}
        {hasSize && <Slider label={t("crosshair.gap")} value={cfg.gap} min={0} max={20} onChange={(v) => set({ gap: v })} />}
        {hasDotSize && <Slider label={t("crosshair.dotSize")} value={cfg.dotSize} min={1} max={12} onChange={(v) => set({ dotSize: v })} />}
        {(hasArms || cfg.shape === "square") && <Slider label={t("crosshair.rotation")} value={cfg.rotation} min={0} max={90} suffix="°" onChange={(v) => set({ rotation: v })} />}
      </Group>
      <Group title={t("crosshair.groupColor")}>
        <ColorPick label={t("crosshair.color")} value={cfg.color} onChange={(v) => set({ color: v })} />
        <Slider label={t("crosshair.opacity")} value={cfg.opacity} min={0.1} max={1} step={0.05} suffix="%" onChange={(v) => set({ opacity: v })} />
      </Group>
      <Group title={t("crosshair.groupOutline")}>
        <Field label={t("crosshair.outline")}>
          <Toggle checked={cfg.outline} onChange={(v) => set({ outline: v })} label={t("crosshair.outline")} />
        </Field>
        {cfg.outline && (
          <>
            <ColorPick label={t("crosshair.outlineColor")} value={cfg.outlineColor} onChange={(v) => set({ outlineColor: v })} />
            <Slider label={t("crosshair.outlineThickness")} value={cfg.outlineThickness} min={1} max={3} onChange={(v) => set({ outlineThickness: v })} />
          </>
        )}
      </Group>
      <Group title={t("crosshair.groupPosition")}>
        <Slider label={t("crosshair.offsetX")} value={cfg.offsetX} min={-100} max={100} onChange={(v) => set({ offsetX: v })} />
        <Slider label={t("crosshair.offsetY")} value={cfg.offsetY} min={-100} max={100} onChange={(v) => set({ offsetY: v })} />
        {(cfg.offsetX !== 0 || cfg.offsetY !== 0) && (
          <div className="pt-1">
            <Button size="sm" variant="ghost" icon={Crosshair} onClick={() => set({ offsetX: 0, offsetY: 0 })}>
              {t("crosshair.center")}
            </Button>
          </div>
        )}
      </Group>
    </>
  );

  return (
    <div className="space-y-6">
      <PageHeader title={t("crosshair.title")} subtitle={t("crosshair.subtitle")} />

      <Card className="!py-1">
        <Row title={t("crosshair.enable")} desc={t("crosshair.fullscreenNote")}>
          <Toggle checked={settings.crosshairEnabled} loading={toggling} onChange={(v) => void setEnabled(v)} label={t("crosshair.enable")} />
        </Row>
        <div className="border-t border-line" />
        <Row title={t("crosshair.hotkey")} desc={t("crosshair.hotkeyHint")}>
          <select
            value={settings.crosshairHotkey}
            onChange={async (e) => {
              const ok = await updateSettings({ crosshairHotkey: e.target.value });
              if (ok) toast({ level: "success", title: t("notice.success"), message: t("crosshair.hotkeySaved", { hotkey: e.target.value.toUpperCase() }) });
            }}
            className="h-9 rounded-lg border border-line bg-panel-2 px-3 text-sm text-ink focus:border-accent focus:outline-none"
          >
            {HOTKEYS.map((h) => (
              <option key={h} value={h}>
                {h.toUpperCase().replace(/\+/g, " + ")}
              </option>
            ))}
          </select>
        </Row>
      </Card>

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_360px]">
        <Card>{controls}</Card>
        <div className="space-y-4">
          <Card>
            <div className="mb-3 text-sm font-semibold">{t("crosshair.preview")}</div>
            {/* Voorbeeld op een donker en een licht vlak, zodat je ziet hoe hij in beide gevallen opvalt */}
            <div className="grid grid-cols-2 overflow-hidden rounded-lg border border-line">
              <div className="flex aspect-square items-center justify-center" style={{ background: "linear-gradient(160deg,#5b7a99,#2f4a2f)" }}>
                <CrosshairShape cfg={cfg} box={140} />
              </div>
              <div className="flex aspect-square items-center justify-center" style={{ background: "linear-gradient(160deg,#e9e3d3,#b9c8d9)" }}>
                <CrosshairShape cfg={cfg} box={140} />
              </div>
            </div>
            <Button size="sm" variant="ghost" icon={RotateCcw} className="mt-3" onClick={() => setCfg(DEFAULT)}>
              {t("crosshair.reset")}
            </Button>
          </Card>
          <Card>
            <SectionTitle>{t("crosshair.builtIn")}</SectionTitle>
            <div className="grid grid-cols-5 gap-2">
              {BUILT_IN.map((b) => {
                const preset = { ...DEFAULT, ...b.patch };
                return (
                  <button
                    key={b.id}
                    onClick={() => setCfg(preset)}
                    title={t(b.label)}
                    className="flex flex-col items-center gap-1 rounded-lg border border-line bg-panel-2 p-1.5 text-[11px] text-ink-2 hover:border-line-2 hover:text-ink"
                  >
                    <div className="h-10 w-10 overflow-hidden rounded-md" style={{ background: "linear-gradient(160deg,#5b7a99,#2f4a2f)" }}>
                      <CrosshairShape cfg={preset} box={40} scale={0.8} />
                    </div>
                    <span className="max-w-full truncate">{t(b.label)}</span>
                  </button>
                );
              })}
            </div>
          </Card>
          <Card>
            <SectionTitle>{t("crosshair.presets")}</SectionTitle>
            <div className="flex gap-2">
              <TextInput value={presetName} onChange={setPresetName} placeholder={t("crosshair.presetName")} />
              <Button icon={Save} disabled={!presetName.trim()} onClick={savePreset}>
                {t("crosshair.savePreset")}
              </Button>
            </div>
            <ul className="mt-3 divide-y divide-line">
              {settings.crosshairPresets.map((p) => (
                <li key={p.name} className="flex items-center gap-3 py-2">
                  <div className="h-10 w-10 shrink-0 overflow-hidden rounded-md bg-[#3c5266]">
                    <CrosshairShape cfg={p.config} box={40} scale={0.8} />
                  </div>
                  <span className="min-w-0 flex-1 truncate text-sm">{p.name}</span>
                  <Button size="sm" onClick={() => setCfg(p.config)}>
                    {t("crosshair.load")}
                  </Button>
                  <button
                    className="rounded p-1.5 text-ink-3 hover:text-bad"
                    aria-label={t("common.remove")}
                    onClick={() => void updateSettings({ crosshairPresets: settings.crosshairPresets.filter((x) => x.name !== p.name) })}
                  >
                    <Trash2 className="h-4 w-4" />
                  </button>
                </li>
              ))}
            </ul>
          </Card>
        </div>
      </div>

      <div className="space-y-2 text-[13px] text-ink-3">
        <p className="flex items-start gap-2">
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-warn" />
          {t("crosshair.fullscreenNote")}
        </p>
        <p className="flex items-start gap-2">
          <Info className="mt-0.5 h-4 w-4 shrink-0" />
          {t("crosshair.rulesNote")}
        </p>
      </div>
    </div>
  );
}
