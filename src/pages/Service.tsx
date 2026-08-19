import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { openPath } from "@tauri-apps/plugin-opener";
import { api } from "../lib/api";
import { errText } from "../lib/errors";
import { toast } from "../lib/toast";
import { useStore } from "../lib/store";
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
  StatusDot,
  Switch,
} from "../components/ui";
import type { ActiveFakesStatus, DiagItem, HostsCheck, ServiceSettings } from "../lib/types";

export function ServicePage({ embedded }: { embedded?: boolean } = {}) {
  const { t } = useTranslation();
  const { status, settings, refreshStatus } = useStore();
  const [svc, setSvc] = useState<ServiceSettings | null>(null);
  const [fakes, setFakes] = useState<ActiveFakesStatus | null>(null);
  const [discordPick, setDiscordPick] = useState("");
  const [gamePick, setGamePick] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [confirmDiscord, setConfirmDiscord] = useState(false);
  const [hosts, setHosts] = useState<HostsCheck | null>(null);
  const [diag, setDiag] = useState<DiagItem[] | null>(null);

  const zapret = status?.zapret;

  async function loadServiceSettings() {
    try {
      setSvc(await api.getServiceSettings());
    } catch {
      /* zapret not installed yet */
    }
  }

  async function loadFakes() {
    try {
      const next = await api.getActiveFakes();
      setFakes(next);
      setDiscordPick(next.discordCurrent ?? next.files[0] ?? "");
      setGamePick(next.gameCurrent ?? next.files[0] ?? "");
    } catch {
      setFakes(null);
    }
  }

  useEffect(() => {
    loadServiceSettings();
    loadFakes();
  }, []);

  async function run(name: string, fn: () => Promise<void>, successMsg?: string) {
    setBusy(name);
    try {
      await fn();
      await Promise.all([refreshStatus(), loadServiceSettings()]);
      if (successMsg) toast(successMsg, "ok");
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setBusy(null);
    }
  }

  const stateTone = (state: string | null | undefined) =>
    state === "RUNNING" ? "ok" : state === "STOP_PENDING" ? "warn" : "off";

  return (
    <div className={embedded ? "" : "mx-auto max-w-3xl"}>
      {!embedded && (
        <PageHeader title={t("service.title")} description={t("service.description")} />
      )}

      {/* Status */}
      <Card className="mb-4">
        <div className="mb-2 flex items-center justify-between">
          <h3 className="text-sm font-bold uppercase tracking-wide text-slate-400">
            {t("service.statusBlock")}
          </h3>
          <Button variant="ghost" onClick={() => refreshStatus()}>
            {t("common.refresh")}
          </Button>
        </div>
        <div className="grid grid-cols-1 gap-x-8 gap-y-2 text-sm sm:grid-cols-2">
          <div className="flex items-center justify-between gap-2">
            <span className="text-slate-500">{t("service.serviceState")}</span>
            <Badge tone={stateTone(zapret?.serviceState)}>
              {zapret?.serviceInstalled
                ? (zapret.serviceState ?? t("common.unknown"))
                : t("common.notInstalled")}
            </Badge>
          </div>
          <div className="flex items-center justify-between gap-2">
            <span className="text-slate-500">{t("service.serviceStrategy")}</span>
            <span className="font-medium text-slate-700 dark:text-slate-200">
              {zapret?.serviceStrategy ?? "—"}
            </span>
          </div>
          <div className="flex items-center justify-between gap-2">
            <span className="text-slate-500">{t("service.windivert")}</span>
            <Badge tone={stateTone(zapret?.windivertState)}>
              {zapret?.windivertState ?? t("common.notInstalled")}
            </Badge>
          </div>
          <div className="flex items-center justify-between gap-2">
            <span className="text-slate-500">{t("service.bypassRunning")}</span>
            <Badge tone={zapret?.running ? "ok" : "off"}>
              {zapret?.running ? t("common.running") : t("common.stopped")}
            </Badge>
          </div>
        </div>
        {zapret && !zapret.windivertSysPresent && (
          <div className="mt-3">
            <Note tone="fail" title={t("service.windivertSys")}>
              {t("service.sysMissing")}
            </Note>
          </div>
        )}
      </Card>

      {/* Install / remove service */}
      <Card className="mb-4 divide-y divide-[rgb(var(--border)/0.45)]">
        <FieldRow
          title={t("service.install")}
          description={`${t("service.installDesc")} (${settings?.selectedStrategy?.replace(/\.bat$/i, "") ?? "—"})`}
        >
          <Button
            variant="primary"
            disabled={busy !== null || !settings?.selectedStrategy}
            onClick={() =>
              run("install", () => api.installZapretService(settings!.selectedStrategy!), t("common.saved"))
            }
          >
            {busy === "install" ? <Spinner /> : null}
            {t("common.install")}
          </Button>
        </FieldRow>
        <FieldRow title={t("service.remove")} description={t("service.removeDesc")}>
          <Button variant="danger" disabled={busy !== null} onClick={() => setConfirmRemove(true)}>
            {busy === "remove" ? <Spinner /> : null}
            {t("service.remove")}
          </Button>
        </FieldRow>
      </Card>

      {/* Settings (game filter / ipset / auto update) */}
      <Card className="mb-4 divide-y divide-[rgb(var(--border)/0.45)]">
        <FieldRow title={t("service.gameFilter")} description={t("service.gameFilterDesc")}>
          <Segmented
            value={svc?.gameFilterMode ?? "off"}
            disabled={!svc || busy !== null}
            options={[
              { value: "off", label: t("service.gameOff") },
              { value: "all", label: t("service.gameAll") },
              { value: "tcp", label: t("service.gameTcp") },
              { value: "udp", label: t("service.gameUdp") },
            ]}
            onChange={(mode) => run("game", () => api.setGameFilter(mode))}
          />
        </FieldRow>
        <FieldRow title={t("service.ipset")} description={t("service.ipsetDesc")}>
          <Segmented
            value={svc?.ipsetMode ?? "none"}
            disabled={!svc || busy !== null}
            options={[
              { value: "none", label: "none" },
              { value: "loaded", label: "loaded" },
              { value: "any", label: "any" },
            ]}
            onChange={(mode) => run("ipset", () => api.setIpsetMode(mode))}
          />
        </FieldRow>
        <FieldRow title={t("service.autoUpdate")} description={t("service.autoUpdateDesc")}>
          <Switch
            checked={svc?.autoUpdateCheck ?? false}
            disabled={!svc || busy !== null}
            onChange={(v) => run("autoupd", () => api.setAutoUpdateCheck(v))}
          />
        </FieldRow>
      </Card>

      {/* Active fakes (FlowSeal service.bat #7) */}
      <Card className="mb-4">
        <div className="mb-1 text-sm font-bold text-[rgb(var(--text))]">{t("service.fakesTitle")}</div>
        <p className="mb-4 text-xs leading-relaxed text-[rgb(var(--text-secondary))]">
          {t("service.fakesDesc")}
        </p>
        {!fakes || fakes.files.length === 0 ? (
          <Note tone="info">{t("service.fakesEmpty")}</Note>
        ) : (
          <div className="space-y-3">
            <FakeRow
              label={t("service.fakeDiscord")}
              current={fakes.discordCurrent}
              value={discordPick}
              files={fakes.files}
              busy={busy === "fake-discord"}
              disabled={busy !== null}
              onChange={setDiscordPick}
              onApply={() =>
                run(
                  "fake-discord",
                  async () => {
                    const next = await api.setActiveFake("discord", discordPick);
                    setFakes(next);
                    setDiscordPick(next.discordCurrent ?? discordPick);
                    setGamePick(next.gameCurrent ?? gamePick);
                  },
                  t("service.fakesApplied"),
                )
              }
              applyLabel={t("common.apply")}
              currentLabel={t("service.fakeCurrent")}
              unknownLabel={t("service.fakeUnknown")}
            />
            <FakeRow
              label={t("service.fakeGame")}
              current={fakes.gameCurrent}
              value={gamePick}
              files={fakes.files}
              busy={busy === "fake-game"}
              disabled={busy !== null}
              onChange={setGamePick}
              onApply={() =>
                run(
                  "fake-game",
                  async () => {
                    const next = await api.setActiveFake("game", gamePick);
                    setFakes(next);
                    setDiscordPick(next.discordCurrent ?? discordPick);
                    setGamePick(next.gameCurrent ?? gamePick);
                  },
                  t("service.fakesApplied"),
                )
              }
              applyLabel={t("common.apply")}
              currentLabel={t("service.fakeCurrent")}
              unknownLabel={t("service.fakeUnknown")}
            />
            <p className="text-[11px] text-[rgb(var(--text-secondary))]">{t("service.fakesRestartHint")}</p>
          </div>
        )}
      </Card>

      {/* Updates */}
      <Card className="mb-4 divide-y divide-[rgb(var(--border)/0.45)]">
        <FieldRow title={t("service.updateIpset")} description={t("service.updateIpsetDesc")}>
          <Button
            disabled={busy !== null}
            onClick={() => run("ipsetupd", () => api.updateIpsetList(), t("service.updateIpsetDone"))}
          >
            {busy === "ipsetupd" ? <Spinner /> : null}
            {t("common.update")}
          </Button>
        </FieldRow>
        <FieldRow title={t("service.hosts")} description={t("service.hostsDesc")}>
          <Button
            disabled={busy !== null}
            onClick={() =>
              run("hosts", async () => {
                setHosts(await api.checkHostsFile());
              })
            }
          >
            {busy === "hosts" ? <Spinner /> : null}
            {t("service.hostsCheck")}
          </Button>
        </FieldRow>
        {hosts && (
          <div className="pt-3">
            <Note tone={hosts.upToDate ? "info" : "warn"}>
              {hosts.upToDate ? t("service.hostsOk") : t("service.hostsOutdated")}
              {!hosts.upToDate && (
                <div className="mt-2 flex gap-2">
                  <Button onClick={() => openPath(hosts.tempFile).catch(() => {})}>
                    {t("service.hostsOpenTemp")}
                  </Button>
                  <Button
                    onClick={() =>
                      openPath(hosts.hostsFile.replace(/\\hosts$/i, "")).catch(() => {})
                    }
                  >
                    {t("service.hostsOpenFolder")}
                  </Button>
                </div>
              )}
            </Note>
          </div>
        )}
      </Card>

      {/* Diagnostics */}
      <Card className="mb-4">
        <FieldRow title={t("service.diagnostics")} description={t("service.diagnosticsDesc")}>
          <Button
            variant="primary"
            disabled={busy !== null}
            onClick={() =>
              run("diag", async () => {
                setDiag(await api.runDiagnostics());
              })
            }
          >
            {busy === "diag" ? <Spinner /> : null}
            {t("service.runDiagnostics")}
          </Button>
        </FieldRow>

        {diag && (
          <div className="mt-2 space-y-1.5">
            {diag.map((item) => (
              <div
                key={item.id}
                className="flex items-center gap-3 rounded-lg bg-slate-50 px-3 py-2 text-sm dark:bg-slate-800/50"
              >
                <StatusDot tone={item.status === "ok" ? "ok" : item.status === "warn" ? "warn" : "fail"} />
                <span className="font-medium text-slate-700 dark:text-slate-200">
                  {t(`service.diag.${item.id}`, { defaultValue: item.id })}
                </span>
                {item.detail && (
                  <span className="min-w-0 flex-1 truncate text-right text-xs text-slate-400" title={item.detail}>
                    {item.detail}
                  </span>
                )}
              </div>
            ))}
            <div className="flex flex-wrap gap-2 pt-2">
              {diag.some((d) => d.id === "conflicting_bypasses" && d.status === "fail") && (
                <Button
                  variant="danger"
                  disabled={busy !== null}
                  onClick={() =>
                    run("conflicts", async () => {
                      const removed = await api.removeConflictingServices();
                      toast(t("service.conflictsRemoved", { list: removed.join(", ") || "—" }), "ok");
                      setDiag(await api.runDiagnostics());
                    })
                  }
                >
                  {t("service.removeConflicts")}
                </Button>
              )}
              <Button disabled={busy !== null} onClick={() => setConfirmDiscord(true)}>
                {t("service.clearDiscordCache")}
              </Button>
            </div>
          </div>
        )}
      </Card>

      {/* Confirmations */}
      <Modal
        open={confirmRemove}
        onClose={() => setConfirmRemove(false)}
        title={t("service.remove")}
        footer={
          <>
            <Button onClick={() => setConfirmRemove(false)}>{t("common.cancel")}</Button>
            <Button
              variant="danger"
              onClick={() => {
                setConfirmRemove(false);
                run("remove", () => api.removeZapretServices(), t("common.saved"));
              }}
            >
              {t("common.confirm")}
            </Button>
          </>
        }
      >
        {t("service.removeConfirm")}
      </Modal>

      <Modal
        open={confirmDiscord}
        onClose={() => setConfirmDiscord(false)}
        title={t("service.clearDiscordCache")}
        footer={
          <>
            <Button onClick={() => setConfirmDiscord(false)}>{t("common.cancel")}</Button>
            <Button
              variant="danger"
              onClick={() => {
                setConfirmDiscord(false);
                run("discord", async () => {
                  const cleared = await api.clearDiscordCache();
                  if (cleared.length === 0) {
                    toast(t("service.cacheNone"), "info");
                  } else {
                    toast(t("service.cacheCleared", { list: cleared.join(", ") }), "ok");
                  }
                });
              }}
            >
              {t("common.confirm")}
            </Button>
          </>
        }
      >
        {t("service.clearDiscordConfirm")}
      </Modal>
    </div>
  );
}

function FakeRow({
  label,
  current,
  value,
  files,
  busy,
  disabled,
  onChange,
  onApply,
  applyLabel,
  currentLabel,
  unknownLabel,
}: {
  label: string;
  current: string | null;
  value: string;
  files: string[];
  busy: boolean;
  disabled: boolean;
  onChange: (v: string) => void;
  onApply: () => void;
  applyLabel: string;
  currentLabel: string;
  unknownLabel: string;
}) {
  return (
    <div className="rounded-2xl bg-[rgb(var(--surface))] px-3.5 py-3 ring-1 ring-[rgb(var(--border)/0.45)]">
      <div className="mb-2 flex flex-wrap items-center justify-between gap-2">
        <div className="text-sm font-semibold text-[rgb(var(--text))]">{label}</div>
        <div className="text-[11px] text-[rgb(var(--text-secondary))]">
          {currentLabel}: <span className="font-medium text-[rgb(var(--text))]">{current ?? unknownLabel}</span>
        </div>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <select
          value={value}
          disabled={disabled}
          onChange={(e) => onChange(e.target.value)}
          className="min-w-0 flex-1 rounded-xl border-0 bg-[rgb(var(--surface-elevated))] px-3 py-2 text-sm ring-1 ring-[rgb(var(--border)/0.55)] outline-none focus:ring-2 focus:ring-accent"
        >
          {files.map((name) => (
            <option key={name} value={name}>
              {name}
            </option>
          ))}
        </select>
        <Button variant="primary" disabled={disabled || !value || value === current} onClick={onApply}>
          {busy ? <Spinner /> : null}
          {applyLabel}
        </Button>
      </div>
    </div>
  );
}
