import { FolderOpen, Gamepad2, Save, Undo2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { GameConfigCard } from "@/components/GameConfigCard";
import { Loader } from "@/components/Loader";
import { GainBadge, StateBadge, TimingBadge } from "@/components/TweakCard";
import { Badge, Button, Chip, EmptyState, Row, SegmentedControl, Toggle } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import { api } from "@/lib/ipc";
import { prettyPath } from "@/lib/privacy";
import type { FortniteSettings, Game, TweakInfo } from "@/lib/types";
import { useApp } from "@/store/app";

/* ---------- Tweaks voor een game ---------- */

export function NowTab({ game }: { game: Game }) {
  const { t, td } = useT();
  const [list, setList] = useState<TweakInfo[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const setTweak = useApp((s) => s.setTweak);
  const handleError = useApp((s) => s.handleError);
  const load = useCallback(() => {
    api.getGameTweaks(game.id).then(setList).catch(handleError);
  }, [game.id, handleError]);
  useEffect(() => {
    load();
  }, [load]);

  if (!list) return <Loader />;
  return (
    <div className="space-y-3">
      <p className="text-[13px] text-ink-3">{t("games.nowHint")}</p>
      <div className="card-inset divide-y divide-line px-4">
        {list.map((tw) => (
          <div key={tw.id} className="grid grid-cols-[1fr_auto] items-center gap-6 py-3.5">
            <div className="min-w-0">
              <div className="text-sm font-medium">{td(`tweak.${tw.id}.name`)}</div>
              <div className="mt-0.5 text-[13px] text-ink-2">{td(`tweak.${tw.id}.what`)}</div>
              <div className="mt-2 flex flex-wrap gap-1.5">
                <TimingBadge timing={tw.timing} />
                <GainBadge gain={tw.gain} />
                <StateBadge tweak={tw} />
              </div>
              {tw.detail && <div className="mt-1.5 text-[12px] text-ink-3">{td(`msg.${tw.detail.key}`, tw.detail.params)}</div>}
            </div>
            <Toggle
              checked={tw.state === "on"}
              disabled={tw.state === "unavailable"}
              loading={busy === tw.id}
              label={td(`tweak.${tw.id}.name`)}
              onChange={async (v) => {
                setBusy(tw.id);
                await setTweak(tw.id, v, null, game.id);
                setBusy(null);
                load();
              }}
            />
          </div>
        ))}
      </div>
      <GameConfigCard gameId={game.id} />
    </div>
  );
}

/* ---------- Fortnite ---------- */

const fpsPresets = [60, 120, 144, 165, 240, 360];

export function FortniteIniTab() {
  const { t } = useT();
  const [data, setData] = useState<FortniteSettings | null>(null);
  const [draft, setDraft] = useState<FortniteSettings | null>(null);
  const [saving, setSaving] = useState(false);
  const toast = useApp((s) => s.toast);
  const handleError = useApp((s) => s.handleError);
  const refreshJournal = useApp((s) => s.refreshJournal);

  useEffect(() => {
    api
      .getFortniteSettings()
      .then((d) => {
        setData(d);
        setDraft(d);
      })
      .catch(handleError);
  }, [handleError]);

  if (!data || !draft) return <Loader />;
  if (!data.exists) return <EmptyState icon={Gamepad2} title={t("fortnite.missingTitle")} description={t("msg.ini_missing")} />;
  const dirty = JSON.stringify(draft) !== JSON.stringify(data);

  const save = async () => {
    setSaving(true);
    try {
      const d = await api.saveFortniteSettings(draft);
      setData(d);
      setDraft(d);
      toast({ level: "success", title: t("notice.success"), message: t("fortnite.saved") });
      void refreshJournal();
    } catch (e) {
      handleError(e);
    } finally {
      setSaving(false);
    }
  };
  const restore = async (b: string) => {
    try {
      const d = await api.restoreFortniteBackup(b);
      setData(d);
      setDraft(d);
      toast({ level: "success", title: t("notice.success"), message: t("fortnite.restored") });
      void refreshJournal();
    } catch (e) {
      handleError(e);
    }
  };
  const num = "h-9 w-24 rounded-lg border border-line bg-panel-2 px-2 text-sm text-ink focus:border-accent focus:outline-none";

  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-center gap-2 text-[12px] text-ink-3">
        <FolderOpen className="h-3.5 w-3.5" />
        <span className="selectable truncate">{prettyPath(data.iniPath)}</span>
        {data.readOnly && <Badge variant="neutral">{t("fortnite.readOnly")}</Badge>}
      </div>
      <div className="card-inset divide-y divide-line px-4">
        <div className="py-3.5">
          <div className="mb-2 text-sm font-medium">{t("fortnite.fpsLimit")}</div>
          <div className="flex flex-wrap items-center gap-1.5">
            {fpsPresets.map((f) => (
              <Chip key={f} active={draft.frameRateLimit === f} onClick={() => setDraft({ ...draft, frameRateLimit: f })}>
                {f}
              </Chip>
            ))}
            <Chip active={draft.frameRateLimit === 0} onClick={() => setDraft({ ...draft, frameRateLimit: 0 })}>
              {t("fortnite.unlimited")}
            </Chip>
            <span className="ml-2 text-[12px] text-ink-3">{t("fortnite.custom")}</span>
            <input type="number" min={0} max={1000} value={draft.frameRateLimit ?? ""} onChange={(e) => setDraft({ ...draft, frameRateLimit: e.target.value === "" ? null : Number(e.target.value) })} className={num} />
          </div>
        </div>
        <Row title={t("fortnite.vsync")} desc={t("fortnite.vsyncHint")}>
          <Toggle checked={draft.vsync ?? false} onChange={(v) => setDraft({ ...draft, vsync: v })} label={t("fortnite.vsync")} />
        </Row>
        <Row title={t("fortnite.windowMode")}>
          <SegmentedControl
            options={[
              { value: "0", label: t("fortnite.mode0") },
              { value: "1", label: t("fortnite.mode1") },
              { value: "2", label: t("fortnite.mode2") },
            ]}
            value={String(draft.fullscreenMode ?? 0)}
            onChange={(v) => setDraft({ ...draft, fullscreenMode: Number(v) })}
          />
        </Row>
        <Row title={t("fortnite.resolution")}>
          <input type="number" value={draft.resX ?? ""} onChange={(e) => setDraft({ ...draft, resX: e.target.value === "" ? null : Number(e.target.value) })} className={num} />
          <span className="text-ink-3">×</span>
          <input type="number" value={draft.resY ?? ""} onChange={(e) => setDraft({ ...draft, resY: e.target.value === "" ? null : Number(e.target.value) })} className={num} />
        </Row>
      </div>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="max-w-md text-[12px] text-ink-3">{t("fortnite.saveHint")}</p>
        <Button variant="primary" icon={Save} loading={saving} disabled={!dirty} onClick={() => void save()}>
          {t("fortnite.save")}
        </Button>
      </div>
      {data.backups.length > 0 && (
        <div>
          <div className="mb-1.5 text-sm font-semibold">{t("fortnite.backups")}</div>
          <ul className="card-inset divide-y divide-line">
            {data.backups.slice(0, 6).map((b) => (
              <li key={b} className="flex items-center justify-between gap-3 px-3 py-2 text-[12px]">
                <span className="selectable truncate font-mono text-ink-2">{b}</span>
                <Button size="sm" variant="ghost" icon={Undo2} onClick={() => void restore(b)}>
                  {t("fortnite.restore")}
                </Button>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
