import { AlertTriangle, CheckCircle2, Cpu, Hammer, Info, MonitorPlay, Pickaxe, RefreshCw, Sparkles, Wifi, XCircle } from "lucide-react";
import { OverlaysCard } from "@/components/OverlaysCard";
import { useCallback, useEffect, useState } from "react";
import { Badge, Button, Card, EmptyState, PageHeader, SectionTitle, Skeleton, Toggle, cn } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import type { TKey } from "@/i18n";
import { api } from "@/lib/ipc";
import type { FortniteOverview, HwAccelInfo } from "@/lib/types";
import { FortniteIniTab, NowTab } from "@/pages/Games";
import { useApp } from "@/store/app";

const GAME_ID = "epic:fortnite";
const QUICK = ["vsync", "fps_limit", "reflex"];

function StatusIcon({ ok }: { ok: boolean }) {
  return ok ? <CheckCircle2 className="h-[18px] w-[18px] shrink-0 text-ok" /> : <XCircle className="h-[18px] w-[18px] shrink-0 text-warn" />;
}

export function FortnitePage() {
  const { t } = useT();
  const games = useApp((s) => s.games);
  const handleError = useApp((s) => s.handleError);
  const toast = useApp((s) => s.toast);
  const setPage = useApp((s) => s.setPage);
  const game = games.find((g) => g.id === GAME_ID) ?? null;

  const [ov, setOv] = useState<FortniteOverview | null>(null);
  const [hw, setHw] = useState<HwAccelInfo[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [o, h] = await Promise.all([api.getFortniteOverview(), api.listHwAccel()]);
      setOv(o);
      setHw(h);
    } catch (e) {
      handleError(e);
    }
  }, [handleError]);

  useEffect(() => {
    void load();
  }, [load]);

  const apply = async (ids: string[]) => {
    setBusy(ids.join(","));
    try {
      const before = ov?.checks.filter((c) => ids.includes(c.id) && !c.ok).length ?? 0;
      setOv(await api.applyFortniteRecommended(ids));
      toast({ level: "success", title: t("notice.success"), message: before > 0 ? t("fn.applied", { count: String(before) }) : t("fn.allGood") });
    } catch (e) {
      handleError(e);
    } finally {
      setBusy(null);
    }
  };

  if (!ov) return <Skeleton className="h-64" />;
  if (!game && !ov.iniExists) {
    return (
      <div className="space-y-6">
        <PageHeader title="Fortnite" subtitle={t("fn.subtitle")} />
        <EmptyState icon={Pickaxe} title={t("fn.notFound")} description={t("msg.ini_missing")} />
      </div>
    );
  }

  const quick = ov.checks.filter((c) => QUICK.includes(c.id));
  const quickTodo = quick.filter((c) => !c.ok).length;
  const render = ov.checks.find((c) => c.id === "render_mode");
  const locked = ov.running || !ov.iniExists;

  const currentText = (id: string, v: string | null) => {
    if (v == null) return t("fn.notSet");
    if (id === "vsync") return v.toLowerCase() === "true" ? t("fn.on") : t("fn.off");
    if (id === "fps_limit") return Number(v) === 0 ? t("fn.unlimited") : `${Math.round(Number(v))} FPS`;
    if (id === "reflex") return t(`fn.reflexValue.${v === "2" ? "2" : v === "1" ? "1" : "0"}` as TKey);
    return v;
  };
  const recText = (id: string, v: string) => (id === "fps_limit" ? (Number(v) === 0 ? t("fn.unlimited") : `${v} FPS`) : currentText(id, v));

  return (
    <div className="space-y-6">
      <PageHeader
        title="Fortnite"
        subtitle={t("fn.subtitle")}
        actions={
          <Button icon={RefreshCw} onClick={() => void load()}>
            {t("fn.refresh")}
          </Button>
        }
      />

      {ov.running && (
        <div className="flex items-start gap-3 rounded-xl border border-warn/40 bg-warn/10 px-4 py-3 text-[13px]">
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-warn" />
          <span>{t("fn.running")}</span>
        </div>
      )}

      {/* Videokaart en per-game tweaks */}
      <section>
        <SectionTitle sub={ov.gpuName ? t("fn.gpuSub", { gpu: ov.gpuName }) : undefined}>{t("fn.gpuTitle")}</SectionTitle>
        {game ? <NowTab game={game} extras={false} /> : <Card className="text-[13px] text-ink-3">{t("fn.notDetected")}</Card>}
      </section>

      {/* Aanbevolen instellingen in GameUserSettings.ini */}
      <section>
        <SectionTitle sub={t("fn.recSub")}>{t("fn.recTitle")}</SectionTitle>
        <Card>
          <ul className="divide-y divide-line">
            {quick.map((c) => (
              <li key={c.id} className="flex items-start gap-3 py-3 first:pt-0">
                <StatusIcon ok={c.ok} />
                <div className="min-w-0 flex-1">
                  <div className="text-sm font-medium">{t(`fn.check.${c.id}.name` as TKey)}</div>
                  <div className="mt-0.5 text-[13px] text-ink-2">{t(`fn.check.${c.id}.why` as TKey)}</div>
                </div>
                <div className="shrink-0 text-right text-[13px]">
                  <div className={cn(c.ok ? "text-ok" : "text-ink")}>{currentText(c.id, c.current)}</div>
                  {!c.ok && <div className="text-ink-3">{t("fn.advice", { value: recText(c.id, c.recommended) })}</div>}
                </div>
              </li>
            ))}
          </ul>
          <div className="mt-4 flex flex-wrap items-center gap-3 border-t border-line pt-4">
            {quickTodo === 0 ? (
              <Badge variant="ok" icon={CheckCircle2}>
                {t("fn.allGood")}
              </Badge>
            ) : (
              <Button variant="primary" icon={Sparkles} loading={busy === QUICK.join(",")} disabled={locked} onClick={() => void apply(QUICK)}>
                {t("fn.applyAll", { count: String(quickTodo) })}
              </Button>
            )}
            <span className="text-[12px] text-ink-3">{ov.running ? t("fn.closeFirst") : t("fn.backupNote")}</span>
          </div>
        </Card>
      </section>

      {/* Rendermodus */}
      {render && (
        <section>
          <SectionTitle>{t("fn.renderTitle")}</SectionTitle>
          <Card>
            <div className="flex items-start gap-3">
              <MonitorPlay className="mt-0.5 h-5 w-5 shrink-0 text-accent" />
              <div className="min-w-0 flex-1">
                <div className="text-sm font-medium">{t("fn.renderNow", { mode: t(`fn.mode.${ov.renderMode}` as TKey) })}</div>
                <p className="mt-1 text-[13px] text-ink-2">{t(`fn.renderAdvice.${ov.advice}` as TKey)}</p>
                <p className="mt-1 text-[13px] text-ink-3">{t("fn.renderDiff")}</p>
                <div className="mt-3">
                  {render.ok ? (
                    <Badge variant="ok" icon={CheckCircle2}>
                      {t("fn.renderOk")}
                    </Badge>
                  ) : (
                    <Button loading={busy === "render_mode"} disabled={locked} onClick={() => void apply(["render_mode"])}>
                      {t("fn.renderSet")}
                    </Button>
                  )}
                </div>
              </div>
            </div>
          </Card>
        </section>
      )}

      {/* Bouwen en editten: staat in je Epic-account */}
      <section>
        <SectionTitle sub={t("fn.buildSub")}>{t("fn.buildTitle")}</SectionTitle>
        <Card className="!py-1">
          <ul className="divide-y divide-line">
            {["turbo", "reset", "hold", "release"].map((id) => (
              <li key={id} className="flex items-start gap-3 py-3">
                <Hammer className="mt-0.5 h-4 w-4 shrink-0 text-accent" />
                <div className="min-w-0 flex-1">
                  <div className="text-sm font-medium">
                    {t(`fn.build.${id}.name` as TKey)} <span className="ml-1 text-accent">{t(`fn.build.${id}.value` as TKey)}</span>
                  </div>
                  <div className="mt-0.5 text-[13px] text-ink-2">{t(`fn.build.${id}.why` as TKey)}</div>
                </div>
              </li>
            ))}
          </ul>
        </Card>
      </section>

      <OverlaysCard />

      {/* Hardwareversnelling */}
      {hw && hw.length > 0 && (
        <section>
          <SectionTitle sub={t("fn.hwSub")}>{t("fn.hwTitle")}</SectionTitle>
          <Card className="!py-1">
            <ul className="divide-y divide-line">
              {hw.map((h) => (
                <li key={h.id} className="flex items-center gap-3 py-3">
                  <Cpu className={cn("h-4 w-4 shrink-0", h.enabled ? "text-warn" : "text-ok")} />
                  <div className="min-w-0 flex-1">
                    <div className="text-sm font-medium">{h.name}</div>
                    <div className="mt-0.5 text-[13px] text-ink-2">
                      {h.running ? t("fn.hwRunning", { app: h.name }) : h.enabled ? t("fn.hwOn") : t("fn.hwOff")}
                    </div>
                  </div>
                  <Toggle
                    checked={!h.enabled}
                    disabled={h.running}
                    loading={busy === `hw:${h.id}`}
                    label={t("fn.hwToggle", { app: h.name })}
                    onChange={async (v) => {
                      setBusy(`hw:${h.id}`);
                      try {
                        setHw(await api.setHwAccel(h.id, !v));
                      } catch (e) {
                        handleError(e);
                      } finally {
                        setBusy(null);
                      }
                    }}
                  />
                </li>
              ))}
            </ul>
          </Card>
        </section>
      )}

      {/* Tips: shaders en ping */}
      <section>
        <SectionTitle>{t("fn.tipsTitle")}</SectionTitle>
        <div className="grid gap-4 lg:grid-cols-2">
          <Card>
            <div className="flex items-start gap-3">
              <Info className="mt-0.5 h-5 w-5 shrink-0 text-accent" />
              <div>
                <div className="text-sm font-semibold">{t("fn.shaderTitle")}</div>
                <p className="mt-1 text-[13px] text-ink-2">{t("fn.shaderText")}</p>
              </div>
            </div>
          </Card>
          <Card>
            <div className="flex items-start gap-3">
              <Wifi className="mt-0.5 h-5 w-5 shrink-0 text-accent" />
              <div>
                <div className="text-sm font-semibold">{t("fn.pingTitle")}</div>
                <p className="mt-1 text-[13px] text-ink-2">{t("fn.pingText")}</p>
                <Button size="sm" className="mt-3" onClick={() => setPage("network")}>
                  {t("fn.pingOpen")}
                </Button>
              </div>
            </div>
          </Card>
        </div>
      </section>

      {/* Zelf aanpassen (bestaande editor met back-ups) */}
      <section>
        <SectionTitle sub={t("fn.advancedSub")}>{t("fn.advancedTitle")}</SectionTitle>
        <Card>
          <FortniteIniTab />
        </Card>
      </section>
    </div>
  );
}
