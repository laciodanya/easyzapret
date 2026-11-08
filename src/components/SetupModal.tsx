import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { api } from "../lib/api";
import { errText } from "../lib/errors";
import { toast } from "../lib/toast";
import { useStore } from "../lib/store";
import { Badge, Button, Modal, Note, Spinner } from "./ui";
import type { InstallProgress } from "../lib/types";

export function SetupModal() {
  const { t } = useTranslation();
  const {
    showSetup,
    components,
    appInfo,
    dismissSetup,
    refreshComponents,
    refreshStrategies,
    refreshStatus,
  } = useStore();
  const [installing, setInstalling] = useState<string | null>(null);
  const [progress, setProgress] = useState<InstallProgress | null>(null);

  useEffect(() => {
    let un: UnlistenFn | null = null;
    listen<InstallProgress>("install-progress", (e) => setProgress(e.payload)).then(
      (u) => (un = u),
    );
    return () => {
      un?.();
    };
  }, []);

  if (!showSetup || !components) return null;

  async function installOne(component: "zapret" | "tgproxy") {
    setInstalling(component);
    try {
      await api.installComponent(component);
      await Promise.all([refreshComponents(), refreshStrategies(), refreshStatus()]);
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setInstalling(null);
      setProgress(null);
    }
  }

  async function installAll() {
    if (!components!.zapretInstalled) await installOne("zapret");
    if (!components!.tgInstalled) await installOne("tgproxy");
    const state = useStore.getState().components;
    if (state?.zapretInstalled && state?.tgInstalled) dismissSetup();
  }

  function row(component: "zapret" | "tgproxy", label: string, installed: boolean) {
    const busy = installing === component;
    const pct =
      busy &&
      progress?.component === component &&
      progress.phase === "downloading" &&
      progress.total > 0
        ? Math.round((progress.downloaded / progress.total) * 100)
        : null;
    return (
      <div className="flex items-center justify-between gap-3 py-3">
        <span className="text-sm font-medium text-fg">{label}</span>
        {installed ? (
          <Badge tone="ok">{t("setup.done")}</Badge>
        ) : busy ? (
          <span className="flex items-center gap-2 text-[13px] text-muted">
            <Spinner size={14} />
            {pct !== null
              ? t("setup.downloading", { percent: pct })
              : progress?.phase === "extracting"
                ? t("setup.extracting")
                : t("common.loading")}
          </span>
        ) : (
          <Button size="sm" onClick={() => void installOne(component)} disabled={installing !== null}>
            {t("setup.download")}
          </Button>
        )}
      </div>
    );
  }

  const allDone = components.zapretInstalled && components.tgInstalled;
  const dir = appInfo?.dataDir ?? "C:\\EasyZapret";

  return (
    <Modal
      open
      title={t("setup.title")}
      size="lg"
      footer={
        <>
          <Button variant="ghost" onClick={dismissSetup} disabled={installing !== null}>
            {allDone ? t("common.close") : t("setup.skip")}
          </Button>
          {!allDone && (
            <Button variant="primary" onClick={() => void installAll()} disabled={installing !== null}>
              {installing && <Spinner size={14} />}
              {t("setup.installAll")}
            </Button>
          )}
        </>
      }
    >
      <p className="mb-2">{t("setup.description", { dir })}</p>
      <div className="divide-y divide-line">
        {row("zapret", t("setup.zapretComponent"), components.zapretInstalled)}
        {row("tgproxy", t("setup.tgComponent"), components.tgInstalled)}
      </div>
      <div className="mt-4 space-y-2">
        <Note tone="fail">{t("setup.securityNote")}</Note>
        <Note tone="warn">{t("setup.antivirusNote", { dir })}</Note>
      </div>
    </Modal>
  );
}
