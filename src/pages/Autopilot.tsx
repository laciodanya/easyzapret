import { useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../lib/api";
import { errText } from "../lib/errors";
import { toast } from "../lib/toast";
import { useStore } from "../lib/store";
import type { AutopilotSettings } from "../lib/types";
import {
  Badge,
  Button,
  Card,
  FieldRow,
  Note,
  PageHeader,
  Segmented,
  Spinner,
  Switch,
} from "../components/ui";

function healthTone(p: number | null | undefined): "ok" | "warn" | "fail" | "off" {
  if (p == null) return "off";
  if (p >= 80) return "ok";
  if (p >= 50) return "warn";
  return "fail";
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
    <div className="mx-auto max-w-3xl space-y-4">
      <PageHeader title={t("autopilot.title")} description={t("autopilot.description")} />

      <Card>
        <FieldRow title={t("autopilot.enabled")} description={t("autopilot.enabledDesc")}>
          <Switch checked={ap.enabled} disabled={busy} onChange={(v) => patch({ enabled: v })} />
        </FieldRow>

        {st && (
          <div className="mt-3 grid grid-cols-2 gap-3 sm:grid-cols-4">
            <Stat label={t("autopilot.health")} value={st.lastHealthPercent != null ? `${st.lastHealthPercent}%` : "—"} tone={healthTone(st.lastHealthPercent)} />
            <Stat label={t("autopilot.lastCheck")} value={st.lastCheckAt ?? "—"} />
            <Stat label={t("autopilot.policy")} value={t(`autopilot.switchModes.${ap.switchMode}`)} />
            <Stat label={t("autopilot.switchesHour")} value={String(st.switchesThisHour)} />
          </div>
        )}

        <div className="mt-4 flex gap-2">
          <Button variant="primary" onClick={runCheck} disabled={checking || !ap.enabled}>
            {checking ? <Spinner /> : null}
            {t("autopilot.runNow")}
          </Button>
        </div>
        {st?.lastMessage && (
          <p className="mt-2 text-xs text-slate-500 dark:text-slate-400">{st.lastMessage}</p>
        )}
      </Card>

      <Card className="divide-y divide-[rgb(var(--border)/0.5)]">
        <FieldRow title={t("autopilot.interval")} description={t("autopilot.intervalDesc")}>
          <Segmented
            value={String(ap.intervalMinutes)}
            disabled={busy}
            onChange={(v) => patch({ intervalMinutes: Number(v) })}
            options={[
              { value: "5", label: "5m" },
              { value: "15", label: "15m" },
              { value: "30", label: "30m" },
              { value: "60", label: "60m" },
            ]}
          />
        </FieldRow>

        <FieldRow title={t("autopilot.switchModeLabel")} description={t("autopilot.switchModeDesc")}>
          <Segmented
            value={ap.switchMode}
            disabled={busy}
            onChange={(v) => patch({ switchMode: v as AutopilotSettings["switchMode"] })}
            options={[
              { value: "rotate_policy", label: t("autopilot.switchModes.rotate_policy") },
              { value: "rotate_custom", label: t("autopilot.switchModes.rotate_custom") },
              { value: "best_by_tests", label: t("autopilot.switchModes.best_by_tests") },
            ]}
          />
        </FieldRow>

        {ap.switchMode === "rotate_policy" && (
          <FieldRow title={t("autopilot.policyLabel")} description={t("autopilot.policyDesc")}>
            <Segmented
              value={ap.policy}
              disabled={busy}
              onChange={(v) => patch({ policy: v as AutopilotSettings["policy"] })}
              options={[
                { value: "stability", label: t("autopilot.policies.stability") },
                { value: "speed", label: t("autopilot.policies.speed") },
                { value: "gaming", label: t("autopilot.policies.gaming") },
              ]}
            />
          </FieldRow>
        )}

        {(ap.switchMode === "rotate_custom" || ap.switchMode === "best_by_tests") && (
          <div className="py-3">
            <div className="mb-2 text-sm font-semibold text-slate-800 dark:text-slate-100">
              {t("autopilot.customList")}
            </div>
            <p className="mb-3 text-xs text-slate-500 dark:text-slate-400">
              {ap.switchMode === "rotate_custom"
                ? t("autopilot.customListDescRotate")
                : t("autopilot.customListDescBest")}
            </p>
            <div className="max-h-48 space-y-1 overflow-y-auto rounded-xl bg-[rgb(var(--surface))] p-2 ring-1 ring-[rgb(var(--border)/0.5)]">
              {strategies.length === 0 ? (
                <p className="p-2 text-xs text-slate-400">{t("strategies.empty")}</p>
              ) : (
                strategies.map((bat) => {
                  const name = bat.replace(/\.bat$/i, "");
                  const checked = ap.allowedStrategies.some((s) => s.toLowerCase() === bat.toLowerCase());
                  return (
                    <label
                      key={bat}
                      className="flex cursor-pointer items-center gap-2 rounded-lg px-2 py-1.5 text-sm hover:bg-accent-soft"
                    >
                      <input
                        type="checkbox"
                        checked={checked}
                        disabled={busy}
                        className="accent-[rgb(var(--accent))]"
                        onChange={(e) => {
                          const next = e.target.checked
                            ? [...ap.allowedStrategies.filter((s) => s.toLowerCase() !== bat.toLowerCase()), bat]
                            : ap.allowedStrategies.filter((s) => s.toLowerCase() !== bat.toLowerCase());
                          patch({ allowedStrategies: next });
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
          <FieldRow title={t("autopilot.maxTestStrategies")} description={t("autopilot.maxTestStrategiesDesc")}>
            <Segmented
              value={String(ap.maxTestStrategies)}
              disabled={busy}
              onChange={(v) => patch({ maxTestStrategies: Number(v) })}
              options={[
                { value: "3", label: "3" },
                { value: "6", label: "6" },
                { value: "9", label: "9" },
              ]}
            />
          </FieldRow>
        )}

        <FieldRow title={t("autopilot.minHealth")} description={t("autopilot.minHealthDesc")}>
          <Segmented
            value={String(ap.minHealthPercent)}
            disabled={busy}
            onChange={(v) => patch({ minHealthPercent: Number(v) })}
            options={[
              { value: "40", label: "40%" },
              { value: "60", label: "60%" },
              { value: "80", label: "80%" },
            ]}
          />
        </FieldRow>

        <FieldRow title={t("autopilot.autoSwitch")} description={t("autopilot.autoSwitchDesc")}>
          <Switch checked={ap.autoSwitchStrategy} disabled={busy} onChange={(v) => patch({ autoSwitchStrategy: v })} />
        </FieldRow>

        <FieldRow title={t("autopilot.maxSwitches")} description={t("autopilot.maxSwitchesDesc")}>
          <Segmented
            value={String(ap.maxSwitchesPerHour)}
            disabled={busy}
            onChange={(v) => patch({ maxSwitchesPerHour: Number(v) })}
            options={[
              { value: "1", label: "1" },
              { value: "3", label: "3" },
              { value: "5", label: "5" },
            ]}
          />
        </FieldRow>

        <FieldRow title={t("autopilot.onlyWhenZapret")} description={t("autopilot.onlyWhenZapretDesc")}>
          <Switch checked={ap.onlyWhenZapretRunning} disabled={busy} onChange={(v) => patch({ onlyWhenZapretRunning: v })} />
        </FieldRow>
      </Card>

      <Card>
        <h3 className="mb-3 text-sm font-bold uppercase tracking-wide text-slate-400">
          {t("autopilot.probes")}
        </h3>
        <div className="space-y-1">
          <ProbeRow label="Discord" checked={ap.probeDiscord} disabled={busy} onChange={(v) => patch({ probeDiscord: v })} />
          <ProbeRow label="YouTube" checked={ap.probeYoutube} disabled={busy} onChange={(v) => patch({ probeYoutube: v })} />
          <ProbeRow label="Cloudflare" checked={ap.probeCloudflare} disabled={busy} onChange={(v) => patch({ probeCloudflare: v })} />
          <ProbeRow label="Google" checked={ap.probeGoogle} disabled={busy} onChange={(v) => patch({ probeGoogle: v })} />
        </div>
      </Card>

      <Card className="divide-y divide-[rgb(var(--border)/0.5)]">
        <FieldRow title={t("autopilot.autoWarp")} description={t("autopilot.autoWarpDesc")}>
          <Switch checked={ap.autoEnableWarp} disabled={busy} onChange={(v) => patch({ autoEnableWarp: v })} />
        </FieldRow>
        <FieldRow title={t("autopilot.notifySwitch")}>
          <Switch checked={ap.notifyOnSwitch} disabled={busy} onChange={(v) => patch({ notifyOnSwitch: v })} />
        </FieldRow>
        <FieldRow title={t("autopilot.notifyDegraded")}>
          <Switch checked={ap.notifyOnDegraded} disabled={busy} onChange={(v) => patch({ notifyOnDegraded: v })} />
        </FieldRow>
      </Card>

      <Note tone="info" title={t("autopilot.serviceNoteTitle")}>
        {t("autopilot.serviceNote")}
      </Note>
      {ap.switchMode === "best_by_tests" && (
        <Note tone="warn" title={t("autopilot.bestByTestsNoteTitle")}>
          {t("autopilot.bestByTestsNote")}
        </Note>
      )}
    </div>
  );
}

function Stat({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone?: "ok" | "warn" | "fail" | "off";
}) {
  return (
    <div className="rounded-xl bg-[rgb(var(--surface))] p-3 ring-1 ring-[rgb(var(--border)/0.5)]">
      <div className="text-[10px] font-semibold uppercase tracking-wide text-slate-400">{label}</div>
      <div className="mt-1 text-sm font-semibold text-slate-800 dark:text-slate-100">
        {tone ? <Badge tone={tone}>{value}</Badge> : value}
      </div>
    </div>
  );
}

function ProbeRow({
  label,
  checked,
  disabled,
  onChange,
}: {
  label: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <FieldRow title={label}>
      <Switch checked={checked} disabled={disabled} onChange={onChange} />
    </FieldRow>
  );
}
