import { AlertTriangle, CheckCircle2, Crosshair, Download, Info, Joystick, Play, RefreshCw, XCircle, type LucideIcon } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { MeasureCard } from "@/components/MeasureCard";
import { Button, Card, PageHeader, SectionTitle, cn } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import type { TKey } from "@/i18n";
import { api } from "@/lib/ipc";
import type { TestCheck, TestStatus, TweakTestStep } from "@/lib/types";
import { useApp } from "@/store/app";
import { useUpdate } from "@/store/update";

const ICON: Record<TestStatus, { icon: LucideIcon; color: string }> = {
  ok: { icon: CheckCircle2, color: "text-ok" },
  warn: { icon: AlertTriangle, color: "text-warn" },
  fail: { icon: XCircle, color: "text-bad" },
  info: { icon: Info, color: "text-ink-3" },
};

function StatusIcon({ status }: { status: TestStatus }) {
  const { icon: Icon, color } = ICON[status];
  return <Icon className={cn("h-[18px] w-[18px] shrink-0", color)} />;
}

export function TestPage() {
  const { t } = useT();
  const handleError = useApp((s) => s.handleError);
  const setPage = useApp((s) => s.setPage);
  const settings = useApp((s) => s.settings);
  const updateStatus = useUpdate((s) => s.status);
  const updateInfo = useUpdate((s) => s.info);
  const checkUpdate = useUpdate((s) => s.check);

  const [checks, setChecks] = useState<TestCheck[] | null>(null);
  const [running, setRunning] = useState(false);
  const [steps, setSteps] = useState<TweakTestStep[] | null>(null);
  const [tweakRunning, setTweakRunning] = useState(false);
  const [crosshairLeft, setCrosshairLeft] = useState(0);
  const crosshairTimer = useRef<number | null>(null);

  const run = useCallback(async () => {
    setRunning(true);
    try {
      setChecks(await api.runCheckup());
    } catch (e) {
      handleError(e);
    } finally {
      setRunning(false);
    }
  }, [handleError]);

  useEffect(() => {
    void run();
  }, [run]);

  useEffect(() => () => {
    if (crosshairTimer.current) clearInterval(crosshairTimer.current);
  }, []);

  const runTweakTest = async () => {
    setTweakRunning(true);
    setSteps(null);
    try {
      setSteps(await api.runTweakTest());
    } catch (e) {
      handleError(e);
    } finally {
      setTweakRunning(false);
    }
  };

  // Crosshair 5 seconden tonen. Stond hij al aan, dan blijft hij aan.
  const testCrosshair = async () => {
    if (!settings) return;
    const wasOn = settings.crosshairEnabled;
    try {
      if (!wasOn) await api.setCrosshairEnabled(true);
    } catch (e) {
      handleError(e);
      return;
    }
    let left = 5;
    setCrosshairLeft(left);
    crosshairTimer.current = window.setInterval(() => {
      left -= 1;
      setCrosshairLeft(left);
      if (left <= 0) {
        if (crosshairTimer.current) clearInterval(crosshairTimer.current);
        crosshairTimer.current = null;
        if (!wasOn) void api.setCrosshairEnabled(false).catch(handleError);
      }
    }, 1000);
  };

  const okCount = checks?.filter((c) => c.status === "ok" || c.status === "info").length ?? 0;
  const problems = checks?.filter((c) => c.status === "fail" || c.status === "warn").length ?? 0;

  const updateText =
    updateStatus === "checking"
      ? t("update.checking")
      : updateStatus === "available" && updateInfo
        ? t("update.available", { version: updateInfo.version })
        : updateStatus === "none"
          ? t("update.latest")
          : updateStatus === "error"
            ? t("update.error")
            : updateStatus === "unconfigured"
              ? t("update.notConfigured")
              : t("test.updateIdle");

  return (
    <div className="space-y-6">
      <PageHeader
        title={t("test.title")}
        subtitle={t("test.subtitle")}
        actions={
          <Button variant="primary" icon={RefreshCw} loading={running} onClick={() => void run()}>
            {t("test.runAgain")}
          </Button>
        }
      />

      <MeasureCard />

      <section>
        <SectionTitle sub={checks && !running ? (problems === 0 ? t("test.allGood") : t("test.someIssues", { count: String(problems) })) : t("test.running")}>
          {t("test.checksTitle")}
          {checks && !running && <span className="ml-2 text-[13px] font-normal text-ink-3">{t("test.score", { ok: String(okCount), total: String(checks.length) })}</span>}
        </SectionTitle>
        <Card className="!py-1">
          <ul className="divide-y divide-line">
            {(checks ?? []).map((c) => (
              <li key={c.id} className="flex items-start gap-3 py-3">
                <StatusIcon status={c.status} />
                <div className="min-w-0 flex-1">
                  <div className="text-sm font-medium">{t(`test.check.${c.id}.title` as TKey)}</div>
                  <div className="mt-0.5 text-[13px] text-ink-2">{t(`test.check.${c.id}.${c.key}` as TKey, c.params)}</div>
                </div>
              </li>
            ))}
            {!checks && <li className="py-6 text-center text-[13px] text-ink-3">{t("test.running")}</li>}
          </ul>
        </Card>
      </section>

      <section>
        <SectionTitle>{t("test.handsOn")}</SectionTitle>
        <div className="grid gap-4 lg:grid-cols-2">
          <Card>
            <div className="flex items-start gap-3">
              <Play className="mt-0.5 h-5 w-5 shrink-0 text-accent" />
              <div className="min-w-0 flex-1">
                <div className="text-sm font-semibold">{t("test.tweakTitle")}</div>
                <p className="mt-1 text-[13px] text-ink-2">{t("test.tweakDesc")}</p>
                <Button className="mt-3" loading={tweakRunning} onClick={() => void runTweakTest()}>
                  {t("test.tweakRun")}
                </Button>
                {steps && (
                  <ul className="mt-3 space-y-1.5">
                    {steps.map((s) => {
                      const status: TestStatus = s.result === "error" ? "fail" : s.result === "skipped" ? "info" : s.restored ? "ok" : "warn";
                      return (
                        <li key={s.id} className="flex items-start gap-2 text-[13px]">
                          <StatusIcon status={status} />
                          <span>
                            <span className="font-medium">{t(`tweak.${s.id}.name` as TKey)}</span>
                            {": "}
                            {t(`test.tweakResult.${s.result === "ok" ? (s.restored ? "ok" : "notRestored") : s.result}` as TKey)}
                          </span>
                        </li>
                      );
                    })}
                  </ul>
                )}
              </div>
            </div>
          </Card>

          <Card>
            <div className="flex items-start gap-3">
              <Crosshair className="mt-0.5 h-5 w-5 shrink-0 text-accent" />
              <div className="min-w-0 flex-1">
                <div className="text-sm font-semibold">{t("test.crosshairTitle")}</div>
                <p className="mt-1 text-[13px] text-ink-2">{t("test.crosshairDesc")}</p>
                <Button className="mt-3" disabled={crosshairLeft > 0} onClick={() => void testCrosshair()}>
                  {crosshairLeft > 0 ? t("test.crosshairLeft", { sec: String(crosshairLeft) }) : t("test.crosshairRun")}
                </Button>
              </div>
            </div>
          </Card>

          <Card>
            <div className="flex items-start gap-3">
              <Download className="mt-0.5 h-5 w-5 shrink-0 text-accent" />
              <div className="min-w-0 flex-1">
                <div className="text-sm font-semibold">{t("test.updateTitle")}</div>
                <p className="mt-1 text-[13px] text-ink-2">{updateText}</p>
                <Button className="mt-3" loading={updateStatus === "checking"} onClick={() => void checkUpdate()}>
                  {t("update.checkNow")}
                </Button>
              </div>
            </div>
          </Card>

          <Card>
            <div className="flex items-start gap-3">
              <Joystick className="mt-0.5 h-5 w-5 shrink-0 text-accent" />
              <div className="min-w-0 flex-1">
                <div className="text-sm font-semibold">{t("test.controllerTitle")}</div>
                <p className="mt-1 text-[13px] text-ink-2">{t("test.controllerDesc")}</p>
                <Button className="mt-3" onClick={() => setPage("controller")}>
                  {t("test.controllerOpen")}
                </Button>
              </div>
            </div>
          </Card>
        </div>
      </section>
    </div>
  );
}
