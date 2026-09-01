import { useState } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api } from "../lib/api";
import { installAppUpdate, type AppUpdateProgress } from "../lib/appUpdate";
import { errText } from "../lib/errors";
import { toast } from "../lib/toast";
import { useStore } from "../lib/store";
import { Button, Modal, Spinner } from "../components/ui";

export function UpdatesModal() {
  const { t } = useTranslation();
  const {
    showUpdatesModal,
    updates,
    appUpdate,
    dismissUpdatesModal,
    refreshComponents,
    refreshStrategies,
    checkUpdates,
  } = useStore();
  const [installing, setInstalling] = useState<string | null>(null);
  const [appInstalling, setAppInstalling] = useState<AppUpdateProgress | null>(null);
  const [confirm, setConfirm] = useState<"zapret" | "tgproxy" | "vpncore" | null>(null);

  const available = updates?.filter((u) => u.updateAvailable) ?? [];
  const appAvailable = !!appUpdate?.updateAvailable;
  if (!showUpdatesModal || (available.length === 0 && !appAvailable)) return null;

  async function install(component: "zapret" | "tgproxy" | "vpncore") {
    setConfirm(null);
    setInstalling(component);
    try {
      await api.installComponent(component);
      await Promise.all([refreshComponents(), refreshStrategies()]);
      await checkUpdates();
      toast(t("setup.done"), "ok");
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setInstalling(null);
    }
  }

  async function installApp() {
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

  return (
    <Modal
      open
      onClose={dismissUpdatesModal}
      title={t("settings.updatesModalTitle")}
      footer={
        <Button onClick={dismissUpdatesModal} disabled={installing !== null || appBusy}>
          {t("common.later")}
        </Button>
      }
    >
      <p className="mb-4">{t("settings.updatesModalText")}</p>
      <div className="space-y-2">
        {appAvailable && (
          <div className="flex flex-col gap-2 rounded-xl bg-amber-500/10 px-4 py-3 ring-1 ring-amber-500/25 sm:flex-row sm:items-center sm:justify-between">
            <div>
              <div className="text-sm font-semibold text-amber-700 dark:text-amber-300">
                EasyZapret
              </div>
              <div className="text-xs text-slate-500 dark:text-slate-400">
                {appUpdate?.current ?? "—"} →{" "}
                <span className="font-semibold">{appUpdate?.latest}</span>
              </div>
            </div>
            <div className="flex shrink-0 flex-wrap gap-2">
            <Button
              variant="primary"
              disabled={appBusy || installing !== null}
              onClick={() => installApp()}
            >
              {appBusy ? <Spinner /> : null}
              {appBusy && appProgress != null
                ? t("settings.installingUpdate", { percent: appProgress })
                : t("settings.installUpdate")}
            </Button>
            {appUpdate?.releaseUrl && (
              <Button
                variant="ghost"
                disabled={appBusy}
                onClick={() => openUrl(appUpdate.releaseUrl!).catch(() => {})}
              >
                {t("settings.downloadUpdate")}
              </Button>
            )}
            </div>
          </div>
        )}
        {available.map((u) => (
          <div
            key={u.component}
            className="flex items-center justify-between gap-3 rounded-xl bg-slate-50 px-4 py-3 dark:bg-slate-800/50"
          >
            <div>
              <div className="text-sm font-semibold text-slate-700 dark:text-slate-200">
                {u.component === "zapret"
                  ? "Zapret"
                  : u.component === "tgproxy"
                    ? "Telegram Proxy"
                    : "VPN (Xray)"}
              </div>
              <div className="text-xs text-slate-400">
                {u.installed ?? "—"} → <span className="font-semibold">{u.latest}</span>
                {u.releaseUrl && (
                  <button
                    className="ml-2 underline"
                    onClick={() => openUrl(u.releaseUrl!).catch(() => {})}
                  >
                    {t("settings.openRelease")}
                  </button>
                )}
              </div>
            </div>
            <Button
              variant="primary"
              disabled={installing !== null}
              onClick={() => setConfirm(u.component)}
            >
              {installing === u.component ? <Spinner /> : null}
              {t("common.update")}
            </Button>
          </div>
        ))}
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
    </Modal>
  );
}
