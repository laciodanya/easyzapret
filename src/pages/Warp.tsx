import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api } from "../lib/api";
import { errText } from "../lib/errors";
import { toast } from "../lib/toast";
import { useStore } from "../lib/store";
import { waitForStatus } from "../lib/status";
import type { WarpStatus } from "../lib/types";
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

type WarpMode = "warp" | "warp+doh" | "doh";

function normalizeMode(mode: string | null | undefined): WarpMode {
  const low = (mode ?? "").toLowerCase();
  if (low.includes("doh") && low.includes("warp")) return "warp+doh";
  if (low === "doh" || (low.includes("doh") && !low.includes("warp"))) return "doh";
  return "warp";
}

export function WarpPage() {
  const { t } = useTranslation();
  const { status, refreshStatus } = useStore();

  const [details, setDetails] = useState<WarpStatus | null>(null);
  const [action, setAction] = useState<"starting" | "stopping" | null>(null);
  const [modeBusy, setModeBusy] = useState(false);
  const [resetting, setResetting] = useState(false);
  const [confirmReset, setConfirmReset] = useState(false);

  const warp = status?.warp;
  const installed = warp?.installed ?? details?.installed ?? false;
  const connected = warp?.connected ?? false;
  const zapretOn = !!(status?.zapret.running || status?.zapret.serviceState === "RUNNING");
  const mode = normalizeMode(details?.mode);

  const loadDetails = useCallback(async () => {
    try {
      setDetails(await api.warpDetails());
    } catch {
      // ignore — quick status from polling still drives the UI
    }
  }, []);

  useEffect(() => {
    loadDetails();
  }, [loadDetails]);

  async function toggle(value: boolean) {
    if (value && !zapretOn) {
      toast(t("warp.needZapret"), "fail");
      return;
    }
    setAction(value ? "starting" : "stopping");
    try {
      if (value) {
        await api.warpConnect();
      } else {
        await api.warpDisconnect();
      }
      const ready = await waitForStatus(
        (next) => next.warp.connected === value,
        (next) => useStore.setState({ status: next }),
        40_000,
      );
      if (!ready) {
        toast(t(value ? "home.startTimeout" : "home.stopTimeout"), "info");
      }
      await loadDetails();
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setAction(null);
    }
  }

  async function changeMode(next: WarpMode) {
    setModeBusy(true);
    try {
      await api.warpSetMode(next);
      await loadDetails();
      toast(t("common.saved"), "ok");
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setModeBusy(false);
    }
  }

  async function resetKeys() {
    setConfirmReset(false);
    setResetting(true);
    try {
      await api.warpResetKeys();
      await Promise.all([refreshStatus(), loadDetails()]);
      toast(t("warp.resetDone"), "ok");
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setResetting(false);
    }
  }

  return (
    <div className="mx-auto max-w-3xl">
      <PageHeader title={t("warp.title")} description={t("warp.description")} />

      {!installed ? (
        <Card className="flex flex-col gap-4">
          <div>
            <div className="text-sm font-semibold text-slate-800 dark:text-slate-100">
              {t("warp.notInstalledTitle")}
            </div>
            <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">
              {t("warp.notInstalledText")}
            </p>
          </div>
          <div>
            <Button variant="primary" onClick={() => openUrl("https://1.1.1.1/").catch(() => {})}>
              {t("warp.download")}
            </Button>
          </div>
        </Card>
      ) : (
        <>
          <Card className="mb-4">
            <FieldRow
              title={t("warp.title")}
              description={
                details?.mode ? `${t("warp.mode")}: ${details.mode}` : undefined
              }
            >
              <div className="flex items-center gap-3">
                <Badge tone={action ? "warn" : connected ? "ok" : "off"}>
                  {action === "starting"
                    ? t("home.statusStarting")
                    : action === "stopping"
                      ? t("home.statusStopping")
                      : connected
                        ? t("warp.connected")
                        : t("warp.disconnected")}
                </Badge>
                <Switch
                  checked={action === "starting" || (action !== "stopping" && connected)}
                  disabled={action !== null || (!connected && !zapretOn)}
                  onChange={toggle}
                />
              </div>
            </FieldRow>

            {!zapretOn && !connected && (
              <Note tone="warn">{t("warp.needZapret")}</Note>
            )}

            <div className="mt-4">
              <div className="mb-2 text-sm font-semibold text-slate-800 dark:text-slate-100">
                {t("warp.mode")}
              </div>
              <Segmented<WarpMode>
                value={mode}
                disabled={modeBusy}
                onChange={changeMode}
                options={[
                  { value: "warp", label: t("warp.modeWarp") },
                  { value: "warp+doh", label: t("warp.modeWarpDoh") },
                  { value: "doh", label: t("warp.modeDoh") },
                ]}
              />
            </div>
          </Card>

          <Card className="mb-4">
            <FieldRow title={t("warp.registration")} description={t("warp.resetKeysDesc")}>
              <div className="flex items-center gap-3">
                <Badge tone={details?.registered ? "ok" : "warn"}>
                  {details?.registered ? t("warp.registered") : t("warp.notRegistered")}
                </Badge>
                <Button variant="danger" disabled={resetting} onClick={() => setConfirmReset(true)}>
                  {resetting ? <Spinner /> : null}
                  {t("warp.resetKeys")}
                </Button>
              </div>
            </FieldRow>
          </Card>

          <div className="grid gap-3 md:grid-cols-2">
            <Note tone="info" title={t("warp.worksTogetherTitle")}>
              {t("warp.dependencyNote")}
            </Note>
            <Note tone="warn" title={t("warp.connectionTipsTitle")}>
              {t("warp.conflictNote")}
            </Note>
          </div>
        </>
      )}

      <Modal
        open={confirmReset}
        onClose={() => setConfirmReset(false)}
        title={t("warp.resetConfirmTitle")}
        footer={
          <>
            <Button variant="ghost" onClick={() => setConfirmReset(false)}>
              {t("common.cancel")}
            </Button>
            <Button variant="danger" onClick={resetKeys}>
              {t("warp.resetKeys")}
            </Button>
          </>
        }
      >
        {t("warp.resetConfirmText")}
      </Modal>
    </div>
  );
}
