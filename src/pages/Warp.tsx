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
  ConfirmModal,
  Note,
  PageHeader,
  Panel,
  Row,
  Segmented,
  Spinner,
  Switch,
} from "../components/ui";

type WarpMode = "warp" | "warp+doh" | "doh";

const SELECTABLE_MODES: readonly string[] = ["warp", "warp+doh", "doh"];

/** The client may report modes this screen cannot set (proxy, dot, ...). */
function asSelectable(mode: string | null | undefined): WarpMode | null {
  return mode && SELECTABLE_MODES.includes(mode) ? (mode as WarpMode) : null;
}

export function WarpPage() {
  const { t } = useTranslation();
  const { status, refreshStatus } = useStore();

  const [details, setDetails] = useState<WarpStatus | null>(null);
  const [action, setAction] = useState<"starting" | "stopping" | null>(null);
  const [pendingMode, setPendingMode] = useState<WarpMode | null>(null);
  const [resetting, setResetting] = useState(false);
  const [confirmReset, setConfirmReset] = useState(false);

  const warp = status?.warp;
  const installed = warp?.installed ?? details?.installed ?? false;
  const connected = warp?.connected ?? false;
  const zapretOn = !!(status?.zapret.running || status?.zapret.serviceState === "RUNNING");

  // What the client actually reports wins; while a change is being applied we show the target.
  const reportedMode = details?.mode ?? null;
  const shownMode = pendingMode ?? asSelectable(reportedMode);

  const loadDetails = useCallback(async () => {
    try {
      setDetails(await api.warpDetails());
    } catch {
      // The polled quick status still drives the rest of the screen.
    }
  }, []);

  // The mode can also change outside the app, so keep the screen in sync with the client.
  useEffect(() => {
    void loadDetails();
    const timer = window.setInterval(() => void loadDetails(), 5000);
    return () => window.clearInterval(timer);
  }, [loadDetails]);

  async function toggle(value: boolean) {
    if (value && !zapretOn) {
      toast(t("warp.needZapret"), "fail");
      return;
    }
    setAction(value ? "starting" : "stopping");
    try {
      if (value) await api.warpConnect();
      else await api.warpDisconnect();
      const ready = await waitForStatus(
        (next) => next.warp.connected === value,
        (next) => useStore.setState({ status: next }),
        40_000,
      );
      if (!ready) toast(t(value ? "home.startTimeout" : "home.stopTimeout"), "info");
      await loadDetails();
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setAction(null);
    }
  }

  async function changeMode(next: WarpMode) {
    if (next === shownMode) return;
    setPendingMode(next);
    try {
      const applied = await api.warpSetMode(next);
      setDetails((prev) => (prev ? { ...prev, mode: applied } : prev));
      if (applied !== next) toast(t("warp.modeMismatch", { mode: applied }), "info");
      else toast(t("common.saved"), "ok");
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setPendingMode(null);
      await loadDetails();
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
    <div className="space-y-6">
      <PageHeader title={t("warp.title")} description={t("warp.description")} />

      {!installed ? (
        <Panel>
          <Row title={t("warp.notInstalledTitle")} description={t("warp.notInstalledText")}>
            <Button variant="primary" onClick={() => openUrl("https://1.1.1.1/").catch(() => {})}>
              {t("warp.download")}
            </Button>
          </Row>
        </Panel>
      ) : (
        <>
          <Panel>
            <Row title={t("warp.connection")}>
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
                label={t("warp.connection")}
                checked={action === "starting" || (action !== "stopping" && connected)}
                disabled={action !== null || (!connected && !zapretOn)}
                onChange={(v) => void toggle(v)}
              />
            </Row>
            <Row title={t("warp.mode")} description={t("warp.modeDesc")}>
              {pendingMode && <Spinner size={14} />}
              <Segmented<WarpMode>
                value={shownMode}
                disabled={pendingMode !== null}
                onChange={(m) => void changeMode(m)}
                options={[
                  { value: "warp", label: t("warp.modeWarp") },
                  { value: "warp+doh", label: t("warp.modeWarpDoh") },
                  { value: "doh", label: t("warp.modeDoh") },
                ]}
              />
            </Row>
          </Panel>

          {!zapretOn && !connected && <Note tone="warn">{t("warp.needZapret")}</Note>}
          {reportedMode && !asSelectable(reportedMode) && (
            <Note>{t("warp.otherMode", { mode: reportedMode })}</Note>
          )}

          <Panel>
            <Row title={t("warp.registration")} description={t("warp.resetKeysDesc")}>
              <Badge tone={details?.registered ? "ok" : "warn"}>
                {details?.registered ? t("warp.registered") : t("warp.notRegistered")}
              </Badge>
              <Button variant="danger" disabled={resetting} onClick={() => setConfirmReset(true)}>
                {resetting && <Spinner size={14} />}
                {t("warp.resetKeys")}
              </Button>
            </Row>
          </Panel>

          <div className="grid gap-3 md:grid-cols-2">
            <Note title={t("warp.worksTogetherTitle")}>{t("warp.dependencyNote")}</Note>
            <Note tone="warn" title={t("warp.connectionTipsTitle")}>
              {t("warp.conflictNote")}
            </Note>
          </div>
        </>
      )}

      <ConfirmModal
        open={confirmReset}
        title={t("warp.resetConfirmTitle")}
        tone="danger"
        confirmLabel={t("warp.resetKeys")}
        onClose={() => setConfirmReset(false)}
        onConfirm={() => void resetKeys()}
      >
        {t("warp.resetConfirmText")}
      </ConfirmModal>
    </div>
  );
}
