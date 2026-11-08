import { useState } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api } from "../lib/api";
import { installAppUpdate, type AppUpdateProgress } from "../lib/appUpdate";
import { errText } from "../lib/errors";
import { toast } from "../lib/toast";
import { useStore } from "../lib/store";
import { Button, ConfirmModal, Modal, Spinner } from "./ui";

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
  const [confirm, setConfirm] = useState<"zapret" | "tgproxy" | null>(null);

  const available = updates?.filter((u) => u.updateAvailable) ?? [];
  const appAvailable = !!appUpdate?.updateAvailable;
  if (!showUpdatesModal || (available.length === 0 && !appAvailable)) return null;

  async function install(component: "zapret" | "tgproxy") {
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
      onClose={confirm ? undefined : dismissUpdatesModal}
      title={t("settings.updatesModalTitle")}
      footer={
        <Button variant="ghost" onClick={dismissUpdatesModal} disabled={installing !== null || appBusy}>
          {t("common.later")}
        </Button>
      }
    >
      <p className="mb-2">{t("settings.updatesModalText")}</p>
      <div className="divide-y divide-line">
        {appAvailable && (
          <div className="flex items-center justify-between gap-4 py-3">
            <div>
              <div className="text-sm font-medium text-fg">EasyZapret</div>
              <div className="text-[13px]">
                {appUpdate?.current ?? "—"} → <span className="font-medium text-fg">{appUpdate?.latest}</span>
              </div>
            </div>
            <div className="flex shrink-0 gap-2">
              {appUpdate?.releaseUrl && (
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={appBusy}
                  onClick={() => openUrl(appUpdate.releaseUrl!).catch(() => {})}
                >
                  {t("settings.downloadUpdate")}
                </Button>
              )}
              <Button
                size="sm"
                variant="primary"
                disabled={appBusy || installing !== null}
                onClick={() => void installApp()}
              >
                {appBusy && <Spinner size={14} />}
                {appBusy && appProgress != null
                  ? t("settings.installingUpdate", { percent: appProgress })
                  : t("settings.installUpdate")}
              </Button>
            </div>
          </div>
        )}
        {available.map((u) => (
          <div key={u.component} className="flex items-center justify-between gap-4 py-3">
            <div>
              <div className="text-sm font-medium text-fg">
                {u.component === "zapret" ? "Zapret" : "Telegram Proxy"}
              </div>
              <div className="text-[13px]">
                {u.installed ?? "—"} → <span className="font-medium text-fg">{u.latest}</span>
                {u.releaseUrl && (
                  <button
                    type="button"
                    className="ml-3 text-accent-text underline-offset-2 hover:underline"
                    onClick={() => openUrl(u.releaseUrl!).catch(() => {})}
                  >
                    {t("settings.openRelease")}
                  </button>
                )}
              </div>
            </div>
            <Button
              size="sm"
              variant="primary"
              disabled={installing !== null}
              onClick={() => setConfirm(u.component)}
            >
              {installing === u.component && <Spinner size={14} />}
              {t("common.update")}
            </Button>
          </div>
        ))}
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
    </Modal>
  );
}
