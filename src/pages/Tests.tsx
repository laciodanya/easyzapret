import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { api } from "../lib/api";
import { errText } from "../lib/errors";
import { toast } from "../lib/toast";
import { useStore } from "../lib/store";
import {
  Badge,
  Button,
  Card,
  Note,
  PageHeader,
  Segmented,
  Spinner,
  cn,
} from "../components/ui";
import type { TestEvent, TestTargetResult } from "../lib/types";

interface ConfigRun {
  config: string;
  results: TestTargetResult[];
  done: boolean;
  ok: number;
  fail: number;
  error?: string;
}

// Module-level so results survive page switches while tests run in backend.
let savedRuns: ConfigRun[] = [];
let savedBest: string | null = null;

export function TestsPage({ embedded }: { embedded?: boolean } = {}) {
  const { t } = useTranslation();
  const { strategies, settings, status, updateSettings, refreshStatus } = useStore();
  const [mode, setMode] = useState<"standard" | "dpi">("standard");
  const [scope, setScope] = useState<"current" | "all">("current");
  const [runs, setRuns] = useState<ConfigRun[]>(savedRuns);
  const [best, setBest] = useState<string | null>(savedBest);
  const [progress, setProgress] = useState<{ current: number; total: number } | null>(null);
  const running = !!status?.testsRunning;
  const unlistenRef = useRef<UnlistenFn | null>(null);

  useEffect(() => {
    let cancelled = false;
    const updateRuns = (fn: (prev: ConfigRun[]) => ConfigRun[]) => {
      setRuns((prev) => {
        const next = fn(prev);
        savedRuns = next;
        return next;
      });
    };
    listen<TestEvent>("test-event", (event) => {
      if (cancelled) return;
      const e = event.payload;
      switch (e.kind) {
        case "started":
          setBest(null);
          savedBest = null;
          updateRuns(() =>
            e.configs.map((config) => ({ config, results: [], done: false, ok: 0, fail: 0 })),
          );
          break;
        case "config-start":
          setProgress({ current: e.index + 1, total: e.total });
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
            prev.map((r) =>
              r.config === e.config
                ? { ...r, done: true, ok: e.ok, fail: e.fail, error: e.error }
                : r,
            ),
          );
          break;
        case "finished":
          setBest(e.best);
          savedBest = e.best;
          setProgress(null);
          refreshStatus();
          break;
        case "cancelled":
          setProgress(null);
          refreshStatus();
          break;
        case "error":
          toast(errText(t, e.message), "fail");
          setProgress(null);
          refreshStatus();
          break;
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

  async function start() {
    const configs =
      scope === "all"
        ? strategies
        : settings?.selectedStrategy
          ? [settings.selectedStrategy]
          : [];
    if (configs.length === 0) {
      toast(t("home.noStrategy"), "fail");
      return;
    }
    try {
      savedRuns = [];
      setRuns([]);
      setBest(null);
      await api.runTests(mode, configs);
      await refreshStatus();
    } catch (e) {
      toast(errText(t, e), "fail");
    }
  }

  async function cancel() {
    await api.cancelTests();
  }

  const statusTone = (s: string) =>
    s === "ok" ? "ok" : s === "warn" ? "warn" : s === "skipped" ? "off" : "fail";
  const statusLabel = (s: string) =>
    s === "ok"
      ? t("tests.statusOk")
      : s === "warn"
        ? t("tests.statusWarn")
        : s === "timeout"
          ? t("tests.statusTimeout")
          : s === "skipped"
            ? t("tests.statusSkipped")
            : t("tests.statusFail");

  const serviceInstalled = !!status?.zapret.serviceInstalled;

  return (
    <div className={embedded ? "" : "mx-auto max-w-3xl"}>
      {!embedded && (
        <PageHeader title={t("tests.title")} description={t("tests.description")} />
      )}

      <Card className="mb-4">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div className="flex flex-wrap gap-6">
            <div>
              <div className="mb-1.5 text-xs font-semibold uppercase tracking-wide text-slate-400">
                {t("tests.mode")}
              </div>
              <Segmented
                value={mode}
                disabled={running}
                options={[
                  { value: "standard", label: t("tests.standard") },
                  { value: "dpi", label: t("tests.dpi") },
                ]}
                onChange={setMode}
              />
            </div>
            <div>
              <div className="mb-1.5 text-xs font-semibold uppercase tracking-wide text-slate-400">
                {t("tests.scope")}
              </div>
              <Segmented
                value={scope}
                disabled={running}
                options={[
                  { value: "current", label: t("tests.scopeCurrent") },
                  { value: "all", label: t("tests.scopeAll") },
                ]}
                onChange={setScope}
              />
            </div>
          </div>
          {running ? (
            <Button variant="danger" onClick={cancel}>
              <Spinner /> {t("tests.cancel")}
            </Button>
          ) : (
            <Button variant="primary" onClick={start} disabled={serviceInstalled}>
              {t("tests.run")}
            </Button>
          )}
        </div>

        {serviceInstalled && (
          <div className="mt-3">
            <Note tone="warn">{t("tests.warnService")}</Note>
          </div>
        )}
        {!serviceInstalled && !running && (
          <div className="mt-3">
            <Note tone="info" title={t("tests.beforeRunTitle")}>
              {t("tests.warnStops")}
            </Note>
          </div>
        )}

        {progress && (
          <div className="mt-4">
            <div className="mb-1 flex justify-between text-xs text-slate-500">
              <span>{t("tests.progress", { current: progress.current, total: progress.total })}</span>
            </div>
            <div className="h-2 overflow-hidden rounded-full bg-slate-100 dark:bg-slate-800">
              <div
                className="h-full rounded-full bg-teal-500 transition-all"
                style={{ width: `${(progress.current / progress.total) * 100}%` }}
              />
            </div>
          </div>
        )}
      </Card>

      {best && (
        <div className="mb-4 flex items-center justify-between gap-3 rounded-xl bg-teal-500/10 px-4 py-3 text-sm text-teal-800 ring-1 ring-teal-500/25 dark:text-teal-200">
          <span className="font-medium">
            {t("tests.bestConfig", { config: best.replace(/\.bat$/i, "") })}
          </span>
          {best !== settings?.selectedStrategy && (
            <Button variant="primary" onClick={() => updateSettings({ selectedStrategy: best })}>
              {t("tests.useBest")}
            </Button>
          )}
        </div>
      )}

      {runs.length === 0 ? (
        <Card>
          <p className="text-sm text-slate-500">{t("tests.noResults")}</p>
        </Card>
      ) : (
        <div className="space-y-3">
          {runs.map((run) => (
            <Card key={run.config} className="p-4">
              <div className="flex items-center justify-between gap-3">
                <div className="flex items-center gap-2.5">
                  {!run.done && running && <Spinner />}
                  <span className="font-semibold text-slate-800 dark:text-slate-100">
                    {run.config.replace(/\.bat$/i, "")}
                  </span>
                  {run.config === best && <Badge tone="ok">★</Badge>}
                </div>
                {run.done && (
                  <span className="text-xs text-slate-400">
                    {t("tests.okCount", { ok: run.ok, fail: run.fail })}
                  </span>
                )}
              </div>
              {run.error && (
                <div className="mt-2">
                  <Note tone="fail">{run.error}</Note>
                </div>
              )}
              {run.results.length > 0 && (
                <div className="mt-3 divide-y divide-slate-100 dark:divide-slate-800">
                  {run.results.map((r, i) => (
                    <div key={i} className="flex items-center justify-between gap-3 px-2 py-1.5">
                      <div className="min-w-0">
                        <div className="truncate text-sm text-slate-600 dark:text-slate-300">{r.name}</div>
                        {r.detail && (
                          <div className="truncate font-mono text-[11px] leading-relaxed text-slate-400 dark:text-slate-500">
                            {r.detail}
                          </div>
                        )}
                      </div>
                      <Badge tone={statusTone(r.status)}>{statusLabel(r.status)}</Badge>
                    </div>
                  ))}
                </div>
              )}
            </Card>
          ))}
        </div>
      )}

      <p className={cn("mt-4 text-xs text-slate-400", runs.length === 0 && "hidden")}>
        {t("tests.logHint")}
      </p>
    </div>
  );
}
