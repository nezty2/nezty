import { AlertTriangle, ExternalLink, FolderOpen, Image, Laptop, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { Button, Card, Kv, PageHeader, Row, SectionTitle, SegmentedControl, TextInput, Toggle } from "@/components/ui";
import { useT } from "@/i18n/hooks";
import { LANGS } from "@/i18n";
import { api, autostart, isTauri } from "@/lib/ipc";
import { formatMb, timeAgo } from "@/lib/format";
import type { DeviceSetting, Lang } from "@/lib/types";
import { AppearanceRows } from "@/components/AppearanceRows";
import { useApp } from "@/store/app";
import { useUpdate } from "@/store/update";

export function SettingsPage() {
  const { t, lang } = useT();
  const settings = useApp((s) => s.settings);
  const updateSettings = useApp((s) => s.updateSettings);
  const appInfo = useApp((s) => s.appInfo);
  const restorePoint = useApp((s) => s.restorePoint);
  const refreshRestorePoint = useApp((s) => s.refreshRestorePoint);
  const toast = useApp((s) => s.toast);
  const handleError = useApp((s) => s.handleError);
  const [startWithWindows, setStartWithWindows] = useState<boolean | null>(null);
  const [creating, setCreating] = useState(false);
  const [pingHost, setPingHost] = useState(settings?.dashboardPingHost ?? "");
  const [sgdb, setSgdb] = useState(settings?.steamGridDbKey ?? "");
  const device = useApp((s) => s.device);
  const [pendingDevice, setPendingDevice] = useState<DeviceSetting | null>(null);

  useEffect(() => {
    autostart.isEnabled().then(setStartWithWindows).catch(() => setStartWithWindows(false));
  }, []);
  useEffect(() => {
    setPingHost(settings?.dashboardPingHost ?? "");
    setSgdb(settings?.steamGridDbKey ?? "");
  }, [settings?.dashboardPingHost, settings?.steamGridDbKey]);

  if (!settings) return null;
  const last = restorePoint?.lastResult ?? null;
  const langValue: Lang | "system" = settings.language ?? "system";

  const createRp = async () => {
    setCreating(true);
    try {
      const r = await api.createRestorePoint();
      toast({ level: r.ok ? "success" : "warning", title: t(r.ok ? "notice.success" : "notice.warning"), message: t(`result.restore.${r.key}`) });
      if (r.ok) void updateSettings({ restorePointDone: true });
      void refreshRestorePoint();
    } catch (e) {
      handleError(e);
    } finally {
      setCreating(false);
    }
  };

  return (
    <div className="space-y-6">
      <PageHeader title={t("settings.title")} />

      <div className="grid gap-6 xl:grid-cols-2">
        <div className="space-y-6">
          <section>
            <SectionTitle>{t("settings.appearance")}</SectionTitle>
            <Card className="divide-y divide-line !py-1">
              <Row title={t("settings.language")}>
                <SegmentedControl<Lang | "system">
                  value={langValue}
                  onChange={(v) => void updateSettings({ language: v === "system" ? null : v })}
                  options={[{ value: "system", label: t("settings.languageSystem") }, ...LANGS.map((l) => ({ value: l.id, label: l.label }))]}
                />
              </Row>
              <AppearanceRows settings={settings} />
              <Row title={t("settings.reduceMotion")} desc={t("settings.reduceMotionDesc")}>
                <Toggle checked={settings.reduceMotion} onChange={(v) => void updateSettings({ reduceMotion: v })} label={t("settings.reduceMotion")} />
              </Row>
            </Card>
          </section>

          <section>
            <SectionTitle>{t("settings.device")}</SectionTitle>
            <Card className="divide-y divide-line !py-1">
              <Row title={t("settings.deviceType")} desc={device ? t("settings.deviceTypeDesc", { detected: t(`device.${device.detected}` as "device.pc") }) : undefined}>
                <SegmentedControl<DeviceSetting>
                  value={pendingDevice ?? settings.deviceType}
                  onChange={(v) => {
                    // Automatisch mag altijd. Handmatig alleen na een duidelijke waarschuwing.
                    if (v === "auto") {
                      setPendingDevice(null);
                      void updateSettings({ deviceType: "auto" });
                    } else if (v !== settings.deviceType) {
                      setPendingDevice(v);
                    }
                  }}
                  options={[
                    { value: "auto", label: t("settings.deviceAuto") },
                    { value: "pc", label: t("device.pc") },
                    { value: "laptop", label: t("device.laptop") },
                  ]}
                />
              </Row>
              {pendingDevice && (
                <div className="flex flex-col gap-3 py-3">
                  <p className="flex items-start gap-2 text-[13px] text-warn">
                    <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
                    {t("settings.deviceWarn")}
                  </p>
                  <div className="flex gap-2">
                    <Button
                      size="sm"
                      variant="primary"
                      onClick={() => {
                        void updateSettings({ deviceType: pendingDevice });
                        setPendingDevice(null);
                      }}
                    >
                      {t("settings.deviceConfirm", { kind: t(`device.${pendingDevice}` as "device.pc") })}
                    </Button>
                    <Button size="sm" variant="ghost" onClick={() => setPendingDevice(null)}>
                      {t("common.cancel")}
                    </Button>
                  </div>
                </div>
              )}
              {device?.effective === "laptop" && (
                <Row title={<span className="flex items-center gap-2"><Laptop className="h-4 w-4 text-ink-3" />{t("settings.boostCharger")}</span>} desc={t("settings.boostChargerDesc")}>
                  <Toggle checked={settings.boostRequiresCharger} onChange={(v) => void updateSettings({ boostRequiresCharger: v })} label={t("settings.boostCharger")} />
                </Row>
              )}
            </Card>
          </section>

          <section>
            <SectionTitle>{t("settings.behavior")}</SectionTitle>
            <Card className="divide-y divide-line !py-1">
              <Row title={t("settings.autoBoost")} desc={t("settings.autoBoostDesc")}>
                <Toggle checked={settings.autoBoostEnabled} onChange={(v) => void updateSettings({ autoBoostEnabled: v })} label={t("settings.autoBoost")} />
              </Row>
              <Row title={t("settings.startWithWindows")} desc={t("settings.startWithWindowsDesc")}>
                <Toggle
                  checked={startWithWindows ?? false}
                  disabled={startWithWindows === null || !isTauri}
                  label={t("settings.startWithWindows")}
                  onChange={async (v) => {
                    try {
                      await autostart.set(v);
                      setStartWithWindows(v);
                    } catch (e) {
                      handleError(e);
                    }
                  }}
                />
              </Row>
              <Row title={t("settings.startMinimized")} desc={t("settings.startMinimizedDesc")}>
                <Toggle checked={settings.startMinimized} onChange={(v) => void updateSettings({ startMinimized: v })} label={t("settings.startMinimized")} />
              </Row>
              <Row title={t("settings.minimizeToTray")} desc={t("settings.minimizeToTrayDesc")}>
                <Toggle checked={settings.minimizeToTray} onChange={(v) => void updateSettings({ minimizeToTray: v })} label={t("settings.minimizeToTray")} />
              </Row>
              <Row title={t("settings.dashboardPing")} desc={t("settings.dashboardPingDesc")}>
                <TextInput value={pingHost} onChange={setPingHost} className="w-40" mono />
                <Button size="sm" disabled={pingHost.trim() === settings.dashboardPingHost || !pingHost.trim()} onClick={() => void updateSettings({ dashboardPingHost: pingHost.trim() })}>
                  {t("common.save")}
                </Button>
              </Row>
            </Card>
          </section>

          <section>
            <SectionTitle>{t("settings.covers")}</SectionTitle>
            <Card className="space-y-3">
              <div className="flex items-start justify-between gap-4 border-b border-line pb-3">
                <div>
                  <div className="text-sm font-medium">{t("settings.onlineCovers")}</div>
                  <div className="mt-0.5 text-[13px] text-ink-3">{t("settings.onlineCoversDesc")}</div>
                </div>
                <Toggle
                  checked={settings.onlineCovers}
                  label={t("settings.onlineCovers")}
                  onChange={async (v) => {
                    if ((await updateSettings({ onlineCovers: v })) && v) void api.refreshCovers().catch(handleError);
                  }}
                />
              </div>
              <div className="flex items-start gap-3">
                <Image className="mt-0.5 h-4 w-4 shrink-0 text-ink-3" />
                <div>
                  <div className="text-sm font-medium">{t("settings.sgdbKey")}</div>
                  <div className="mt-0.5 text-[13px] text-ink-3">{t("settings.sgdbDesc")}</div>
                </div>
              </div>
              <div className="flex flex-wrap gap-2">
                <TextInput value={sgdb} onChange={setSgdb} type="password" mono className="min-w-[220px] flex-1" placeholder="••••••••••••" />
                <Button disabled={sgdb.trim() === settings.steamGridDbKey} onClick={() => void updateSettings({ steamGridDbKey: sgdb.trim() })}>
                  {t("common.save")}
                </Button>
              </div>
              <div className="flex flex-wrap gap-2">
                <Button size="sm" variant="ghost" icon={ExternalLink} onClick={() => api.openUrl("https://www.steamgriddb.com/profile/preferences/api").catch(handleError)}>
                  {t("settings.getKey")}
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  icon={RefreshCw}
                  onClick={async () => {
                    try {
                      await api.refreshCovers();
                      toast({ level: "info", title: t("notice.info"), message: t("settings.refetching") });
                    } catch (e) {
                      handleError(e);
                    }
                  }}
                >
                  {t("settings.refetch")}
                </Button>
              </div>
            </Card>
          </section>
        </div>

        <div className="space-y-6">
          <section>
            <SectionTitle>{t("settings.safety")}</SectionTitle>
            <Card className="divide-y divide-line !py-1">
              <Row title={t("settings.restorePoint")} desc={t("settings.restorePointDesc")}>
                <SegmentedControl
                  value={settings.restorePointPolicy}
                  onChange={(v) => void updateSettings({ restorePointPolicy: v })}
                  options={[
                    { value: "auto", label: t("settings.policyAuto") },
                    { value: "never", label: t("settings.policyNever") },
                  ]}
                />
              </Row>
              <div className="py-3.5">
                <div className="text-[13px]">
                  {settings.restorePointDone ? (
                    <span className="text-ok">{t("dashboard.restoreDone", { time: last ? timeAgo(lang, last.ts) : "" })}</span>
                  ) : last ? (
                    <span className={last.ok ? "text-ok" : "text-warn"}>{t(`result.restore.${last.key}`)}</span>
                  ) : (
                    <span className="text-ink-2">{t("dashboard.restoreAuto")}</span>
                  )}
                </div>
                {restorePoint?.protectionEnabled === false && <div className="mt-1 text-[12px] text-warn">{t("settings.protectionOff")}</div>}
                <div className="mt-2.5 flex flex-wrap gap-2">
                  <Button size="sm" loading={creating} onClick={() => void createRp()}>
                    {t("settings.createNow")}
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => api.openPath("C:\\Windows\\System32\\SystemPropertiesProtection.exe").catch(handleError)}>
                    {t("settings.openProtection")}
                  </Button>
                </div>
              </div>
              <div className="py-3.5 text-[12px] leading-relaxed text-ink-3">{t("settings.neverTouch")}</div>
            </Card>
          </section>

          <UpdateSection />

          <section>
            <SectionTitle>{t("settings.about")}</SectionTitle>
            <Card>
              <div className="divide-y divide-line">
                <Kv k={t("settings.version")} v={`nezty ${appInfo?.version ?? ""}`} />
                <Kv k={t("settings.windows")} v={appInfo?.osVersion ?? "-"} />
                <Kv k={t("dashboard.cpu")} v={appInfo?.cpuName ?? "-"} />
                <Kv k={t("dashboard.gpu")} v={appInfo?.gpuName ?? "-"} />
                <Kv k={t("settings.memory")} v={appInfo ? formatMb(lang, appInfo.totalMemoryMb) : "-"} />
                <Kv k={t("settings.dataFolder")} v={<span className="text-[12px]">{appInfo?.dataDir ?? "-"}</span>} />
              </div>
              <div className="mt-3 flex flex-wrap gap-2">
                <Button size="sm" icon={FolderOpen} onClick={() => appInfo && api.openPath(appInfo.dataDir).catch(handleError)}>
                  {t("settings.openFolder")}
                </Button>
                <Button size="sm" variant="ghost" onClick={() => void updateSettings({ firstRunDone: false })}>
                  {t("settings.showWelcome")}
                </Button>
              </div>
              <p className="mt-3 text-[12px] text-ink-3">{t("settings.presentMon")}</p>
            </Card>
          </section>
        </div>
      </div>
    </div>
  );
}

/** Updates: nezty zoekt zelf (verplicht); hier kun je ook meteen zoeken. */
function UpdateSection() {
  const { t } = useT();
  const settings = useApp((s) => s.settings);
  const version = useApp((s) => s.appInfo?.version ?? "");
  const { status, info, check, install } = useUpdate();
  if (!settings) return null;
  const message =
    status === "checking"
      ? t("update.checking")
      : status === "none"
        ? t("update.latest", { version })
        : status === "available" && info
          ? t("update.available", { version: info.version })
          : status === "error"
            ? t("update.error")
            : status === "installError"
              ? t("update.installError")
              : status === "unconfigured"
                ? t("update.notConfigured")
                : null;
  return (
    <section>
      <SectionTitle>{t("update.settingsTitle")}</SectionTitle>
      <Card className="divide-y divide-line !py-1">
        <Row title={t("update.current", { version })} desc={message ?? undefined}>
          {status === "available" ? (
            <Button size="sm" variant="primary" onClick={() => void install()}>
              {t("update.install")}
            </Button>
          ) : (
            <Button size="sm" icon={RefreshCw} loading={status === "checking" || status === "installing"} onClick={() => void check()}>
              {t("update.checkNow")}
            </Button>
          )}
        </Row>
      </Card>
    </section>
  );
}
