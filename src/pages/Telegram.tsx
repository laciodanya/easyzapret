import { useState } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api } from "../lib/api";
import { copyText } from "../lib/clipboard";
import { errText } from "../lib/errors";
import { toast } from "../lib/toast";
import { useStore } from "../lib/store";
import { Badge, Button, CopyField, Note, PageHeader, Panel, Row, Spinner, Switch } from "../components/ui";

export function TelegramPage() {
  const { t } = useTranslation();
  const { status, components, refreshStatus, refreshComponents } = useStore();
  const [busy, setBusy] = useState(false);
  const [installing, setInstalling] = useState(false);

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
    const ok = await copyText(tg.shareLink);
    toast(ok ? t("common.copied", { what: t("telegram.link") }) : t("common.copyFailed"), ok ? "ok" : "fail");
  }

  return (
    <div className="space-y-6">
      <PageHeader title={t("telegram.title")} description={t("telegram.description")} />

      {!components?.tgInstalled ? (
        <Panel>
          <Row title={t("telegram.notInstalled")}>
            <Button variant="primary" onClick={() => void install()} disabled={installing}>
              {installing && <Spinner size={14} />}
              {t("telegram.installNow")}
            </Button>
          </Row>
        </Panel>
      ) : (
        <>
          <Panel>
            <Row
              title={t("home.tgTitle")}
              description={
                components.tgVersion ? `${t("common.version")} ${components.tgVersion}` : undefined
              }
            >
              <Badge tone={tg?.running ? "ok" : "off"}>
                {tg?.running ? t("common.running") : t("common.stopped")}
              </Badge>
              <Switch
                label={t("home.tgTitle")}
                checked={!!tg?.running}
                disabled={busy}
                onChange={(v) => void toggle(v)}
              />
            </Row>
          </Panel>

          <Panel
            title={t("telegram.connectTitle")}
            description={t("telegram.connectDesc")}
            action={
              <>
                <Button size="sm" disabled={!tg?.shareLink} onClick={() => void copyLink()}>
                  {t("telegram.copyLink")}
                </Button>
                <Button
                  size="sm"
                  variant="primary"
                  disabled={!tg?.tgLink}
                  onClick={() => tg?.tgLink && openUrl(tg.tgLink).catch(() => {})}
                >
                  {t("telegram.openInTelegram")}
                </Button>
              </>
            }
          >
            <CopyField label={t("telegram.server")} value={tg?.host ?? "127.0.0.1"} />
            <CopyField label={t("telegram.port")} value={String(tg?.port ?? 1443)} />
            {tg?.secret ? (
              <CopyField label={t("telegram.secret")} value={tg.secret} secret />
            ) : (
              <div className="py-3.5 text-[13px] text-muted">{t("telegram.noSecret")}</div>
            )}
          </Panel>

          <div className="grid gap-3 md:grid-cols-2">
            <Note title={t("telegram.trayTitle")}>{t("telegram.trayNote")}</Note>
            <Note tone="warn" title={t("telegram.mediaHelpTitle")}>
              {t("telegram.mediaHint")}
            </Note>
          </div>
        </>
      )}
    </div>
  );
}
