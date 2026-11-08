import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../lib/api";
import { errText } from "../lib/errors";
import { formatClock } from "../lib/format";
import { HOME_SERVICES, useStore } from "../lib/store";
import { waitForStatus } from "../lib/status";
import type { ServiceHealth } from "../lib/types";
import { Icon, type IconName } from "../components/icons";
import { PowerButton } from "../components/PowerButton";
import { StrategyPickerModal } from "../components/StrategyPickerModal";
import { Badge, Button, Panel, Spinner, StatusDot, type Tone } from "../components/ui";

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

function Tile({
  label,
  on,
  busy,
  disabled,
  icon,
  onClick,
  extra,
}: {
  label: string;
  on: boolean;
  busy: boolean;
  disabled: boolean;
  icon: IconName;
  onClick: () => void;
  extra?: ReactNode;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex w-28 flex-col items-center gap-2 text-center">
      <PowerButton size="md" icon={icon} on={on} busy={busy} disabled={disabled} label={label} onClick={onClick} />
      <div className="text-[13px] font-medium">{label}</div>
      <div className={on ? "text-xs text-ok" : "text-xs text-muted"}>{on ? t("home.tileOn") : t("home.tileOff")}</div>
      {extra}
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
    <div className="space-y-7">
      <section className="flex flex-col items-center gap-4 text-center">
        <PowerButton
          on={zapretAction ? zapretAction === "starting" : zapretOn}
          busy={zapretAction !== null}
          disabled={!components?.zapretInstalled && !zapretOn}
          label="Zapret"
          onClick={() => void toggleZapret(!zapretOn)}
        />
        <div className="min-h-9">
          <h1 className="text-2xl font-semibold leading-tight">{title}</h1>
          <p className={error ? "mt-1.5 max-w-md text-sm text-fail" : "mt-1.5 text-sm text-muted"}>
            {error ?? (viaService ? t("home.viaService", { name: strategyName }) : null)}
          </p>
        </div>
        <button
          type="button"
          disabled={viaService}
          onClick={() => setPickerOpen(true)}
          className="flex min-w-64 items-center justify-between gap-4 rounded-panel border border-line bg-panel px-4 py-2.5 text-left transition-colors enabled:hover:border-line-strong disabled:opacity-50"
        >
          <span className="min-w-0">
            <span className="block text-xs text-muted">{t("home.strategyLabel")}</span>
            <span className="block truncate text-sm font-medium">{strategyName}</span>
          </span>
          <Icon name="chevronDown" size={16} className="shrink-0 text-muted" />
        </button>
      </section>

      <section className="flex items-start justify-center gap-12">
        <Tile
          label={t("home.tgShort")}
          on={tgAction ? tgAction === "starting" : !!tg?.running}
          busy={tgAction !== null}
          disabled={!components?.tgInstalled && !tg?.running}
          icon="send"
          onClick={() => void toggleTg(!tg?.running)}
        />
        <Tile
          label={t("home.warpShort")}
          on={warpAction ? warpAction === "starting" : !!warp?.connected}
          busy={warpAction !== null}
          disabled={!warp?.installed && !warp?.connected}
          icon="cloud"
          onClick={() => void toggleWarp(!warp?.connected)}
        />
        <Tile
          label={t("home.autopilotTitle")}
          on={!!settings?.autopilot.enabled}
          busy={false}
          disabled={!settings}
          icon="activity"
          onClick={() =>
            settings &&
            void updateSettings({ autopilot: { ...settings.autopilot, enabled: !settings.autopilot.enabled } })
          }
          extra={
            <button
              type="button"
              onClick={() => setPage("autopilot")}
              className="text-xs text-muted underline-offset-2 hover:text-fg hover:underline"
            >
              {t("home.configure")}
            </button>
          }
        />
      </section>

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
