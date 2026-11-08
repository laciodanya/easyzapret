import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../lib/api";
import { errText } from "../lib/errors";
import { formatClock } from "../lib/format";
import { HOME_SERVICES, useStore } from "../lib/store";
import { waitForStatus } from "../lib/status";
import type { ServiceHealth } from "../lib/types";
import { StrategyPickerModal } from "../components/StrategyPickerModal";
import { Badge, Button, Panel, Row, Spinner, StatusDot, Switch, type Tone } from "../components/ui";

type ToggleAction = "starting" | "stopping" | null;

const SERVICE_TONE: Record<ServiceHealth["state"], Tone> = {
  ok: "ok",
  partial: "warn",
  down: "fail",
};

function ServiceRow({ id, health }: { id: string; health?: ServiceHealth }) {
  const { t } = useTranslation();

  const detail = (() => {
    if (!health) return null;
    if (health.failed.length > 0) {
      return t("home.failedTargets", { list: health.failed.slice(0, 3).join(", ") });
    }
    if (health.bulkBlocked) return t("home.bulkBlocked");
    return null;
  })();

  return (
    <div className="flex items-center gap-4 py-3">
      <StatusDot tone={health ? SERVICE_TONE[health.state] : "off"} size={9} />
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium">{t(`services.${id}`)}</div>
        {detail && <div className="mt-0.5 truncate text-xs text-muted">{detail}</div>}
      </div>
      {health ? (
        <>
          <span className="text-[13px] tabular-nums text-muted">
            {health.avgMs !== null ? t("common.ms", { value: health.avgMs }) : ""}
          </span>
          <span className="w-12 text-right text-[13px] font-medium tabular-nums">
            {health.ok}/{health.total}
          </span>
          <div className="w-28 text-right">
            <Badge tone={SERVICE_TONE[health.state]}>{t(`home.svc_${health.state}`)}</Badge>
          </div>
        </>
      ) : (
        <span className="text-[13px] text-faint">{t("home.notChecked")}</span>
      )}
    </div>
  );
}

export function HomePage() {
  const { t } = useTranslation();
  const {
    status,
    settings,
    components,
    updates,
    appUpdate,
    updatesCheckedAt,
    updatesError,
    checkUpdates,
    setPage,
    updateSettings,
    serviceHealth,
    serviceCheckedAt,
    serviceChecking,
    checkServices,
  } = useStore();

  const [zapretAction, setZapretAction] = useState<ToggleAction>(null);
  const [tgAction, setTgAction] = useState<ToggleAction>(null);
  const [warpAction, setWarpAction] = useState<ToggleAction>(null);
  const [error, setError] = useState<string | null>(null);
  const [checkingUpdates, setCheckingUpdates] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);

  const zapret = status?.zapret;
  const tg = status?.tg;
  const warp = status?.warp;
  const zapretOn = !!(zapret?.running || zapret?.serviceState === "RUNNING");
  const viaService = !zapret?.running && zapret?.serviceState === "RUNNING";
  const strategyName = viaService
    ? (zapret?.serviceStrategy ?? t("home.noStrategy"))
    : (settings?.selectedStrategy?.replace(/\.bat$/i, "") ?? t("home.noStrategy"));

  // Re-check reachability shortly after Zapret comes up, unless a recent result exists.
  useEffect(() => {
    if (!zapretOn) return;
    const fresh = serviceCheckedAt && Date.now() - serviceCheckedAt.getTime() < 10 * 60_000;
    if (fresh) return;
    const timer = window.setTimeout(() => void checkServices(), 3500);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [zapretOn]);

  async function toggleZapret(value: boolean) {
    setError(null);
    if (!components?.zapretInstalled) {
      setError(t("errors.not_installed"));
      setPage("settings");
      return;
    }
    setZapretAction(value ? "starting" : "stopping");
    try {
      if (value) {
        const strategy = settings?.selectedStrategy;
        if (!strategy) {
          setError(t("home.noStrategy"));
          setPickerOpen(true);
          return;
        }
        await api.startZapret(strategy);
      } else {
        await api.stopZapret();
      }
      const ready = await waitForStatus(
        (next) =>
          value
            ? next.zapret.running || next.zapret.serviceState === "RUNNING"
            : !next.zapret.running && next.zapret.serviceState !== "RUNNING",
        (next) => useStore.setState({ status: next }),
        30_000,
      );
      if (!ready) setError(t(value ? "home.startTimeout" : "home.stopTimeout"));
    } catch (e) {
      setError(errText(t, e));
    } finally {
      setZapretAction(null);
    }
  }

  async function toggleTg(value: boolean) {
    setError(null);
    if (!components?.tgInstalled) {
      setError(t("errors.not_installed"));
      setPage("telegram");
      return;
    }
    setTgAction(value ? "starting" : "stopping");
    try {
      if (value) await api.startTg();
      else await api.stopTg();
      const ready = await waitForStatus(
        (next) => next.tg.running === value,
        (next) => useStore.setState({ status: next }),
        20_000,
      );
      if (!ready) setError(t(value ? "home.startTimeout" : "home.stopTimeout"));
    } catch (e) {
      setError(errText(t, e));
    } finally {
      setTgAction(null);
    }
  }

  async function toggleWarp(value: boolean) {
    setError(null);
    if (value && !warp?.installed) {
      setError(t("errors.warp_not_installed"));
      setPage("warp");
      return;
    }
    if (value && !zapretOn) {
      setError(t("warp.needZapret"));
      return;
    }
    setWarpAction(value ? "starting" : "stopping");
    try {
      if (value) await api.warpConnect();
      else await api.warpDisconnect();
      const ready = await waitForStatus(
        (next) => next.warp.connected === value,
        (next) => useStore.setState({ status: next }),
        40_000,
      );
      if (!ready) setError(t(value ? "home.startTimeout" : "home.stopTimeout"));
    } catch (e) {
      setError(errText(t, e));
    } finally {
      setWarpAction(null);
    }
  }

  async function onCheckUpdates() {
    setCheckingUpdates(true);
    try {
      await checkUpdates();
    } finally {
      setCheckingUpdates(false);
    }
  }

  const updatesAvailable = !!(updates?.some((u) => u.updateAvailable) || appUpdate?.updateAvailable);
  const updatesLabel = !updatesCheckedAt
    ? t("home.updatesUnknown")
    : updatesError
      ? t("home.updatesError")
      : updatesAvailable
        ? t("home.updatesAvailable")
        : t("home.updatesNone");

  const title =
    zapretAction === "starting"
      ? t("home.statusStarting")
      : zapretAction === "stopping"
        ? t("home.statusStopping")
        : zapretOn
          ? t("home.statusOn")
          : t("home.statusOff");

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between gap-6 pb-2">
        <div className="min-w-0">
          <div className="flex items-center gap-3">
            <StatusDot tone={zapretOn ? "ok" : "off"} size={11} />
            <h1 className="text-[28px] font-semibold leading-none">{title}</h1>
          </div>
          <p className={error ? "mt-3 text-sm text-fail" : "mt-3 truncate text-sm text-muted"}>
            {error ??
              (viaService
                ? t("home.viaService", { name: strategyName })
                : t("home.strategyLine", { name: strategyName }))}
          </p>
        </div>
        <div className="flex shrink-0 gap-2">
          <Button size="lg" onClick={() => setPickerOpen(true)} disabled={viaService}>
            {t("home.changeStrategy")}
          </Button>
          <Button
            size="lg"
            variant={zapretOn ? "secondary" : "primary"}
            icon="power"
            disabled={zapretAction !== null || (!components?.zapretInstalled && !zapretOn)}
            onClick={() => void toggleZapret(!zapretOn)}
          >
            {zapretOn ? t("home.stop") : t("home.start")}
          </Button>
        </div>
      </div>

      <Panel
        title={t("home.servicesTitle")}
        description={
          serviceCheckedAt
            ? t("home.checkedAt", { time: formatClock(serviceCheckedAt) })
            : t("home.servicesDesc")
        }
        action={
          <Button
            size="sm"
            icon={serviceChecking ? undefined : "refresh"}
            disabled={serviceChecking}
            onClick={() => void checkServices()}
          >
            {serviceChecking && <Spinner size={14} />}
            {serviceChecking ? t("home.checking") : t("home.check")}
          </Button>
        }
      >
        {HOME_SERVICES.map((id) => (
          <ServiceRow key={id} id={id} health={serviceHealth[id]} />
        ))}
      </Panel>

      <Panel title={t("home.extrasTitle")}>
        <Row title={t("home.tgTitle")} description={t("home.tgDesc")}>
          {tgAction && <Spinner size={14} />}
          <Switch
            label={t("home.tgTitle")}
            checked={tgAction ? tgAction === "starting" : !!tg?.running}
            disabled={tgAction !== null || (!components?.tgInstalled && !tg?.running)}
            onChange={(v) => void toggleTg(v)}
          />
        </Row>
        <Row title={t("home.warpTitle")} description={t("home.warpDesc")}>
          {warpAction && <Spinner size={14} />}
          <Switch
            label={t("home.warpTitle")}
            checked={warpAction ? warpAction === "starting" : !!warp?.connected}
            disabled={warpAction !== null || (!warp?.installed && !warp?.connected)}
            onChange={(v) => void toggleWarp(v)}
          />
        </Row>
        <Row title={t("home.autopilotTitle")} description={t("home.autopilotDesc")}>
          <Button variant="ghost" size="sm" onClick={() => setPage("autopilot")}>
            {t("home.configure")}
          </Button>
          <Switch
            label={t("home.autopilotTitle")}
            checked={!!settings?.autopilot.enabled}
            disabled={!settings}
            onChange={(v) =>
              settings && void updateSettings({ autopilot: { ...settings.autopilot, enabled: v } })
            }
          />
        </Row>
      </Panel>

      <div className="flex items-center justify-between gap-4 px-1 text-[13px]">
        <span className={updatesAvailable ? "font-medium text-warn" : "text-muted"}>
          {t("home.lastUpdateCheck", { result: updatesLabel })}
          {updatesCheckedAt && (
            <span className="ml-1.5 text-faint">{formatClock(updatesCheckedAt)}</span>
          )}
        </span>
        <Button
          size="sm"
          variant={updatesAvailable ? "primary" : "ghost"}
          disabled={checkingUpdates}
          onClick={() => void onCheckUpdates()}
        >
          {checkingUpdates && <Spinner size={14} />}
          {updatesAvailable ? t("common.update") : t("home.checkNow")}
        </Button>
      </div>

      <StrategyPickerModal open={pickerOpen} onClose={() => setPickerOpen(false)} />
    </div>
  );
}
