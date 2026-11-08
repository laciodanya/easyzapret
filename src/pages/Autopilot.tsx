import { useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../lib/api";
import { errText } from "../lib/errors";
import { toast } from "../lib/toast";
import { useStore } from "../lib/store";
import type {
  AutopilotHistoryKind,
  AutopilotSettings,
  AutopilotStatus,
  ServiceHealth,
} from "../lib/types";
import {
  Badge,
  Button,
  Chip,
  Empty,
  Note,
  PageHeader,
  Panel,
  Row,
  Segmented,
  Spinner,
  StatusDot,
  Switch,
  cn,
  type Tone,
} from "../components/ui";

function healthTone(p: number | null | undefined): Tone {
  if (p == null) return "off";
  if (p >= 80) return "ok";
  if (p >= 50) return "warn";
  return "fail";
}

function historyTone(kind: AutopilotHistoryKind): Tone {
  switch (kind) {
    case "check_ok":
    case "recovered":
      return "ok";
    case "degraded":
    case "flaky":
    case "offline":
      return "warn";
    case "failed":
      return "fail";
    case "switch":
      return "info";
    case "skipped":
    case "info":
      return "off";
    default: {
      const exhaustive: never = kind;
      return exhaustive;
    }
  }
}

const SERVICE_TONE: Record<ServiceHealth["state"], Tone> = {
  ok: "ok",
  partial: "warn",
  down: "fail",
};

type ProbeKey = "probeDiscord" | "probeYoutube" | "probeTelegram" | "probeCloudflare" | "probeGoogle";

const PROBES: { key: ProbeKey; service: string }[] = [
  { key: "probeDiscord", service: "discord" },
  { key: "probeYoutube", service: "youtube" },
  { key: "probeTelegram", service: "telegram" },
  { key: "probeCloudflare", service: "cloudflare" },
  { key: "probeGoogle", service: "google" },
];

function Fact({ label, value, tone }: { label: string; value: string; tone?: Tone }) {
  return (
    <div className="flex items-center justify-between gap-6 py-1.5 text-[13px]">
      <span className="text-muted">{label}</span>
      <span className="flex items-center gap-2 font-medium">
        {tone && <StatusDot tone={tone} size={7} />}
        {value}
      </span>
    </div>
  );
}

function StatusPanel({
  st,
  enabled,
  checking,
  onCheck,
}: {
  st: AutopilotStatus | undefined;
  enabled: boolean;
  checking: boolean;
  onCheck: () => void;
}) {
  const { t } = useTranslation();
  const percent = st?.lastHealthPercent ?? null;
  const tone = healthTone(percent);
  const lastCheck = st?.lastCheckAt ? (st.lastCheckAt.split(" ")[1] ?? st.lastCheckAt) : null;
  const busy = checking || !!st?.checking || !!st?.switching;

  const internet: { text: string; tone: Tone } =
    st?.internetOk == null
      ? { text: "—", tone: "off" }
      : st.internetOk
        ? { text: t("autopilot.internetOk"), tone: "ok" }
        : { text: t("autopilot.internetDown"), tone: "fail" };

  return (
    <Panel
      title={t("autopilot.statusTitle")}
      action={
        <Button variant="primary" size="sm" onClick={onCheck} disabled={busy || !enabled}>
          {busy && <Spinner size={14} />}
          {st?.switching ? t("autopilot.switching") : t("autopilot.runNow")}
        </Button>
      }
      padded
    >
      <div className="flex flex-wrap items-center gap-x-12 gap-y-5">
        <div>
          <div
            className={cn(
              "text-[44px] font-semibold leading-none tabular-nums",
              tone === "ok" && "text-ok",
              tone === "warn" && "text-warn",
              tone === "fail" && "text-fail",
              tone === "off" && "text-faint",
            )}
          >
            {percent != null ? `${percent}%` : "—"}
          </div>
          <div className="mt-2 text-[13px] text-muted">{t("autopilot.health")}</div>
        </div>
        <div className="min-w-[260px] flex-1 divide-y divide-line">
          <Fact label={t("autopilot.internet")} value={internet.text} tone={internet.tone} />
          <Fact label={t("autopilot.lastCheck")} value={lastCheck ?? "—"} />
          <Fact
            label={t("autopilot.nextCheck")}
            value={enabled ? (st?.nextCheckAt ?? "—") : t("autopilot.off")}
          />
          <Fact label={t("autopilot.switchesHour")} value={String(st?.switchesThisHour ?? 0)} />
        </div>
      </div>

      {st && st.services.length > 0 && (
        <div className="mt-5 divide-y divide-line border-t border-line">
          {st.services.map((svc) => (
            <div key={svc.id} className="flex items-center gap-3 py-2.5">
              <StatusDot tone={SERVICE_TONE[svc.state]} />
              <span className="flex-1 text-sm">{t(`services.${svc.id}`)}</span>
              {svc.bulkBlocked && <Badge tone="warn">{t("autopilot.throttled")}</Badge>}
              {svc.avgMs != null && (
                <span className="text-[13px] tabular-nums text-muted">
                  {t("common.ms", { value: svc.avgMs })}
                </span>
              )}
              <span className="w-10 text-right text-[13px] font-medium tabular-nums">
                {svc.ok}/{svc.total}
              </span>
            </div>
          ))}
        </div>
      )}

      {st?.lastMessage && (
        <p className="mt-4 text-[13px] leading-relaxed text-muted">{st.lastMessage}</p>
      )}
    </Panel>
  );
}

export function AutopilotPage() {
  const { t } = useTranslation();
  const { settings, status, strategies, updateSettings, refreshStatus } = useStore();
  const [checking, setChecking] = useState(false);
  const [busy, setBusy] = useState(false);

  const ap = settings?.autopilot;
  const st = status?.autopilot;

  async function patch(p: Partial<AutopilotSettings>) {
    if (!ap) return;
    setBusy(true);
    try {
      await updateSettings({ autopilot: { ...ap, ...p } });
    } finally {
      setBusy(false);
    }
  }

  async function runCheck() {
    setChecking(true);
    try {
      await api.runAutopilotCheckNow();
      await refreshStatus();
      toast(t("autopilot.checkDone"), "ok");
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setChecking(false);
    }
  }

  if (!ap) return null;

  return (
    <div className="space-y-6">
      <PageHeader title={t("autopilot.title")} description={t("autopilot.description")} />

      <Panel>
        <Row title={t("autopilot.enabled")} description={t("autopilot.enabledDesc")}>
          <Switch
            label={t("autopilot.enabled")}
            checked={ap.enabled}
            disabled={busy}
            onChange={(v) => void patch({ enabled: v })}
          />
        </Row>
      </Panel>

      {st?.monitoringOnly && (
        <Note tone="warn" title={t("autopilot.monitoringTitle")}>
          {t("autopilot.monitoringText")}
        </Note>
      )}

      <StatusPanel st={st} enabled={ap.enabled} checking={checking} onCheck={() => void runCheck()} />

      <Panel title={t("autopilot.historyTitle")} description={t("autopilot.historyDesc")}>
        {st && st.history.length > 0 ? (
          st.history.slice(0, 12).map((entry, i) => (
            <div key={`${entry.at}-${i}`} className="flex items-start gap-3 py-2.5">
              <span className="w-[52px] shrink-0 pt-px font-mono text-xs text-faint">{entry.at}</span>
              <span className="mt-[6px] flex">
                <StatusDot tone={historyTone(entry.kind)} size={7} />
              </span>
              <span className="selectable min-w-0 flex-1 text-[13px] leading-snug">
                {entry.message}
              </span>
            </div>
          ))
        ) : (
          <Empty>{t("autopilot.historyEmpty")}</Empty>
        )}
      </Panel>

      <Panel title={t("autopilot.probes")} description={t("autopilot.probesDesc")}>
        <div className="flex flex-wrap gap-2 py-4">
          {PROBES.map(({ key, service }) => (
            <Chip
              key={key}
              active={ap[key]}
              disabled={busy}
              onClick={() => void patch({ [key]: !ap[key] })}
            >
              {t(`services.${service}`)}
            </Chip>
          ))}
        </div>
        <Row title={t("autopilot.deepCheck")} description={t("autopilot.deepCheckDesc")}>
          <Switch
            label={t("autopilot.deepCheck")}
            checked={ap.deepCheck}
            disabled={busy}
            onChange={(v) => void patch({ deepCheck: v })}
          />
        </Row>
      </Panel>

      <Panel title={t("autopilot.reactionTitle")}>
        <Row title={t("autopilot.autoSwitch")} description={t("autopilot.autoSwitchDesc")}>
          <Switch
            label={t("autopilot.autoSwitch")}
            checked={ap.autoSwitchStrategy}
            disabled={busy}
            onChange={(v) => void patch({ autoSwitchStrategy: v })}
          />
        </Row>
        <Row title={t("autopilot.switchModeLabel")} description={t("autopilot.switchModeDesc")}>
          <Segmented
            value={ap.switchMode}
            disabled={busy}
            onChange={(v) => void patch({ switchMode: v })}
            options={[
              { value: "rotate_policy", label: t("autopilot.switchModes.rotate_policy") },
              { value: "rotate_custom", label: t("autopilot.switchModes.rotate_custom") },
              { value: "best_by_tests", label: t("autopilot.switchModes.best_by_tests") },
            ]}
          />
        </Row>

        {ap.switchMode === "rotate_policy" && (
          <Row title={t("autopilot.policyLabel")} description={t("autopilot.policyDesc")}>
            <Segmented
              value={ap.policy}
              disabled={busy}
              onChange={(v) => void patch({ policy: v })}
              options={[
                { value: "stability", label: t("autopilot.policies.stability") },
                { value: "speed", label: t("autopilot.policies.speed") },
                { value: "gaming", label: t("autopilot.policies.gaming") },
              ]}
            />
          </Row>
        )}

        {(ap.switchMode === "rotate_custom" || ap.switchMode === "best_by_tests") && (
          <div className="py-3.5">
            <div className="text-sm font-medium">{t("autopilot.customList")}</div>
            <p className="mb-3 mt-0.5 text-[13px] text-muted">
              {ap.switchMode === "rotate_custom"
                ? t("autopilot.customListDescRotate")
                : t("autopilot.customListDescBest")}
            </p>
            <div className="grid max-h-56 grid-cols-2 gap-x-4 overflow-y-auto rounded-control border border-line bg-sunken p-2">
              {strategies.length === 0 ? (
                <p className="col-span-2 p-2 text-[13px] text-faint">{t("strategies.empty")}</p>
              ) : (
                strategies.map((bat) => {
                  const name = bat.replace(/\.bat$/i, "");
                  const checked = ap.allowedStrategies.some(
                    (s) => s.toLowerCase() === bat.toLowerCase(),
                  );
                  return (
                    <label
                      key={bat}
                      className="flex h-8 items-center gap-2.5 rounded-md px-2 text-[13px] hover:bg-raised"
                    >
                      <input
                        type="checkbox"
                        checked={checked}
                        disabled={busy}
                        className="accent-[var(--accent)]"
                        onChange={(e) => {
                          const others = ap.allowedStrategies.filter(
                            (s) => s.toLowerCase() !== bat.toLowerCase(),
                          );
                          void patch({ allowedStrategies: e.target.checked ? [...others, bat] : others });
                        }}
                      />
                      <span className="truncate">{name}</span>
                    </label>
                  );
                })
              )}
            </div>
          </div>
        )}

        {ap.switchMode === "best_by_tests" && (
          <Row
            title={t("autopilot.maxTestStrategies")}
            description={t("autopilot.maxTestStrategiesDesc")}
          >
            <Segmented
              value={String(ap.maxTestStrategies)}
              disabled={busy}
              onChange={(v) => void patch({ maxTestStrategies: Number(v) })}
              options={[
                { value: "3", label: "3" },
                { value: "6", label: "6" },
                { value: "9", label: "9" },
              ]}
            />
          </Row>
        )}

        <Row title={t("autopilot.minHealth")} description={t("autopilot.minHealthDesc")}>
          <Segmented
            value={String(ap.minHealthPercent)}
            disabled={busy}
            onChange={(v) => void patch({ minHealthPercent: Number(v) })}
            options={[
              { value: "40", label: "40%" },
              { value: "60", label: "60%" },
              { value: "80", label: "80%" },
            ]}
          />
        </Row>
        <Row title={t("autopilot.maxSwitches")} description={t("autopilot.maxSwitchesDesc")}>
          <Segmented
            value={String(ap.maxSwitchesPerHour)}
            disabled={busy}
            onChange={(v) => void patch({ maxSwitchesPerHour: Number(v) })}
            options={[
              { value: "1", label: "1" },
              { value: "3", label: "3" },
              { value: "5", label: "5" },
            ]}
          />
        </Row>
      </Panel>

      <Panel title={t("autopilot.scheduleTitle")}>
        <Row title={t("autopilot.interval")} description={t("autopilot.intervalDesc")}>
          <Segmented
            value={String(ap.intervalMinutes)}
            disabled={busy}
            onChange={(v) => void patch({ intervalMinutes: Number(v) })}
            options={[
              { value: "5", label: t("autopilot.minutes", { n: 5 }) },
              { value: "15", label: t("autopilot.minutes", { n: 15 }) },
              { value: "30", label: t("autopilot.minutes", { n: 30 }) },
              { value: "60", label: t("autopilot.minutes", { n: 60 }) },
            ]}
          />
        </Row>
        <Row title={t("autopilot.onlyWhenZapret")} description={t("autopilot.onlyWhenZapretDesc")}>
          <Switch
            label={t("autopilot.onlyWhenZapret")}
            checked={ap.onlyWhenZapretRunning}
            disabled={busy}
            onChange={(v) => void patch({ onlyWhenZapretRunning: v })}
          />
        </Row>
        <Row title={t("autopilot.autoWarp")} description={t("autopilot.autoWarpDesc")}>
          <Switch
            label={t("autopilot.autoWarp")}
            checked={ap.autoEnableWarp}
            disabled={busy}
            onChange={(v) => void patch({ autoEnableWarp: v })}
          />
        </Row>
        <Row title={t("autopilot.notifySwitch")}>
          <Switch
            label={t("autopilot.notifySwitch")}
            checked={ap.notifyOnSwitch}
            disabled={busy}
            onChange={(v) => void patch({ notifyOnSwitch: v })}
          />
        </Row>
        <Row title={t("autopilot.notifyDegraded")}>
          <Switch
            label={t("autopilot.notifyDegraded")}
            checked={ap.notifyOnDegraded}
            disabled={busy}
            onChange={(v) => void patch({ notifyOnDegraded: v })}
          />
        </Row>
      </Panel>

      {ap.switchMode === "best_by_tests" && (
        <Note tone="warn" title={t("autopilot.bestByTestsNoteTitle")}>
          {t("autopilot.bestByTestsNote")}
        </Note>
      )}
    </div>
  );
}
