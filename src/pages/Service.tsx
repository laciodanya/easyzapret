import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { openPath } from "@tauri-apps/plugin-opener";
import { api } from "../lib/api";
import { errText } from "../lib/errors";
import { toast } from "../lib/toast";
import { useStore } from "../lib/store";
import {
  Badge,
  Button,
  ConfirmModal,
  Note,
  Panel,
  Row,
  Segmented,
  Select,
  Spinner,
  StatusDot,
  Switch,
  type Tone,
} from "../components/ui";
import type { ActiveFakesStatus, DiagItem, HostsCheck, ServiceSettings } from "../lib/types";

const DIAG_TONE: Record<DiagItem["status"], Tone> = { ok: "ok", warn: "warn", fail: "fail" };

function stateTone(state: string | null | undefined): Tone {
  if (state === "RUNNING") return "ok";
  if (state === "STOP_PENDING") return "warn";
  return "off";
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
}: {
  label: string;
  current: string | null;
  value: string;
  files: string[];
  busy: boolean;
  disabled: boolean;
  onChange: (v: string) => void;
  onApply: () => void;
}) {
  const { t } = useTranslation();
  return (
    <Row
      title={label}
      description={`${t("service.fakeCurrent")}: ${current ?? t("service.fakeUnknown")}`}
    >
      <Select value={value} disabled={disabled} onChange={(e) => onChange(e.target.value)} className="w-56">
        {files.map((name) => (
          <option key={name} value={name}>
            {name}
          </option>
        ))}
      </Select>
      <Button variant="primary" disabled={disabled || !value || value === current} onClick={onApply}>
        {busy && <Spinner size={14} />}
        {t("common.apply")}
      </Button>
    </Row>
  );
}

export function ServicePage() {
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
  const diagResults = useRef<HTMLDivElement>(null);

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
    void loadServiceSettings();
    void loadFakes();
  }, []);

  // Results land far below the fold, so bring them into view as soon as they exist.
  useEffect(() => {
    if (!diag) return;
    const frame = requestAnimationFrame(() => {
      diagResults.current?.scrollIntoView({ behavior: "smooth", block: "start" });
    });
    return () => cancelAnimationFrame(frame);
  }, [diag]);

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

  const diagCounts = diag
    ? {
        fail: diag.filter((d) => d.status === "fail").length,
        warn: diag.filter((d) => d.status === "warn").length,
      }
    : null;

  const diagSummary = diagCounts
    ? diagCounts.fail === 0 && diagCounts.warn === 0
      ? t("service.diagAllGood")
      : t("service.diagSummary", { fail: diagCounts.fail, warn: diagCounts.warn })
    : t("service.diagnosticsDesc");

  return (
    <div className="space-y-6">
      <Panel
        title={t("service.statusBlock")}
        action={
          <Button size="sm" variant="ghost" icon="refresh" onClick={() => void refreshStatus()}>
            {t("common.refresh")}
          </Button>
        }
      >
        <Row title={t("service.bypassRunning")}>
          <Badge tone={zapret?.running ? "ok" : "off"}>
            {zapret?.running ? t("common.running") : t("common.stopped")}
          </Badge>
        </Row>
        <Row title={t("service.serviceState")}>
          <Badge tone={stateTone(zapret?.serviceState)}>
            {zapret?.serviceInstalled
              ? (zapret.serviceState ?? t("common.unknown"))
              : t("common.notInstalled")}
          </Badge>
        </Row>
        <Row title={t("service.serviceStrategy")}>
          <span className="text-sm">{zapret?.serviceStrategy ?? "—"}</span>
        </Row>
        <Row title={t("service.windivert")}>
          <Badge tone={stateTone(zapret?.windivertState)}>
            {zapret?.windivertState ?? t("common.notInstalled")}
          </Badge>
        </Row>
      </Panel>

      {zapret && !zapret.windivertSysPresent && (
        <Note tone="fail" title={t("service.windivertSys")}>
          {t("service.sysMissing")}
        </Note>
      )}

      <Panel title={t("service.serviceBlock")}>
        <Row
          title={t("service.install")}
          description={`${t("service.installDesc")} (${settings?.selectedStrategy?.replace(/\.bat$/i, "") ?? "—"})`}
        >
          <Button
            variant="primary"
            disabled={busy !== null || !settings?.selectedStrategy}
            onClick={() =>
              void run(
                "install",
                () => api.installZapretService(settings!.selectedStrategy!),
                t("common.saved"),
              )
            }
          >
            {busy === "install" && <Spinner size={14} />}
            {t("common.install")}
          </Button>
        </Row>
        <Row title={t("service.remove")} description={t("service.removeDesc")}>
          <Button variant="danger" disabled={busy !== null} onClick={() => setConfirmRemove(true)}>
            {busy === "remove" && <Spinner size={14} />}
            {t("service.remove")}
          </Button>
        </Row>
      </Panel>

      <Panel title={t("service.filtersBlock")}>
        <Row title={t("service.gameFilter")} description={t("service.gameFilterDesc")}>
          <Segmented
            value={svc?.gameFilterMode ?? "off"}
            disabled={!svc || busy !== null}
            options={[
              { value: "off", label: t("service.gameOff") },
              { value: "all", label: t("service.gameAll") },
              { value: "tcp", label: t("service.gameTcp") },
              { value: "udp", label: t("service.gameUdp") },
            ]}
            onChange={(mode) => void run("game", () => api.setGameFilter(mode))}
          />
        </Row>
        <Row title={t("service.ipset")} description={t("service.ipsetDesc")}>
          <Segmented
            value={svc?.ipsetMode ?? "none"}
            disabled={!svc || busy !== null}
            options={[
              { value: "none", label: "none" },
              { value: "loaded", label: "loaded" },
              { value: "any", label: "any" },
            ]}
            onChange={(mode) => void run("ipset", () => api.setIpsetMode(mode))}
          />
        </Row>
        <Row title={t("service.autoUpdate")} description={t("service.autoUpdateDesc")}>
          <Switch
            label={t("service.autoUpdate")}
            checked={svc?.autoUpdateCheck ?? false}
            disabled={!svc || busy !== null}
            onChange={(v) => void run("autoupd", () => api.setAutoUpdateCheck(v))}
          />
        </Row>
      </Panel>

      <Panel title={t("service.fakesTitle")} description={t("service.fakesDesc")}>
        {!fakes || fakes.files.length === 0 ? (
          <div className="py-4">
            <Note>{t("service.fakesEmpty")}</Note>
          </div>
        ) : (
          <>
            <FakeRow
              label={t("service.fakeDiscord")}
              current={fakes.discordCurrent}
              value={discordPick}
              files={fakes.files}
              busy={busy === "fake-discord"}
              disabled={busy !== null}
              onChange={setDiscordPick}
              onApply={() =>
                void run(
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
                void run(
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
            />
            <p className="py-3 text-xs text-faint">{t("service.fakesRestartHint")}</p>
          </>
        )}
      </Panel>

      <Panel title={t("service.updatesBlock")}>
        <Row title={t("service.updateIpset")} description={t("service.updateIpsetDesc")}>
          <Button
            disabled={busy !== null}
            onClick={() =>
              void run("ipsetupd", () => api.updateIpsetList(), t("service.updateIpsetDone"))
            }
          >
            {busy === "ipsetupd" && <Spinner size={14} />}
            {t("common.update")}
          </Button>
        </Row>
        <Row title={t("service.hosts")} description={t("service.hostsDesc")}>
          <Button
            disabled={busy !== null}
            onClick={() =>
              void run("hosts", async () => {
                setHosts(await api.checkHostsFile());
              })
            }
          >
            {busy === "hosts" && <Spinner size={14} />}
            {t("service.hostsCheck")}
          </Button>
        </Row>
        {hosts && (
          <div className="py-3.5">
            <Note
              tone={hosts.upToDate ? "info" : "warn"}
              action={
                !hosts.upToDate && (
                  <div className="flex gap-2">
                    <Button size="sm" onClick={() => openPath(hosts.tempFile).catch(() => {})}>
                      {t("service.hostsOpenTemp")}
                    </Button>
                    <Button
                      size="sm"
                      onClick={() => void api.openHostsFolder().catch(() => {})}
                    >
                      {t("service.hostsOpenFolder")}
                    </Button>
                  </div>
                )
              }
            >
              {hosts.upToDate ? t("service.hostsOk") : t("service.hostsOutdated")}
            </Note>
          </div>
        )}
      </Panel>

      <Panel title={t("service.cacheBlock")}>
        <Row title={t("service.clearDiscordCache")} description={t("service.cacheDesc")}>
          <Button disabled={busy !== null} onClick={() => setConfirmDiscord(true)}>
            {busy === "discord" && <Spinner size={14} />}
            {t("service.clearDiscordCache")}
          </Button>
        </Row>
      </Panel>

      <div ref={diagResults} className="scroll-mt-6">
        <Panel
          title={t("service.diagnostics")}
          description={diagSummary}
          action={
            <Button
              variant="primary"
              disabled={busy !== null}
              onClick={() =>
                void run("diag", async () => {
                  setDiag(await api.runDiagnostics());
                })
              }
            >
              {busy === "diag" && <Spinner size={14} />}
              {t("service.runDiagnostics")}
            </Button>
          }
        >
          {diag && (
            <>
              {diag.map((item) => (
                <div key={item.id} className="flex items-start gap-3 py-3">
                  <span className="mt-[6px] flex">
                    <StatusDot tone={DIAG_TONE[item.status]} />
                  </span>
                  <div className="min-w-0 flex-1">
                    <div className="text-sm font-medium">
                      {t(`service.diag.${item.id}`, { defaultValue: item.id })}
                    </div>
                    {item.detail && (
                      <div className="selectable mt-0.5 break-words text-[13px] text-muted">
                        {item.detail}
                      </div>
                    )}
                  </div>
                </div>
              ))}
              {diag.some((d) => d.id === "conflicting_bypasses" && d.status === "fail") && (
                <div className="py-3.5">
                  <Button
                    variant="danger"
                    disabled={busy !== null}
                    onClick={() =>
                      void run("conflicts", async () => {
                        const removed = await api.removeConflictingServices();
                        toast(
                          t("service.conflictsRemoved", { list: removed.join(", ") || "—" }),
                          "ok",
                        );
                        setDiag(await api.runDiagnostics());
                      })
                    }
                  >
                    {t("service.removeConflicts")}
                  </Button>
                </div>
              )}
            </>
          )}
        </Panel>
      </div>

      <ConfirmModal
        open={confirmRemove}
        title={t("service.remove")}
        tone="danger"
        confirmLabel={t("common.confirm")}
        onClose={() => setConfirmRemove(false)}
        onConfirm={() => {
          setConfirmRemove(false);
          void run("remove", () => api.removeZapretServices(), t("common.saved"));
        }}
      >
        {t("service.removeConfirm")}
      </ConfirmModal>

      <ConfirmModal
        open={confirmDiscord}
        title={t("service.clearDiscordCache")}
        tone="danger"
        confirmLabel={t("service.cacheConfirmButton")}
        onClose={() => setConfirmDiscord(false)}
        onConfirm={() => {
          setConfirmDiscord(false);
          void run("discord", async () => {
            const cleared = await api.clearDiscordCache();
            toast(
              cleared.length === 0
                ? t("service.cacheNone")
                : t("service.cacheCleared", { list: cleared.join(", ") }),
              cleared.length === 0 ? "info" : "ok",
            );
          });
        }}
      >
        {t("service.clearDiscordConfirm")}
      </ConfirmModal>
    </div>
  );
}
