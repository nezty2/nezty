import { Check, ImagePlus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useT } from "@/i18n/hooks";
import type { TKey } from "@/i18n";
import { api, fileUrl, isTauri, pickImage } from "@/lib/ipc";
import type { Accent, Settings } from "@/lib/types";
import { useApp } from "@/store/app";
import { Button, Row, cn } from "./ui";

const ACCENTS: { id: Accent; color: string }[] = [
  { id: "violet", color: "#7c6cff" },
  { id: "cyan", color: "#22b8d6" },
  { id: "emerald", color: "#2fb67c" },
  { id: "rose", color: "#e5577a" },
  { id: "pink", color: "#f472b6" },
  { id: "amber", color: "#d99a1e" },
];

/** Voorbeeldkleuren per thema (achtergrond, kaart, lijn). "tinted" kleurt mee met je accent. */
const THEMES: { id: Settings["theme"]; bg: string; panel: string }[] = [
  { id: "midnight", bg: "#0f1115", panel: "#1c2029" },
  { id: "oled", bg: "#000000", panel: "#111114" },
  { id: "slate", bg: "#141821", panel: "#222836" },
  { id: "tinted", bg: "color-mix(in oklab, var(--accent) 10%, #0b0d11)", panel: "color-mix(in oklab, var(--accent) 22%, #171a22)" },
];

function Slider({ value, min, max, onChange, label, suffix }: { value: number; min: number; max: number; onChange: (v: number) => void; label: string; suffix: string }) {
  return (
    <label className="flex items-center gap-3 text-[12px] text-ink-3">
      <span className="w-20 shrink-0">{label}</span>
      <input type="range" min={min} max={max} value={value} onChange={(e) => onChange(Number(e.target.value))} className="w-40" aria-label={label} />
      <span className="w-12 text-right tnum text-ink-2">
        {value}
        {suffix}
      </span>
    </label>
  );
}

/** Thema, accentkleur (vast of eigen kleur) en eigen achtergrond. */
export function AppearanceRows({ settings }: { settings: Settings }) {
  const { t } = useT();
  const updateSettings = useApp((s) => s.updateSettings);
  const handleError = useApp((s) => s.handleError);
  const [preview, setPreview] = useState<string | null>(null);
  const [dim, setDim] = useState(settings.backgroundDim);
  const [blur, setBlur] = useState(settings.backgroundBlur);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!settings.backgroundImage) return setPreview(null);
    api.getBackgroundFile().then((p) => (p ? fileUrl(p) : null)).then(setPreview).catch(() => setPreview(null));
  }, [settings.backgroundImage]);

  // Schuifjes pas na een korte pauze opslaan.
  useEffect(() => {
    if (dim === settings.backgroundDim && blur === settings.backgroundBlur) return;
    const id = setTimeout(() => void updateSettings({ backgroundDim: dim, backgroundBlur: blur }), 250);
    return () => clearTimeout(id);
  }, [dim, blur, settings.backgroundDim, settings.backgroundBlur, updateSettings]);

  const setBackground = async (path: string | null) => {
    setBusy(true);
    try {
      const s = await api.setBackground(path);
      useApp.setState({ settings: s });
    } catch (e) {
      handleError(e);
    } finally {
      setBusy(false);
    }
  };

  const custom = settings.accentCustom;
  return (
    <>
      <Row title={t("settings.theme")}>
        <div className="grid grid-cols-2 gap-1.5">
          {THEMES.map((th) => (
            <button
              key={th.id}
              onClick={() => void updateSettings({ theme: th.id })}
              className={cn(
                "flex items-center gap-2 rounded-lg border px-2.5 py-1.5 text-[12px] transition-colors",
                settings.theme === th.id ? "border-accent text-ink" : "border-line text-ink-2 hover:border-line-2",
              )}
            >
              <span className="relative h-5 w-7 overflow-hidden rounded border border-line-2" style={{ background: th.bg }}>
                <span className="absolute bottom-0.5 left-0.5 h-2 w-4 rounded-sm" style={{ background: th.panel }} />
              </span>
              {t(`settings.themes.${th.id}` as TKey)}
            </button>
          ))}
        </div>
      </Row>
      <Row title={t("settings.accent")} desc={t("settings.accentDesc")}>
        <div className="flex flex-wrap items-center gap-2">
          {ACCENTS.map((a) => {
            const active = !custom && settings.accent === a.id;
            return (
              <button
                key={a.id}
                aria-label={a.id}
                onClick={() => void updateSettings({ accent: a.id, accentCustom: null })}
                className={cn("flex h-7 w-7 items-center justify-center rounded-full border-2 transition-transform", active ? "scale-110 border-white" : "border-transparent hover:scale-105")}
                style={{ background: a.color }}
              >
                {active && <Check className="h-3.5 w-3.5 text-white" />}
              </button>
            );
          })}
          <label
            className={cn("relative flex h-7 cursor-pointer items-center gap-1.5 rounded-full border-2 pl-1 pr-2.5 text-[12px] transition-transform hover:scale-105", custom ? "border-white text-ink" : "border-line-2 text-ink-2")}
            title={t("settings.accentCustom")}
          >
            <span className="h-5 w-5 rounded-full" style={{ background: custom ?? "conic-gradient(#f43f5e, #f59e0b, #84cc16, #22d3ee, #8b5cf6, #f43f5e)" }} />
            {custom ? custom.toUpperCase() : t("settings.accentCustom")}
            <input type="color" value={custom ?? "#7c6cff"} onChange={(e) => void updateSettings({ accentCustom: e.target.value })} className="absolute inset-0 cursor-pointer opacity-0" aria-label={t("settings.accentCustom")} />
          </label>
        </div>
      </Row>
      <Row title={t("settings.background")} desc={t("settings.backgroundDesc")}>
        <div className="flex flex-col items-end gap-2">
          <div className="flex items-center gap-2">
            {preview && <img src={preview} alt="" className="h-9 w-14 rounded-md border border-line object-cover" />}
            <Button
              size="sm"
              icon={ImagePlus}
              loading={busy}
              disabled={!isTauri}
              onClick={async () => {
                const p = await pickImage(t("settings.backgroundPick"));
                if (p) await setBackground(p);
              }}
            >
              {t("settings.backgroundPick")}
            </Button>
            {settings.backgroundImage && <Button size="sm" variant="ghost" icon={Trash2} onClick={() => void setBackground(null)} aria-label={t("settings.backgroundRemove")} />}
          </div>
          {settings.backgroundImage && (
            <>
              <Slider label={t("settings.backgroundDim")} value={dim} min={0} max={90} suffix="%" onChange={setDim} />
              <Slider label={t("settings.backgroundBlur")} value={blur} min={0} max={30} suffix=" px" onChange={setBlur} />
            </>
          )}
        </div>
      </Row>
    </>
  );
}
