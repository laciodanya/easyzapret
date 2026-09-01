import { useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../lib/api";
import { errText } from "../lib/errors";
import { useStore } from "../lib/store";
import { waitForStatus } from "../lib/status";
import { StrategyPickerModal } from "../components/StrategyPickerModal";
import { Button, Spinner, Switch, cn } from "../components/ui";

type ToggleAction = "starting" | "stopping" | null;
type PowerTarget = "zapret" | "tg" | "warp";

function PowerButton({
  on,
  action,
  disabled,
  label,
  onClick,
  size = "lg",
}: {
  on: boolean;
  action: ToggleAction;
  disabled?: boolean;
  label: string;
  onClick: () => void;
  size?: "lg" | "sm";
}) {
  const busy = action !== null;
  const displayOn = action === "starting" || (action !== "stopping" && on);
  const dims = size === "lg" ? "h-40 w-40" : "h-16 w-16";
  const icon = size === "lg" ? "h-12 w-12" : "h-6 w-6";

  return (
    <button
      type="button"
      disabled={disabled || busy}
      onClick={onClick}
      aria-pressed={displayOn}
      aria-label={label}
      className={cn(
        "ez-power relative flex items-center justify-center rounded-full disabled:cursor-not-allowed disabled:opacity-50",
        dims,
        displayOn
          ? "ez-power-on bg-accent text-[rgb(var(--accent-fg))]"
          : "bg-[rgb(var(--surface-elevated))] text-[rgb(var(--text-secondary))] ring-1 ring-[rgb(var(--border)/0.7)]",
      )}
    >
      {displayOn && !busy && <span className="ez-power-halo" />}
      {busy && <span className="ez-power-ring" />}
      <span className="relative z-10 flex flex-col items-center gap-1">
        {busy ? (
          <Spinner className={cn(size === "lg" ? "h-8 w-8" : "h-5 w-5", "border-white/40 border-t-white")} />
        ) : (
          <svg className={icon} fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.8}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M5.636 5.636a9 9 0 1012.728 0M12 3v9" />
          </svg>
        )}
      </span>
    </button>
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
  } = useStore();

  const [zapretAction, setZapretAction] = useState<ToggleAction>(null);
  const [tgAction, setTgAction] = useState<ToggleAction>(null);
  const [warpAction, setWarpAction] = useState<ToggleAction>(null);
  const [zapretError, setZapretError] = useState<string | null>(null);
  const [tgError, setTgError] = useState<string | null>(null);
  const [warpError, setWarpError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [focus, setFocus] = useState<PowerTarget>("zapret");
  const [showAll, setShowAll] = useState(true);

  const zapret = status?.zapret;
  const tg = status?.tg;
  const warp = status?.warp;
  const zapretOn = !!(zapret?.running || zapret?.serviceState === "RUNNING");
  const viaService = !zapret?.running && zapret?.serviceState === "RUNNING";
  const strategyName = viaService
    ? (zapret?.serviceStrategy ?? t("home.noStrategy"))
    : (settings?.selectedStrategy?.replace(/\.bat$/i, "") ?? t("home.noStrategy"));

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
    if (value && status?.vpn?.connected) {
      setWarpError(t("errors.warp_vpn_exclusive"));
      return;
    }
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

  const statusLine =
    zapretError ||
    tgError ||
    warpError ||
    (zapretAction === "starting"
      ? t("home.statusStarting")
      : zapretAction === "stopping"
        ? t("home.statusStopping")
        : zapretOn
          ? t("home.statusConnected")
          : t("home.statusDisconnected"));

  const focusOn =
    focus === "zapret" ? zapretOn : focus === "tg" ? !!tg?.running : !!warp?.connected;
  const focusAction = focus === "zapret" ? zapretAction : focus === "tg" ? tgAction : warpAction;
  const focusLabel =
    focus === "zapret" ? t("home.zapretTitle") : focus === "tg" ? t("home.tgTitle") : t("home.warpTitle");

  function toggleFocus() {
    const next = !focusOn;
    if (focus === "zapret") void toggleZapret(next);
    else if (focus === "tg") void toggleTg(next);
    else void toggleWarp(next);
  }

  return (
    <div className="relative mx-auto flex h-full max-w-2xl flex-col">
      {settings && (
        <div className="absolute right-0 top-0 z-10 flex items-center gap-2 rounded-2xl bg-[rgb(var(--surface-elevated)/0.8)] px-3 py-2 ring-1 ring-[rgb(var(--border)/0.5)] backdrop-blur-md">
          <button
            type="button"
            className="text-xs font-medium text-[rgb(var(--text-secondary))] hover:text-accent"
            onClick={() => setPage("autopilot")}
          >
            {t("home.autopilotTitle")}
          </button>
          <Switch
            checked={settings.autopilot.enabled}
            onChange={(v) => updateSettings({ autopilot: { ...settings.autopilot, enabled: v } })}
          />
        </div>
      )}

      <div className="flex flex-1 flex-col items-center justify-center gap-8 pb-6 pt-10">
        <div className="ez-fade-up flex flex-col items-center gap-4">
          {!showAll && (
            <div className="mb-1 inline-flex rounded-full bg-[rgb(var(--surface-elevated))] p-1 ring-1 ring-[rgb(var(--border)/0.55)]">
              {(
                [
                  ["zapret", t("home.zapretShort")],
                  ["tg", t("home.tgShort")],
                  ["warp", t("home.warpShort")],
                ] as const
              ).map(([id, label]) => (
                <button
                  key={id}
                  type="button"
                  onClick={() => setFocus(id)}
                  className={cn(
                    "rounded-full px-3 py-1 text-xs font-semibold transition-colors",
                    focus === id ? "bg-accent text-[rgb(var(--accent-fg))]" : "text-[rgb(var(--text-secondary))]",
                  )}
                >
                  {label}
                </button>
              ))}
            </div>
          )}

          {showAll ? (
            <PowerButton
              on={zapretOn}
              action={zapretAction}
              disabled={!components?.zapretInstalled && !zapretOn}
              label={t("home.zapretTitle")}
              onClick={() => void toggleZapret(!zapretOn)}
            />
          ) : (
            <PowerButton
              on={focusOn}
              action={focusAction}
              label={focusLabel}
              onClick={toggleFocus}
              disabled={
                (focus === "zapret" && !components?.zapretInstalled && !zapretOn) ||
                (focus === "tg" && !components?.tgInstalled && !tg?.running) ||
                (focus === "warp" && !warp?.installed && !warp?.connected)
              }
            />
          )}

          <div className="text-center">
            <div className="text-lg font-semibold tracking-tight text-[rgb(var(--text))]">
              {showAll ? t("home.zapretTitle") : focusLabel}
            </div>
            <div
              className={cn(
                "mt-1 text-sm",
                zapretError || tgError || warpError
                  ? "text-red-500 dark:text-red-400"
                  : "text-[rgb(var(--text-secondary))]",
              )}
            >
              {statusLine}
            </div>
          </div>
        </div>

        <button
          type="button"
          onClick={() => setPickerOpen(true)}
          className="ez-fade-up-delay-1 group flex min-w-[240px] items-center justify-between gap-3 rounded-2xl bg-[rgb(var(--surface-elevated))] px-4 py-3 ring-1 ring-[rgb(var(--border)/0.55)] transition hover:ring-[rgb(var(--accent)/0.35)]"
        >
          <div className="min-w-0 text-left">
            <div className="text-[11px] font-semibold uppercase tracking-wide text-[rgb(var(--text-secondary))]">
              {t("home.strategy")}
            </div>
            <div className="truncate text-sm font-semibold text-[rgb(var(--text))]">{strategyName}</div>
          </div>
          <svg
            className="h-4 w-4 shrink-0 text-[rgb(var(--text-secondary))] transition group-hover:text-accent"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={2}
          >
            <path strokeLinecap="round" strokeLinejoin="round" d="M8 9l4-4 4 4M8 15l4 4 4-4" />
          </svg>
        </button>

        {showAll && (
          <div className="ez-fade-up-delay-2 flex items-start gap-10">
            <div className="flex flex-col items-center gap-2">
              <PowerButton
                size="sm"
                on={!!tg?.running}
                action={tgAction}
                disabled={!components?.tgInstalled && !tg?.running}
                label={t("home.tgTitle")}
                onClick={() => void toggleTg(!tg?.running)}
              />
              <span className="text-xs font-semibold text-[rgb(var(--text-secondary))]">{t("home.tgShort")}</span>
            </div>
            <div className="flex flex-col items-center gap-2">
              <PowerButton
                size="sm"
                on={!!warp?.connected}
                action={warpAction}
                disabled={!warp?.installed && !warp?.connected}
                label={t("home.warpTitle")}
                onClick={() => void toggleWarp(!warp?.connected)}
              />
              <span className="text-xs font-semibold text-[rgb(var(--text-secondary))]">{t("home.warpShort")}</span>
            </div>
          </div>
        )}

        <button
          type="button"
          className="text-[11px] font-medium text-[rgb(var(--text-secondary))] underline-offset-2 hover:text-accent hover:underline"
          onClick={() => setShowAll((v) => !v)}
        >
          {showAll ? t("home.focusOne") : t("home.showAll")}
        </button>
      </div>

      <div className="mt-auto flex items-center justify-between rounded-2xl bg-[rgb(var(--surface-elevated)/0.75)] px-5 py-3.5 text-sm ring-1 ring-[rgb(var(--border)/0.55)] backdrop-blur-sm">
        <span
          className={
            updatesAvailable
              ? "font-medium text-amber-600 dark:text-amber-400"
              : "text-[rgb(var(--text-secondary))]"
          }
        >
          {t("home.lastUpdateCheck", { result: updatesLabel })}
          {updatesCheckedAt && (
            <span className="ml-1 text-xs opacity-70">({updatesCheckedAt.toLocaleTimeString()})</span>
          )}
        </span>
        <Button
          variant={updatesAvailable ? "primary" : "ghost"}
          onClick={onCheckUpdates}
          disabled={checking}
        >
          {checking ? <Spinner /> : null}
          {updatesAvailable ? t("common.update") : t("home.checkNow")}
        </Button>
      </div>

      <StrategyPickerModal open={pickerOpen} onClose={() => setPickerOpen(false)} />
    </div>
  );
}
