import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../lib/api";
import { errText } from "../lib/errors";
import { formatClock } from "../lib/format";
import { HOME_SERVICES, useStore } from "../lib/store";
import { waitForStatus } from "../lib/status";
import type { ServiceHealth } from "../lib/types";
import type { VpnDetails } from "../lib/vpn";
import { Icon, type IconName } from "../components/icons";
import { PowerButton } from "../components/PowerButton";
import { StrategyPickerModal } from "../components/StrategyPickerModal";
import { VpnServerButton, VpnServerPicker } from "../components/VpnServerPicker";
import { IconButton, StatusDot, type Tone } from "../components/ui";

type ToggleAction = "starting" | "stopping" | null;

const SERVICE_TONE: Record<ServiceHealth["state"], Tone> = {
  ok: "ok",
  partial: "warn",
  down: "fail",
};

function ServiceItem({ id, health }: { id: string; health?: ServiceHealth }) {
  const { t } = useTranslation();
  return (
    <div className="flex items-center gap-2.5 text-[13px]">
      <StatusDot tone={health ? SERVICE_TONE[health.state] : "off"} size={7} />
      <span className={health ? "text-fg" : "text-muted"}>{t(`services.${id}`)}</span>
      <span className="ml-auto tabular-nums text-faint">
        {health ? `${health.ok}/${health.total}` : "—"}
      </span>
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
  return (
    <div className="flex w-28 flex-col items-center gap-2 text-center">
      <PowerButton size="md" icon={icon} on={on} busy={busy} disabled={disabled} label={label} onClick={onClick} />
      <div className="text-[13px] font-medium">{label}</div>
      {extra}
    </div>
  );
}

export function HomePage() {
  const { t } = useTranslation();
  const {
    status,
    appInfo,
    vpnInstalling,
    refreshStatus,
    settings,
    components,
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
  const [vpnAction, setVpnAction] = useState<ToggleAction>(null);
  const [vpnDetails, setVpnDetails] = useState<VpnDetails | null>(null);
  const [vpnPickerOpen, setVpnPickerOpen] = useState(false);
  const [vpnPicking, setVpnPicking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pickerOpen, setPickerOpen] = useState(false);

  const zapret = status?.zapret;
  const tg = status?.tg;
  const warp = status?.warp;
  const vpn = status?.vpn;
  const vpnOn = !!vpn?.running;
  const vpnInstalled = vpn?.installed ?? vpnDetails?.status.installed ?? false;
  const selectedServer = vpnDetails?.profiles.find((server) => server.id === vpnDetails.selectedProfile);
  const vpnSwitching = vpnAction !== null || vpnPicking;
  const zapretOn = !!(zapret?.running || zapret?.serviceState === "RUNNING");
  const viaService = !!zapret?.serviceInstalled;
  const strategyName = viaService
    ? (zapret?.serviceStrategy ?? t("home.noStrategy"))
    : ((zapret?.currentStrategy ?? settings?.selectedStrategy)?.replace(/\.bat$/i, "") ??
      t("home.noStrategy"));

  useEffect(() => {
    let active = true;
    api.vpnDetails().then((details) => {
      if (active) setVpnDetails(details);
    }).catch(() => {});
    return () => { active = false; };
  }, []);

  function pickVpnServer() {
    if (vpnDetails?.profiles.length) setVpnPickerOpen(true);
    else setPage("vpn");
  }

  async function selectVpnServer(id: string) {
    if (id === vpnDetails?.selectedProfile) {
      setVpnPickerOpen(false);
      return;
    }
    setVpnPicking(true);
    setError(null);
    try {
      setVpnDetails(await api.vpnSelectProfile(id));
      setVpnPickerOpen(false);
    } catch (error) {
      setError(errText(t, error));
      setVpnPickerOpen(false);
    } finally {
      await refreshStatus();
      setVpnPicking(false);
    }
  }

  async function toggleVpn(value: boolean) {
    setError(null);
    if (value && (!vpnInstalled || !selectedServer)) {
      setPage("vpn");
      return;
    }
    const vpnSettings = vpnDetails?.settings;
    if (value && vpnSettings && ["only_apps", "bypass_apps"].includes(vpnSettings.mode) && !vpnSettings.apps.length) {
      setPage("vpn");
      return;
    }
    setVpnAction(value ? "starting" : "stopping");
    try {
      if (value) await api.vpnConnect();
      else await api.vpnDisconnect();
    } catch (error) {
      setError(errText(t, error));
    } finally {
      await refreshStatus();
      setVpnAction(null);
    }
  }

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
        const strategy = zapret?.currentStrategy ?? settings?.selectedStrategy;
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

  const serviceProblems = HOME_SERVICES.flatMap((id) => {
    const health = serviceHealth[id];
    if (!health || health.state === "ok") return [];
    const detail =
      health.failed.length > 0
        ? t("home.failedTargets", { list: health.failed.slice(0, 3).join(", ") })
        : health.bulkBlocked
          ? t("home.bulkBlocked")
          : t(`home.svc_${health.state}`);
    return [`${t(`services.${id}`)}: ${detail}`];
  });

  return (
    <div className="space-y-7">
      <section className="mx-auto grid max-w-2xl grid-cols-2 items-start gap-6 text-center">
        <div className="flex min-w-0 flex-col items-center gap-4">
          <PowerButton
            on={zapretAction ? zapretAction === "starting" : zapretOn}
            busy={zapretAction !== null}
            disabled={vpnOn || vpnSwitching || (!components?.zapretInstalled && !zapretOn)}
            label="Zapret"
            onClick={() => void toggleZapret(!zapretOn)}
          />
          <div className="min-h-12">
            <h1 className="text-2xl font-semibold leading-tight">Zapret</h1>
            <p className="mt-1 text-xs text-muted">
              {vpnOn ? t("vpn.stopToUseZapret") : viaService ? t("home.viaService", { name: strategyName }) : null}
            </p>
          </div>
          <button type="button" disabled={viaService || vpnSwitching} onClick={() => setPickerOpen(true)}
            className="flex min-w-0 w-full items-center justify-between gap-4 rounded-panel border border-line bg-panel px-4 py-2.5 text-left transition-colors enabled:hover:border-line-strong disabled:opacity-50">
            <span className="min-w-0">
              <span className="block text-xs text-muted">{t("home.strategyLabel")}</span>
              <span className="block truncate text-sm font-medium">{strategyName}</span>
            </span>
            <Icon name="chevronDown" size={16} className="shrink-0 text-muted" />
          </button>
        </div>
        <div className="flex min-w-0 flex-col items-center gap-4">
          <PowerButton
            on={vpnAction ? vpnAction === "starting" : vpnOn}
            busy={vpnSwitching || vpnInstalling}
            disabled={appInfo?.isWindows === false || zapretAction !== null || warpAction !== null}
            label="VPN"
            onClick={() => void toggleVpn(!vpnOn)}
          />
          <div className="min-h-12">
            <h2 className="text-2xl font-semibold leading-tight">VPN</h2>
            <p className="mt-1 text-xs text-muted">
              {t(vpnOn ? "vpn.connected" : vpnInstalling ? "vpn.installing" : !vpnInstalled ? "vpn.installTitle" : "vpn.disconnected")}
            </p>
          </div>
          <VpnServerButton name={selectedServer?.name}
            disabled={vpnSwitching || vpnInstalling || appInfo?.isWindows === false} onClick={pickVpnServer} />
        </div>
        {error && <p role="alert" className="col-span-2 -mt-2 text-sm text-fail">{error}</p>}
        {!vpnOn && (viaService || zapretOn || warp?.connected) && (
          <p className="col-span-2 -mt-2 text-xs text-muted">{t("vpn.switchToVpn")}</p>
        )}
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
          disabled={vpnOn || vpnSwitching || (!warp?.installed && !warp?.connected)}
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

      <section className="mt-14 border-t border-line pt-5">
        <div className="mb-3.5 flex items-center justify-between gap-3">
          <h2 className="text-[13px] font-medium text-muted">
            {t("home.servicesTitle")}
            {serviceCheckedAt && (
              <span className="ml-2 font-normal text-faint">
                {t("home.checkedAt", { time: formatClock(serviceCheckedAt) })}
              </span>
            )}
          </h2>
          <IconButton
            icon="refresh"
            label={serviceChecking ? t("home.checking") : t("home.check")}
            disabled={serviceChecking}
            className={serviceChecking ? "[&_svg]:animate-spin" : undefined}
            onClick={() => void checkServices()}
          />
        </div>
        <div className="grid grid-cols-2 gap-x-8 gap-y-2.5 md:grid-cols-4">
          {HOME_SERVICES.map((id) => (
            <ServiceItem key={id} id={id} health={serviceHealth[id]} />
          ))}
        </div>
        {serviceProblems.length > 0 && (
          <ul className="mt-3.5 space-y-1 text-xs text-muted">
            {serviceProblems.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        )}
      </section>

      <StrategyPickerModal open={pickerOpen} onClose={() => setPickerOpen(false)} />
      {vpnPickerOpen && vpnDetails && <VpnServerPicker details={vpnDetails} busy={vpnPicking}
        onClose={() => setVpnPickerOpen(false)} onPick={(id) => void selectVpnServer(id)} />}
    </div>
  );
}
