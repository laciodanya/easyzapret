import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api } from "../lib/api";
import { installAppUpdate, type AppUpdateProgress } from "../lib/appUpdate";
import { errText } from "../lib/errors";
import { toast } from "../lib/toast";
import { useStore } from "../lib/store";
import {
  Badge,
  Button,
  Card,
  FieldRow,
  Modal,
  Note,
  PageHeader,
  Segmented,
  Spinner,
  Switch,
} from "../components/ui";
import type { InstallProgress, Settings } from "../lib/types";

export function SettingsPage() {
  const { t } = useTranslation();
  const {
    appInfo,
    settings,
    components,
    updates,
    appUpdate,
    updateSettings,
    updateAutostart,
    checkUpdates,
    refreshComponents,
    refreshStrategies,
  } = useStore();
  const [installing, setInstalling] = useState<string | null>(null);
  const [progress, setProgress] = useState<InstallProgress | null>(null);
  const [checking, setChecking] = useState(false);
  const [appInstalling, setAppInstalling] = useState<AppUpdateProgress | null>(null);
  const [confirm, setConfirm] = useState<"zapret" | "tgproxy" | "vpncore" | null>(null);

  useEffect(() => {
    let un: UnlistenFn | null = null;
    listen<InstallProgress>("install-progress", (e) => setProgress(e.payload)).then(
      (u) => (un = u),
    );
    return () => {
      un?.();
    };
  }, []);

  function requestInstall(component: "zapret" | "tgproxy" | "vpncore") {
    const alreadyInstalled =
      component === "zapret"
        ? components?.zapretInstalled
        : component === "tgproxy"
          ? components?.tgInstalled
          : components?.vpnCoreInstalled;
    if (alreadyInstalled) {
      setConfirm(component);
    } else {
      void install(component);
    }
  }

  async function install(component: "zapret" | "tgproxy" | "vpncore") {
    setConfirm(null);
    setInstalling(component);
    try {
      await api.installComponent(component);
      await Promise.all([refreshComponents(), refreshStrategies()]);
      toast(t("setup.done"), "ok");
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setInstalling(null);
      setProgress(null);
    }
  }

  async function onInstallAppUpdate() {
    setAppInstalling({ phase: "checking" });
    try {
      const installed = await installAppUpdate((p) => setAppInstalling(p));
      if (!installed) {
        toast(t("settings.appUpToDate"), "ok");
        setAppInstalling(null);
      }
    } catch (e) {
      toast(errText(t, e), "fail");
      setAppInstalling(null);
    }
  }

  const appBusy = appInstalling !== null;
  const appProgress =
    appInstalling?.phase === "downloading"
      ? appInstalling.percent
      : appInstalling?.phase === "installing"
        ? 100
        : null;

  async function onCheckUpdates() {
    setChecking(true);
    try {
      await checkUpdates();
    } finally {
      setChecking(false);
    }
  }

  function componentCard(component: "zapret" | "tgproxy" | "vpncore") {
    const installed =
      component === "zapret"
        ? components?.zapretInstalled
        : component === "tgproxy"
          ? components?.tgInstalled
          : components?.vpnCoreInstalled;
    const version =
      component === "zapret"
        ? components?.zapretVersion
        : component === "tgproxy"
          ? components?.tgVersion
          : components?.vpnCoreVersion;
    const upd = updates?.find((u) => u.component === component);
    const name =
      component === "zapret"
        ? "Zapret (zapret-discord-youtube)"
        : component === "tgproxy"
          ? "Telegram Proxy (tg-ws-proxy)"
          : "VPN core (Xray)";
    const busy = installing === component;
    const pct =
      busy && progress?.component === component && progress.phase === "downloading" && progress.total > 0
        ? Math.round((progress.downloaded / progress.total) * 100)
        : null;

    return (
      <div className="flex items-center justify-between gap-4 py-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2 text-sm font-semibold text-slate-800 dark:text-slate-100">
            {name}
            {installed ? (
              <Badge tone="ok">{version ?? t("common.installed")}</Badge>
            ) : (
              <Badge tone="off">{t("common.notInstalled")}</Badge>
            )}
          </div>
          <div className="mt-0.5 text-xs text-slate-400">
            {upd?.updateAvailable ? (
              <span className="font-medium text-amber-600 dark:text-amber-400">
                {t("settings.updateAvailable", { version: upd.latest })}
                {upd.releaseUrl && (
                  <button
                    className="ml-2 underline"
                    onClick={() => openUrl(upd.releaseUrl!).catch(() => {})}
                  >
                    {t("settings.openRelease")}
                  </button>
                )}
              </span>
            ) : upd?.latest ? (
              t("settings.upToDate")
            ) : (
              t("settings.notChecked")
            )}
          </div>
        </div>
        <Button
          variant={upd?.updateAvailable || !installed ? "primary" : "secondary"}
          disabled={installing !== null}
          onClick={() => requestInstall(component)}
        >
          {busy ? <Spinner /> : null}
          {busy
            ? pct !== null
              ? t("setup.downloading", { percent: pct })
              : progress?.phase === "extracting"
                ? t("setup.extracting")
                : t("common.loading")
            : !installed
              ? t("common.install")
              : upd?.updateAvailable
                ? t("common.update")
                : t("settings.reinstall")}
        </Button>
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-3xl">
      <PageHeader title={t("settings.title")} />

      <Card className="mb-4 divide-y divide-slate-100 dark:divide-slate-800">
        <FieldRow title={t("settings.language")}>
          <Segmented
            value={settings?.language ?? "auto"}
            options={[
              { value: "auto", label: t("settings.langAuto") },
              { value: "ru", label: "Русский" },
              { value: "en", label: "English" },
            ]}
            onChange={(v) => updateSettings({ language: v === "auto" ? null : (v as "ru" | "en") })}
          />
        </FieldRow>
        <FieldRow title={t("settings.theme")}>
          <Segmented
            value={settings?.theme ?? "system"}
            options={[
              { value: "system", label: t("settings.themeSystem") },
              { value: "light", label: t("settings.themeLight") },
              { value: "purple", label: t("settings.themePurple") },
            ]}
            onChange={(theme) => updateSettings({ theme: theme as Settings["theme"] })}
          />
        </FieldRow>
        <FieldRow title={t("settings.dataDir")} description={t("settings.dataDirNote")}>
          <code className="rounded-lg bg-slate-100 px-2.5 py-1.5 text-xs text-slate-600 dark:bg-slate-800 dark:text-slate-300">
            {appInfo?.dataDir ?? "C:\\EasyZapret"}
          </code>
        </FieldRow>
      </Card>

      <Card className="mb-4 divide-y divide-[rgb(var(--border)/0.35)]">
        <h3 className="pb-1 text-sm font-bold uppercase tracking-wide text-[rgb(var(--text-secondary))]">
          {t("settings.autostartTitle")}
        </h3>
        <p className="pb-3 text-xs leading-relaxed text-[rgb(var(--text-secondary))]">
          {t("settings.autostartDesc")}
        </p>
        <FieldRow title={t("settings.launchAtLogin")} description={t("settings.launchAtLoginDesc")}>
          <Switch
            checked={settings?.autostart.launchAtLogin ?? true}
            onChange={async (v) => {
              try {
                await updateAutostart({ launchAtLogin: v });
              } catch (e) {
                toast(errText(t, e), "fail");
              }
            }}
          />
        </FieldRow>
        <FieldRow title={t("settings.startMinimized")} description={t("settings.startMinimizedDesc")}>
          <Switch
            checked={settings?.autostart.startMinimized ?? false}
            onChange={(v) => updateAutostart({ startMinimized: v })}
          />
        </FieldRow>
        <FieldRow title={t("settings.autoStartZapret")} description={t("settings.autoStartZapretDesc")}>
          <Switch
            checked={settings?.autostart.autoStartZapret ?? false}
            disabled={settings?.autostart.autoStartWarp}
            onChange={(v) => updateAutostart({ autoStartZapret: v })}
          />
        </FieldRow>
        <FieldRow title={t("settings.autoStartWarp")} description={t("settings.autoStartWarpDesc")}>
          <Switch
            checked={settings?.autostart.autoStartWarp ?? false}
            onChange={(v) => updateAutostart({ autoStartWarp: v, ...(v ? { autoStartZapret: true } : {}) })}
          />
        </FieldRow>
        <FieldRow title={t("settings.autoStartTg")} description={t("settings.autoStartTgDesc")}>
          <Switch
            checked={settings?.autostart.autoStartTg ?? false}
            onChange={(v) => updateAutostart({ autoStartTg: v })}
          />
        </FieldRow>
        <Note tone="info">{t("settings.autostartOrderNote")}</Note>
      </Card>

      <Card className="mb-4">
        <div className="mb-1 flex items-center justify-between">
          <h3 className="text-sm font-bold uppercase tracking-wide text-slate-400">
            {t("settings.components")}
          </h3>
          <Button variant="ghost" onClick={onCheckUpdates} disabled={checking}>
            {checking ? <Spinner /> : null}
            {t("settings.checkUpdates")}
          </Button>
        </div>
        <div className="divide-y divide-slate-100 dark:divide-slate-800">
          {componentCard("zapret")}
          {componentCard("tgproxy")}
          {componentCard("vpncore")}
        </div>
        <FieldRow title={t("settings.checkUpdatesOnStart")}>
          <Switch
            checked={settings?.checkUpdatesOnStart ?? true}
            onChange={(v) => updateSettings({ checkUpdatesOnStart: v })}
          />
        </FieldRow>
      </Card>

      <div className="grid gap-3 md:grid-cols-2">
        <Note tone="info" title={t("settings.secureDns")}>
          {t("settings.secureDnsText")}
        </Note>
        <Note tone="warn" title={t("settings.antivirus")}>
          {t("settings.antivirusText")}
        </Note>
        <Note tone="fail" title={t("settings.fakeRepos")}>
          {t("settings.fakeReposText")}
        </Note>
        <div className="md:col-span-2">
          <Note tone={appUpdate?.updateAvailable ? "warn" : "info"} title={t("settings.about")}>
            {t("settings.aboutText", { version: appInfo?.version ?? "0.6.0" })}{" "}
            <button
              className="underline"
              onClick={() => openUrl("https://github.com/Flowseal/zapret-discord-youtube").catch(() => {})}
            >
              zapret-discord-youtube
            </button>
            {" · "}
            <button
              className="underline"
              onClick={() => openUrl("https://github.com/Flowseal/tg-ws-proxy").catch(() => {})}
            >
              tg-ws-proxy
            </button>
            <div className="mt-2">
              {appUpdate?.updateAvailable ? (
                <div className="flex flex-wrap items-center gap-2">
                  <span className="font-medium text-amber-600 dark:text-amber-400">
                    {t("settings.appUpdateAvailable", { version: appUpdate.latest })}
                  </span>
                  <Button
                    variant="primary"
                    disabled={appBusy}
                    onClick={() => onInstallAppUpdate()}
                  >
                    {appBusy ? <Spinner /> : null}
                    {appBusy && appProgress != null
                      ? t("settings.installingUpdate", { percent: appProgress })
                      : t("settings.installUpdate")}
                  </Button>
                  {appUpdate.releaseUrl && (
                    <button
                      className="text-sm underline"
                      disabled={appBusy}
                      onClick={() => openUrl(appUpdate.releaseUrl!).catch(() => {})}
                    >
                      {t("settings.downloadUpdate")}
                    </button>
                  )}
                </div>
              ) : appUpdate?.latest ? (
                <span className="text-slate-400">{t("settings.appUpToDate")}</span>
              ) : null}
              <p className="mt-2 text-xs text-slate-500 dark:text-slate-400">
                {t("settings.appUpdateNote")}
              </p>
            </div>
          </Note>
        </div>
      </div>

      <Modal
        open={confirm !== null}
        onClose={() => setConfirm(null)}
        title={t("settings.updateConfirmTitle")}
        footer={
          <>
            <Button onClick={() => setConfirm(null)}>{t("common.cancel")}</Button>
            <Button variant="primary" onClick={() => confirm && install(confirm)}>
              {t("settings.updateContinue")}
            </Button>
          </>
        }
      >
        <p>
          {confirm === "tgproxy"
            ? t("settings.updateConfirmTg")
            : confirm === "vpncore"
              ? t("settings.updateConfirmVpn")
              : t("settings.updateConfirmZapret")}
        </p>
      </Modal>
    </div>
  );
}
