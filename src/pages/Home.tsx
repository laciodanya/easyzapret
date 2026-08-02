import { useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../lib/api";
import { errText } from "../lib/errors";
import { useStore } from "../lib/store";
import { waitForStatus } from "../lib/status";
import { Badge, Button, Card, Spinner, Switch } from "../components/ui";

type ToggleAction = "starting" | "stopping" | null;

function BigToggleCard({
  icon,
  title,
  description,
  on,
  action,
  error,
  subtitle,
  disabled,
  onToggle,
  footer,
}: {
  icon: React.ReactNode;
  title: string;
  description: string;
  on: boolean;
  action: ToggleAction;
  error?: string | null;
  subtitle?: React.ReactNode;
  disabled?: boolean;
  onToggle: (value: boolean) => void;
  footer?: React.ReactNode;
}) {
  const { t } = useTranslation();
  const busy = action !== null;
  const displayOn = action === "starting" || (action !== "stopping" && on);
  const tone = error ? "fail" : busy ? "warn" : displayOn ? "ok" : "off";
  const label = error
    ? t("home.statusError")
    : action === "starting"
      ? t("home.statusStarting")
      : action === "stopping"
        ? t("home.statusStopping")
      : displayOn
        ? t("home.statusConnected")
        : t("home.statusDisconnected");

  return (
    <Card className="flex flex-col gap-4 p-6">
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-center gap-4">
          <div
            className={
              "flex h-14 w-14 items-center justify-center rounded-2xl transition-colors " +
              (displayOn
                ? "bg-accent-soft text-accent"
                : "bg-slate-100 text-slate-400 dark:bg-slate-800")
            }
          >
            {icon}
          </div>
          <div>
            <div className="flex items-center gap-2.5">
              <h2 className="text-xl font-bold text-slate-900 dark:text-white">{title}</h2>
              <Badge tone={tone}>{busy ? <Spinner className="h-3 w-3" /> : null}{label}</Badge>
            </div>
            <p className="mt-1 text-sm text-slate-500 dark:text-slate-400">{description}</p>
          </div>
        </div>
        <Switch size="lg" checked={displayOn} disabled={busy || disabled} onChange={onToggle} />
      </div>
      {subtitle && <div className="text-sm text-slate-500 dark:text-slate-400">{subtitle}</div>}
      {error && (
        <div className="rounded-xl bg-red-500/10 px-3.5 py-2.5 text-xs leading-relaxed text-red-700 ring-1 ring-red-500/25 dark:text-red-300">
          {error}
        </div>
      )}
      {footer}
    </Card>
  );
}

export function HomePage() {
  const { t } = useTranslation();
  const { status, settings, components, updates, appUpdate, updatesCheckedAt, updatesError, checkUpdates, setPage, updateSettings } =
    useStore();

  const [zapretAction, setZapretAction] = useState<ToggleAction>(null);
  const [tgAction, setTgAction] = useState<ToggleAction>(null);
  const [warpAction, setWarpAction] = useState<ToggleAction>(null);
  const [zapretError, setZapretError] = useState<string | null>(null);
  const [tgError, setTgError] = useState<string | null>(null);
  const [warpError, setWarpError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);

  const zapret = status?.zapret;
  const tg = status?.tg;
  const warp = status?.warp;
  const zapretOn = !!(zapret?.running || zapret?.serviceState === "RUNNING");
  const viaService = !zapret?.running && zapret?.serviceState === "RUNNING";

  async function toggleZapret(value: boolean) {
    setZapretError(null);
    if (!components?.zapretInstalled) {
      setZapretError(t("errors.not_installed"));
      setPage("settings");
      return;
    }
    setZapretAction(value ? "starting" : "stopping");
    try {
      if (value) {
        const strategy = settings?.selectedStrategy;
        if (!strategy) {
          setZapretError(t("home.noStrategy"));
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
      if (!ready) setZapretError(t(value ? "home.startTimeout" : "home.stopTimeout"));
    } catch (e) {
      setZapretError(errText(t, e));
    } finally {
      setZapretAction(null);
    }
  }

  async function toggleTg(value: boolean) {
    setTgError(null);
    if (!components?.tgInstalled) {
      setTgError(t("errors.not_installed"));
      setPage("telegram");
      return;
    }
    setTgAction(value ? "starting" : "stopping");
    try {
      if (value) {
        await api.startTg();
      } else {
        await api.stopTg();
      }
      const ready = await waitForStatus(
        (next) => next.tg.running === value,
        (next) => useStore.setState({ status: next }),
        20_000,
      );
      if (!ready) setTgError(t(value ? "home.startTimeout" : "home.stopTimeout"));
    } catch (e) {
      setTgError(errText(t, e));
    } finally {
      setTgAction(null);
    }
  }

  async function toggleWarp(value: boolean) {
    setWarpError(null);
    if (value && !warp?.installed) {
      setWarpError(t("errors.warp_not_installed"));
      setPage("warp");
      return;
    }
    if (value && !zapretOn) {
      setWarpError(t("warp.needZapret"));
      return;
    }
    setWarpAction(value ? "starting" : "stopping");
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
      if (!ready) setWarpError(t(value ? "home.startTimeout" : "home.stopTimeout"));
    } catch (e) {
      setWarpError(errText(t, e));
    } finally {
      setWarpAction(null);
    }
  }

  async function onCheckUpdates() {
    setChecking(true);
    try {
      await checkUpdates();
    } finally {
      setChecking(false);
    }
  }

  const updatesAvailable = updates?.some((u) => u.updateAvailable) || appUpdate?.updateAvailable;
  const updatesLabel = !updatesCheckedAt
    ? t("home.updatesUnknown")
    : updatesError
      ? t("home.updatesError")
      : updatesAvailable
        ? t("home.updatesAvailable")
        : t("home.updatesNone");

  return (
    <div className="mx-auto flex h-full max-w-3xl flex-col gap-5">
      {settings && (
        <Card className="flex flex-wrap items-center justify-between gap-4 p-4">
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-accent-soft text-accent">
              <svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z" />
              </svg>
            </div>
            <div>
              <div className="flex items-center gap-2 text-sm font-bold text-slate-900 dark:text-white">
                {t("home.autopilotTitle")}
                {status?.autopilot.enabled && (
                  <Badge tone={status.autopilot.checking ? "warn" : "ok"}>
                    {status.autopilot.checking ? t("home.autopilotChecking") : t("home.autopilotOn")}
                  </Badge>
                )}
              </div>
              <p className="text-xs text-slate-500 dark:text-slate-400">
                {status?.autopilot.lastHealthPercent != null
                  ? t("home.autopilotHealth", { percent: status.autopilot.lastHealthPercent })
                  : t("home.autopilotDesc")}
              </p>
            </div>
          </div>
          <div className="flex items-center gap-3">
            <Switch
              checked={settings.autopilot.enabled}
              onChange={(v) =>
                updateSettings({ autopilot: { ...settings.autopilot, enabled: v } })
              }
            />
            <Button variant="ghost" onClick={() => setPage("autopilot")}>
              {t("common.details")}
            </Button>
          </div>
        </Card>
      )}

      <BigToggleCard
        icon={
          <svg className="h-7 w-7" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.7}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M9 12.75L11.25 15 15 9.75m-3-7.036A11.959 11.959 0 013.598 6 11.99 11.99 0 003 9.749c0 5.592 3.824 10.29 9 11.623 5.176-1.332 9-6.03 9-11.622 0-1.31-.21-2.571-.598-3.751h-.152c-3.196 0-6.1-1.248-8.25-3.285z" />
          </svg>
        }
        title={t("home.zapretTitle")}
        description={t("home.zapretDesc")}
        on={zapretOn}
        action={zapretAction}
        error={zapretError}
        disabled={!components?.zapretInstalled && !zapretOn}
        onToggle={toggleZapret}
        subtitle={
          <span className="flex flex-wrap items-center gap-x-2 gap-y-1">
            <span className="font-medium text-slate-600 dark:text-slate-300">
              {t("home.strategy")}:
            </span>
            <button
              className="text-accent underline-offset-2 hover:underline"
              onClick={() => setPage("strategies")}
            >
              {viaService
                ? (zapret?.serviceStrategy ?? t("home.noStrategy"))
                : (settings?.selectedStrategy?.replace(/\.bat$/i, "") ?? t("home.noStrategy"))}
            </button>
            {viaService && <Badge tone="info">{t("home.viaService")}</Badge>}
            {!components?.zapretInstalled && <Badge tone="warn">{t("home.installFirst")}</Badge>}
          </span>
        }
      />

      <BigToggleCard
        icon={
          <svg className="h-7 w-7" fill="currentColor" viewBox="0 0 24 24">
            <path d="M21.5 4.5L2.8 11.7c-.8.3-.8 1.4.05 1.7l4.7 1.6 1.8 5.5c.25.75 1.2.9 1.7.3l2.5-2.9 4.9 3.6c.6.45 1.5.1 1.65-.65l3-15c.2-.9-.7-1.65-1.6-1.35z" />
          </svg>
        }
        title={t("home.tgTitle")}
        description={t("home.tgDesc")}
        on={!!tg?.running}
        action={tgAction}
        error={tgError}
        disabled={!components?.tgInstalled && !tg?.running}
        onToggle={toggleTg}
        subtitle={
          <span className="flex flex-wrap items-center gap-x-2 gap-y-1">
            <span className="font-medium text-slate-600 dark:text-slate-300">
              {t("telegram.server")}:
            </span>
            <code className="rounded bg-slate-100 px-1.5 py-0.5 text-xs dark:bg-slate-800">
              {tg?.host ?? "127.0.0.1"}:{tg?.port ?? 1443}
            </code>
            <button
              className="text-accent underline-offset-2 hover:underline"
              onClick={() => setPage("telegram")}
            >
              {t("common.details")}
            </button>
            {!components?.tgInstalled && <Badge tone="warn">{t("home.installFirst")}</Badge>}
          </span>
        }
      />

      <BigToggleCard
        icon={
          <svg className="h-7 w-7" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.7}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M3 15a4 4 0 004 4h9a5 5 0 10-.1-9.999 5.002 5.002 0 10-9.78 2.096A4.001 4.001 0 003 15z" />
          </svg>
        }
        title={t("home.warpTitle")}
        description={t("home.warpDesc")}
        on={!!warp?.connected}
        action={warpAction}
        error={warpError}
        disabled={!warp?.installed && !warp?.connected}
        onToggle={toggleWarp}
        subtitle={
          <span className="flex flex-wrap items-center gap-x-2 gap-y-1">
            {!zapretOn && !warp?.connected ? (
              <Badge tone="warn">{t("warp.needZapret")}</Badge>
            ) : (
              <span className="font-medium text-slate-600 dark:text-slate-300">
                {t("warp.dependencyNote")}
              </span>
            )}
            <button
              className="text-accent underline-offset-2 hover:underline"
              onClick={() => setPage("warp")}
            >
              {t("common.details")}
            </button>
            {!warp?.installed && <Badge tone="warn">{t("common.notInstalled")}</Badge>}
          </span>
        }
      />

      <div className="mt-auto flex items-center justify-between rounded-2xl bg-white/60 px-5 py-3.5 text-sm ring-1 ring-slate-200/70 dark:bg-slate-900/60 dark:ring-slate-800">
        <span
          className={
            updatesAvailable
              ? "font-medium text-amber-600 dark:text-amber-400"
              : "text-slate-500 dark:text-slate-400"
          }
        >
          {t("home.lastUpdateCheck", { result: updatesLabel })}
          {updatesCheckedAt && (
            <span className="ml-1 text-xs text-slate-400">
              ({updatesCheckedAt.toLocaleTimeString()})
            </span>
          )}
        </span>
        <Button variant="ghost" onClick={onCheckUpdates} disabled={checking}>
          {checking ? <Spinner /> : null}
          {t("home.checkNow")}
        </Button>
      </div>
    </div>
  );
}
