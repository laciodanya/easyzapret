import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { api } from "../lib/api";
import { errText } from "../lib/errors";
import { toast } from "../lib/toast";
import { useStore } from "../lib/store";
import { Icon } from "../components/icons";
import {
  Badge,
  Button,
  Chip,
  Note,
  Panel,
  ProgressBar,
  Segmented,
  Spinner,
  cn,
  type Tone,
} from "../components/ui";
import type { ServiceId, TestEvent, TestMode, TestTargetResult } from "../lib/types";

interface ConfigRun {
  config: string;
  results: TestTargetResult[];
  done: boolean;
  error?: string;
}

interface SavedRun {
  runs: ConfigRun[];
  best: string | null;
  mode: TestMode;
  services: string[];
}

const SELECTABLE_SERVICES: ServiceId[] = ["discord", "youtube", "telegram", "cloudflare", "games"];

// Module-level so results survive page switches while tests run in the backend.
let saved: SavedRun = { runs: [], best: null, mode: "service", services: [] };
let savedPicks: { mode: TestMode; services: ServiceId[]; scope: "current" | "all" } = {
  mode: "service",
  services: ["discord", "youtube"],
  scope: "current",
};

function counts(results: TestTargetResult[]) {
  return { ok: results.filter((r) => r.status === "ok").length, total: results.length };
}

function countsTone(ok: number, total: number): Tone {
  if (total === 0) return "off";
  if (ok === total) return "ok";
  if (ok === 0) return "fail";
  return "warn";
}

function resultTone(status: TestTargetResult["status"]): Tone {
  switch (status) {
    case "ok":
      return "ok";
    case "warn":
      return "warn";
    case "skipped":
      return "off";
    case "fail":
    case "timeout":
      return "fail";
    default: {
      const exhaustive: never = status;
      return exhaustive;
    }
  }
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div>
      <div className="mb-2 text-[13px] font-medium text-muted">{label}</div>
      {children}
    </div>
  );
}

export function TestsPage() {
  const { t } = useTranslation();
  const { strategies, settings, status, updateSettings, refreshStatus } = useStore();
  const [mode, setMode] = useState<TestMode>(savedPicks.mode);
  const [scope, setScope] = useState<"current" | "all">(savedPicks.scope);
  const [picked, setPicked] = useState<ServiceId[]>(savedPicks.services);
  const [runs, setRuns] = useState<ConfigRun[]>(saved.runs);
  const [best, setBest] = useState<string | null>(saved.best);
  const [shownMode, setShownMode] = useState<TestMode>(saved.mode);
  const [shownServices, setShownServices] = useState<string[]>(saved.services);
  const [progress, setProgress] = useState<{ current: number; total: number; config: string } | null>(
    null,
  );
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const running = !!status?.testsRunning;
  const unlistenRef = useRef<UnlistenFn | null>(null);

  useEffect(() => {
    savedPicks = { mode, services: picked, scope };
  }, [mode, picked, scope]);

  useEffect(() => {
    let cancelled = false;
    const updateRuns = (fn: (prev: ConfigRun[]) => ConfigRun[]) => {
      setRuns((prev) => {
        const next = fn(prev);
        saved = { ...saved, runs: next };
        return next;
      });
    };
    listen<TestEvent>("test-event", (event) => {
      if (cancelled) return;
      const e = event.payload;
      switch (e.kind) {
        case "started": {
          const startedMode = (e.mode as TestMode) ?? "standard";
          const startedServices = e.services ?? [];
          setBest(null);
          setShownMode(startedMode);
          setShownServices(startedServices);
          setExpanded({});
          saved = { runs: [], best: null, mode: startedMode, services: startedServices };
          updateRuns(() => e.configs.map((config) => ({ config, results: [], done: false })));
          break;
        }
        case "config-start":
          setProgress({ current: e.index + 1, total: e.total, config: e.config });
          break;
        case "target-result":
          updateRuns((prev) =>
            prev.map((r) =>
              r.config === e.result.config ? { ...r, results: [...r.results, e.result] } : r,
            ),
          );
          break;
        case "config-done":
          updateRuns((prev) =>
            prev.map((r) => (r.config === e.config ? { ...r, done: true, error: e.error } : r)),
          );
          break;
        case "finished":
          setBest(e.best);
          saved = { ...saved, best: e.best };
          setProgress(null);
          void refreshStatus();
          break;
        case "cancelled":
          setProgress(null);
          void refreshStatus();
          break;
        case "error":
          toast(errText(t, e.message), "fail");
          setProgress(null);
          void refreshStatus();
          break;
        default: {
          const exhaustive: never = e;
          return exhaustive;
        }
      }
    }).then((un) => {
      unlistenRef.current = un;
    });
    return () => {
      cancelled = true;
      unlistenRef.current?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function toggleService(id: ServiceId) {
    setPicked((prev) => (prev.includes(id) ? prev.filter((s) => s !== id) : [...prev, id]));
  }

  async function start() {
    const configs =
      scope === "all" ? strategies : settings?.selectedStrategy ? [settings.selectedStrategy] : [];
    if (configs.length === 0) {
      toast(t("home.noStrategy"), "fail");
      return;
    }
    if (mode === "service" && picked.length === 0) {
      toast(t("errors.no_services"), "fail");
      return;
    }
    try {
      saved = { runs: [], best: null, mode, services: picked };
      setRuns([]);
      setBest(null);
      await api.runTests(mode, configs, mode === "service" ? picked : []);
      await refreshStatus();
    } catch (e) {
      toast(errText(t, e), "fail");
    }
  }

  const serviceInstalled = !!status?.zapret.serviceInstalled;
  const matrix = shownMode === "service" && shownServices.length > 0;

  // Once a run is finished, the strongest strategies float to the top.
  const finished = !running && runs.length > 0 && runs.every((r) => r.done || r.error);
  const ordered = finished
    ? [...runs].sort((a, b) => counts(b.results).ok - counts(a.results).ok)
    : runs;

  return (
    <div className="space-y-6">
      <Panel title={t("tests.runTitle")} padded>
        <div className="space-y-5">
          <Field label={t("tests.mode")}>
            <div className="flex flex-wrap items-center gap-3">
              <Segmented
                value={mode}
                disabled={running}
                options={[
                  { value: "service", label: t("tests.modeService") },
                  { value: "standard", label: t("tests.standard") },
                  { value: "dpi", label: t("tests.dpi") },
                ]}
                onChange={setMode}
              />
              <span className="text-[13px] text-muted">{t(`tests.modeHint.${mode}`)}</span>
            </div>
          </Field>

          {mode === "service" && (
            <Field label={t("tests.services")}>
              <div className="flex flex-wrap gap-2">
                {SELECTABLE_SERVICES.map((id) => (
                  <Chip
                    key={id}
                    active={picked.includes(id)}
                    disabled={running}
                    onClick={() => toggleService(id)}
                  >
                    {t(`services.${id}`)}
                  </Chip>
                ))}
              </div>
            </Field>
          )}

          <div className="flex flex-wrap items-end justify-between gap-4">
            <Field label={t("tests.scope")}>
              <Segmented
                value={scope}
                disabled={running}
                options={[
                  { value: "current", label: t("tests.scopeCurrent") },
                  { value: "all", label: t("tests.scopeAll", { count: strategies.length }) },
                ]}
                onChange={setScope}
              />
            </Field>
            {running ? (
              <Button variant="danger" onClick={() => void api.cancelTests()}>
                <Spinner size={14} />
                {t("tests.cancel")}
              </Button>
            ) : (
              <Button variant="primary" icon="play" onClick={() => void start()} disabled={serviceInstalled}>
                {t("tests.run")}
              </Button>
            )}
          </div>

          {progress && (
            <div>
              <div className="mb-2 flex justify-between text-[13px] text-muted">
                <span>{t("tests.progress", { current: progress.current, total: progress.total })}</span>
                <span className="font-mono text-xs">{progress.config.replace(/\.bat$/i, "")}</span>
              </div>
              <ProgressBar value={((progress.current - 1) / progress.total) * 100} />
            </div>
          )}

          {serviceInstalled ? (
            <Note tone="warn">{t("tests.warnService")}</Note>
          ) : (
            !running && (
              <Note title={t("tests.beforeRunTitle")}>{t("tests.warnStops")}</Note>
            )
          )}
        </div>
      </Panel>

      {best && (
        <div className="flex items-center justify-between gap-4 rounded-panel border border-ok/30 bg-ok-soft px-5 py-3.5">
          <span className="text-sm font-medium">
            {t("tests.bestConfig", { config: best.replace(/\.bat$/i, "") })}
          </span>
          {best !== settings?.selectedStrategy && (
            <Button size="sm" variant="primary" onClick={() => void updateSettings({ selectedStrategy: best })}>
              {t("tests.useBest")}
            </Button>
          )}
        </div>
      )}

      <Panel title={t("tests.resultsTitle")}>
        {ordered.length === 0 ? (
          <p className="py-6 text-center text-[13px] text-faint">{t("tests.noResults")}</p>
        ) : (
          <div>
            <div className="flex items-center gap-4 py-2.5 text-xs text-faint">
              <span className="w-4" />
              <span className="flex-1">{t("tests.colStrategy")}</span>
              {matrix ? (
                shownServices.map((id) => (
                  <span key={id} className="w-[72px] text-center">
                    {t(`services.${id}`)}
                  </span>
                ))
              ) : (
                <span className="w-[72px] text-center">{t("tests.colResult")}</span>
              )}
            </div>
            {ordered.map((run) => (
              <ResultRow
                key={run.config}
                run={run}
                columns={matrix ? shownServices : null}
                isBest={run.config === best}
                isCurrent={run.config === settings?.selectedStrategy}
                open={!!expanded[run.config]}
                onToggle={() => setExpanded((prev) => ({ ...prev, [run.config]: !prev[run.config] }))}
                onUse={() => void updateSettings({ selectedStrategy: run.config })}
              />
            ))}
          </div>
        )}
      </Panel>

      {ordered.length > 0 && <p className="px-1 text-xs text-faint">{t("tests.logHint")}</p>}
    </div>
  );
}

function ResultRow({
  run,
  columns,
  isBest,
  isCurrent,
  open,
  onToggle,
  onUse,
}: {
  run: ConfigRun;
  columns: string[] | null;
  isBest: boolean;
  isCurrent: boolean;
  open: boolean;
  onToggle: () => void;
  onUse: () => void;
}) {
  const { t } = useTranslation();
  const total = counts(run.results);
  const pending = !run.done && !run.error;

  return (
    <div className="border-t border-line">
      <button
        type="button"
        onClick={onToggle}
        aria-expanded={open}
        className="flex w-full items-center gap-4 py-3 text-left"
      >
        <span className="flex w-4 justify-center text-faint">
          {pending && run.results.length > 0 ? (
            <Spinner size={13} />
          ) : (
            <Icon name={open ? "chevronDown" : "chevronRight"} size={15} />
          )}
        </span>
        <span className="flex min-w-0 flex-1 items-center gap-2">
          <span className="truncate text-sm font-medium">{run.config.replace(/\.bat$/i, "")}</span>
          {isBest && <Badge tone="ok">{t("tests.badgeBest")}</Badge>}
          {isCurrent && <Badge tone="info">{t("tests.badgeCurrent")}</Badge>}
        </span>
        {columns ? (
          columns.map((id) => {
            const c = counts(run.results.filter((r) => r.group === id));
            return (
              <span key={id} className="flex w-[72px] justify-center">
                {c.total === 0 ? (
                  <span className="text-[13px] text-faint">—</span>
                ) : (
                  <Badge tone={countsTone(c.ok, c.total)}>
                    {c.ok}/{c.total}
                  </Badge>
                )}
              </span>
            );
          })
        ) : (
          <span className="flex w-[72px] justify-center">
            {total.total === 0 ? (
              <span className="text-[13px] text-faint">—</span>
            ) : (
              <Badge tone={countsTone(total.ok, total.total)}>
                {total.ok}/{total.total}
              </Badge>
            )}
          </span>
        )}
      </button>

      {open && (
        <div className="mb-3 ml-8 space-y-3 rounded-control bg-raised p-4">
          {run.error && <Note tone="fail">{run.error}</Note>}
          {run.results.length === 0 && !run.error && (
            <p className="text-[13px] text-faint">{t("tests.waiting")}</p>
          )}
          {run.results.length > 0 && (
            <div className="divide-y divide-line">
              {run.results.map((r, i) => (
                <div key={i} className="flex items-start justify-between gap-4 py-2">
                  <div className="min-w-0">
                    <div className="truncate text-[13px]">
                      {columns && r.group && (
                        <span className="mr-2 text-faint">{t(`services.${r.group}`)}</span>
                      )}
                      {r.name}
                    </div>
                    {r.detail && (
                      <div className="selectable mt-0.5 break-all font-mono text-[11px] text-muted">
                        {r.detail}
                      </div>
                    )}
                  </div>
                  <Badge tone={resultTone(r.status)}>{t(`tests.status.${r.status}`)}</Badge>
                </div>
              ))}
            </div>
          )}
          {!isCurrent && (
            <div className={cn("flex justify-end", run.results.length === 0 && "hidden")}>
              <Button size="sm" onClick={onUse}>
                {t("tests.useThis")}
              </Button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
