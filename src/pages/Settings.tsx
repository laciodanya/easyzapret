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
  ConfirmModal,
  Note,
  PageHeader,
  Panel,
  ProgressBar,
  Row,
  Segmented,
  Spinner,
  Switch,
} from "../components/ui";
import type { InstallProgress, Settings } from "../lib/types";

const REPO_URL = "https://github.com/laciodanya/easyzapret";

function Link({ href, children }: { href: string; children: string }) {
  return (
    <button
      type="button"
      className="text-accent-text underline-offset-2 hover:underline"
      onClick={() => openUrl(href).catch(() => {})}
    >
      {children}
    </button>
  );
}

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
  const [confirm, setConfirm] = useState<"zapret" | "tgproxy" | null>(null);

  useEffect(() => {
    let un: UnlistenFn | null = null;
    listen<InstallProgress>("install-progress", (e) => setProgress(e.payload)).then((u) => (un = u));
    return () => {
      un?.();
    };
  }, []);

  function requestInstall(component: "zapret" | "tgproxy") {
    const alreadyInstalled =
      component === "zapret" ? components?.zapretInstalled : components?.tgInstalled;
    // Updating/reinstalling an installed component stops the running bypass —
    // warn first. A first-time install has nothing to stop.
    if (alreadyInstalled) setConfirm(component);
    else void install(component);
  }

  async function install(component: "zapret" | "tgproxy") {
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

  function componentRow(component: "zapret" | "tgproxy") {
    const installed = component === "zapret" ? components?.zapretInstalled : components?.tgInstalled;
    const version = component === "zapret" ? components?.zapretVersion : components?.tgVersion;
    const upd = updates?.find((u) => u.component === component);
    const name = component === "zapret" ? "Zapret" : "Telegram Proxy";
    const source = component === "zapret" ? "Flowseal/zapret-discord-youtube" : "Flowseal/tg-ws-proxy";
    const busy = installing === component;
    const pct =
      busy &&
      progress?.component === component &&
      progress.phase === "downloading" &&
      progress.total > 0
        ? Math.round((progress.downloaded / progress.total) * 100)
        : null;

    return (
      <Row
        key={component}
        title={
          <span className="flex items-center gap-2">
            {name}
            {installed ? (
              <Badge tone="ok">{version ?? t("common.installed")}</Badge>
            ) : (
              <Badge tone="off">{t("common.notInstalled")}</Badge>
            )}
          </span>
        }
        description={
          upd?.updateAvailable ? (
            <span className="text-warn">
              {t("settings.updateAvailable", { version: upd.latest })}
              {upd.releaseUrl && (
                <>
                  {" · "}
                  <Link href={upd.releaseUrl}>{t("settings.openRelease")}</Link>
                </>
              )}
            </span>
          ) : upd?.latest ? (
            `${source} · ${t("settings.upToDate")}`
          ) : (
            `${source} · ${t("settings.notChecked")}`
          )
        }
      >
        <Button
          variant={upd?.updateAvailable || !installed ? "primary" : "secondary"}
          disabled={installing !== null}
          onClick={() => requestInstall(component)}
        >
          {busy && <Spinner size={14} />}
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
      </Row>
    );
  }

  const autostart = settings?.autostart;

  return (
    <div className="space-y-6">
      <PageHeader title={t("settings.title")} />

      <Panel title={t("settings.appearance")}>
        <Row title={t("settings.language")}>
          <Segmented
            value={settings?.language ?? "auto"}
            options={[
              { value: "auto", label: t("settings.langAuto") },
              { value: "ru", label: "Русский" },
              { value: "en", label: "English" },
            ]}
            onChange={(v) => void updateSettings({ language: v === "auto" ? null : v })}
          />
        </Row>
        <Row title={t("settings.theme")}>
          <Segmented<Settings["theme"]>
            value={settings?.theme ?? "system"}
            options={[
              { value: "system", label: t("settings.themeSystem") },
              { value: "light", label: t("settings.themeLight") },
              { value: "dark", label: t("settings.themeDark") },
            ]}
            onChange={(theme) => void updateSettings({ theme })}
          />
        </Row>
      </Panel>

      <Panel title={t("settings.autostartTitle")} description={t("settings.autostartDesc")}>
        <Row title={t("settings.launchAtLogin")} description={t("settings.launchAtLoginDesc")}>
          <Switch
            label={t("settings.launchAtLogin")}
            checked={autostart?.launchAtLogin ?? true}
            onChange={async (v) => {
              try {
                await updateAutostart({ launchAtLogin: v });
              } catch (e) {
                toast(errText(t, e), "fail");
              }
            }}
          />
        </Row>
        <Row title={t("settings.startMinimized")} description={t("settings.startMinimizedDesc")}>
          <Switch
            label={t("settings.startMinimized")}
            checked={autostart?.startMinimized ?? false}
            onChange={(v) => void updateAutostart({ startMinimized: v })}
          />
        </Row>
        <Row title={t("settings.autoStartZapret")} description={t("settings.autoStartZapretDesc")}>
          <Switch
            label={t("settings.autoStartZapret")}
            checked={autostart?.autoStartZapret ?? false}
            disabled={autostart?.autoStartWarp}
            onChange={(v) => void updateAutostart({ autoStartZapret: v })}
          />
        </Row>
        <Row title={t("settings.autoStartWarp")} description={t("settings.autoStartWarpDesc")}>
          <Switch
            label={t("settings.autoStartWarp")}
            checked={autostart?.autoStartWarp ?? false}
            onChange={(v) =>
              void updateAutostart({ autoStartWarp: v, ...(v ? { autoStartZapret: true } : {}) })
            }
          />
        </Row>
        <Row title={t("settings.autoStartTg")} description={t("settings.autoStartTgDesc")}>
          <Switch
            label={t("settings.autoStartTg")}
            checked={autostart?.autoStartTg ?? false}
            onChange={(v) => void updateAutostart({ autoStartTg: v })}
          />
        </Row>
        <p className="py-3 text-xs text-faint">{t("settings.autostartOrderNote")}</p>
      </Panel>

      <Panel
        title={t("settings.components")}
        action={
          <Button size="sm" variant="ghost" icon="refresh" onClick={() => void onCheckUpdates()} disabled={checking}>
            {t("settings.checkUpdates")}
          </Button>
        }
      >
        {componentRow("zapret")}
        {componentRow("tgproxy")}
        <Row title={t("settings.checkUpdatesOnStart")}>
          <Switch
            label={t("settings.checkUpdatesOnStart")}
            checked={settings?.checkUpdatesOnStart ?? true}
            onChange={(v) => void updateSettings({ checkUpdatesOnStart: v })}
          />
        </Row>
      </Panel>

      <Panel title={t("settings.about")}>
        <Row
          title={`EasyZapret ${appInfo?.version ?? ""}`}
          description={
            appUpdate?.updateAvailable ? (
              <span className="text-warn">
                {t("settings.appUpdateAvailable", { version: appUpdate.latest })}
              </span>
            ) : appUpdate?.latest ? (
              t("settings.appUpToDate")
            ) : undefined
          }
        >
          {appUpdate?.updateAvailable && (
            <>
              {appUpdate.releaseUrl && !appBusy && (
                <Button variant="ghost" onClick={() => openUrl(appUpdate.releaseUrl!).catch(() => {})}>
                  {t("settings.downloadUpdate")}
                </Button>
              )}
              <Button variant="primary" disabled={appBusy} onClick={() => void onInstallAppUpdate()}>
                {appBusy && <Spinner size={14} />}
                {appBusy && appProgress != null
                  ? t("settings.installingUpdate", { percent: appProgress })
                  : t("settings.installUpdate")}
              </Button>
            </>
          )}
        </Row>
        {appBusy && appProgress != null && (
          <div className="pb-4">
            <ProgressBar value={appProgress} />
          </div>
        )}
        <Row title={t("settings.dataDir")} description={t("settings.dataDirNote")}>
          <code className="selectable rounded-md bg-sunken px-2.5 py-1.5 font-mono text-xs">
            {appInfo?.dataDir ?? "C:\\EasyZapret"}
          </code>
        </Row>
        <Row title={t("settings.links")} description={t("settings.aboutText")}>
          <span className="flex gap-4 text-[13px]">
            <Link href={REPO_URL}>EasyZapret</Link>
            <Link href="https://github.com/Flowseal/zapret-discord-youtube">zapret-discord-youtube</Link>
            <Link href="https://github.com/Flowseal/tg-ws-proxy">tg-ws-proxy</Link>
            <Link href={`${REPO_URL}/issues`}>{t("settings.reportProblem")}</Link>
          </span>
        </Row>
        <p className="py-3 text-xs text-faint">{t("settings.appUpdateNote")}</p>
      </Panel>

      <div className="grid gap-3">
        <Note title={t("settings.secureDns")}>{t("settings.secureDnsText")}</Note>
        <Note tone="warn" title={t("settings.antivirus")}>
          {t("settings.antivirusText")}
        </Note>
        <Note tone="fail" title={t("settings.fakeRepos")}>
          {t("settings.fakeReposText")}
        </Note>
      </div>

      <ConfirmModal
        open={confirm !== null}
        title={t("settings.updateConfirmTitle")}
        confirmLabel={t("settings.updateContinue")}
        onClose={() => setConfirm(null)}
        onConfirm={() => confirm && void install(confirm)}
      >
        {confirm === "tgproxy" ? t("settings.updateConfirmTg") : t("settings.updateConfirmZapret")}
      </ConfirmModal>
    </div>
  );
}
