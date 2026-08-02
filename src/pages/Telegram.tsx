import { useState } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { api } from "../lib/api";
import { errText } from "../lib/errors";
import { toast } from "../lib/toast";
import { useStore } from "../lib/store";
import {
  Badge,
  Button,
  Card,
  FieldRow,
  Note,
  PageHeader,
  Spinner,
  Switch,
} from "../components/ui";

export function TelegramPage() {
  const { t } = useTranslation();
  const { status, components, refreshStatus, refreshComponents } = useStore();
  const [busy, setBusy] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [showSecret, setShowSecret] = useState(false);

  const tg = status?.tg;

  async function toggle(value: boolean) {
    setBusy(true);
    try {
      if (value) {
        await api.startTg();
        await new Promise((r) => setTimeout(r, 1200));
      } else {
        await api.stopTg();
      }
      await refreshStatus();
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setBusy(false);
    }
  }

  async function install() {
    setInstalling(true);
    try {
      await api.installComponent("tgproxy");
      await Promise.all([refreshComponents(), refreshStatus()]);
      toast(t("setup.done"), "ok");
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setInstalling(false);
    }
  }

  async function copyLink() {
    if (!tg?.shareLink) return;
    try {
      await writeText(tg.shareLink);
      toast(t("common.copied"), "ok");
    } catch (e) {
      toast(errText(t, e), "fail");
    }
  }

  return (
    <div className="mx-auto max-w-3xl">
      <PageHeader title={t("telegram.title")} description={t("telegram.description")} />

      {!components?.tgInstalled ? (
        <Card className="flex items-center justify-between gap-4">
          <span className="text-sm text-slate-500">{t("telegram.notInstalled")}</span>
          <Button variant="primary" onClick={install} disabled={installing}>
            {installing ? <Spinner /> : null}
            {t("telegram.installNow")}
          </Button>
        </Card>
      ) : (
        <>
          <Card className="mb-4">
            <FieldRow
              title={t("home.tgTitle")}
              description={
                components.tgVersion
                  ? `${t("common.version")}: ${components.tgVersion}`
                  : undefined
              }
            >
              <div className="flex items-center gap-3">
                <Badge tone={tg?.running ? "ok" : "off"}>
                  {tg?.running ? t("common.running") : t("common.stopped")}
                </Badge>
                <Switch checked={!!tg?.running} disabled={busy} onChange={toggle} />
              </div>
            </FieldRow>
            <div className="mt-2 grid grid-cols-1 gap-2 text-sm sm:grid-cols-3">
              <div className="rounded-xl bg-slate-50 p-3 dark:bg-slate-800/50">
                <div className="text-xs text-slate-400">{t("telegram.server")}</div>
                <code className="text-sm font-semibold text-slate-700 dark:text-slate-200">
                  {tg?.host ?? "127.0.0.1"}
                </code>
              </div>
              <div className="rounded-xl bg-slate-50 p-3 dark:bg-slate-800/50">
                <div className="text-xs text-slate-400">{t("telegram.port")}</div>
                <code className="text-sm font-semibold text-slate-700 dark:text-slate-200">
                  {tg?.port ?? 1443}
                </code>
              </div>
              <div className="rounded-xl bg-slate-50 p-3 dark:bg-slate-800/50">
                <div className="text-xs text-slate-400">{t("telegram.secret")}</div>
                {tg?.secret ? (
                  <button
                    className="block max-w-full truncate text-left font-mono text-sm font-semibold text-slate-700 dark:text-slate-200"
                    title={showSecret ? tg.secret : t("telegram.secretHidden")}
                    onClick={() => setShowSecret(!showSecret)}
                  >
                    {showSecret ? tg.secret : "••••••••••••"}
                  </button>
                ) : (
                  <span className="text-xs text-slate-400">{t("telegram.noSecret")}</span>
                )}
              </div>
            </div>

            <div className="mt-4 flex flex-wrap gap-2">
              <Button
                variant="primary"
                disabled={!tg?.tgLink}
                onClick={() => tg?.tgLink && openUrl(tg.tgLink).catch(() => {})}
              >
                {t("telegram.openInTelegram")}
              </Button>
              <Button disabled={!tg?.shareLink} onClick={copyLink}>
                {t("telegram.copyLink")}
              </Button>
            </div>
          </Card>

          <div className="grid gap-3 md:grid-cols-2">
            <Note tone="info" title={t("telegram.trayTitle")}>
              {t("telegram.trayNote")}
            </Note>
            <Note tone="warn" title={t("telegram.mediaHelpTitle")}>
              {t("telegram.mediaHint")}
            </Note>
          </div>
        </>
      )}
    </div>
  );
}
