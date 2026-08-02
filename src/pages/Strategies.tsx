import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../lib/api";
import { errText } from "../lib/errors";
import { toast } from "../lib/toast";
import { useStore } from "../lib/store";
import { Badge, Button, Card, Note, PageHeader, Spinner, cn } from "../components/ui";

type StrategyGroup = "base" | "alt" | "fakeTls" | "simpleFake";

const GROUP_ORDER: StrategyGroup[] = ["base", "alt", "fakeTls", "simpleFake"];

function prettyName(filename: string) {
  return filename.replace(/\.bat$/i, "");
}

function strategyGroup(filename: string): StrategyGroup {
  const pretty = prettyName(filename);
  if (pretty === "general") return "base";
  if (pretty.includes("FAKE TLS")) return "fakeTls";
  if (pretty.includes("SIMPLE FAKE")) return "simpleFake";
  return "alt";
}

export function StrategiesPage() {
  const { t } = useTranslation();
  const { strategies, settings, status, updateSettings, refreshStatus } = useStore();
  const [restarting, setRestarting] = useState(false);
  const [query, setQuery] = useState("");

  const selected = settings?.selectedStrategy;
  const zapretRunning = !!status?.zapret.running;

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return strategies;
    return strategies.filter((name) => prettyName(name).toLowerCase().includes(q));
  }, [strategies, query]);

  const grouped = useMemo(() => {
    const map = new Map<StrategyGroup, string[]>();
    for (const g of GROUP_ORDER) map.set(g, []);
    for (const name of filtered) {
      map.get(strategyGroup(name))!.push(name);
    }
    return GROUP_ORDER.map((id) => ({ id, items: map.get(id)! })).filter((g) => g.items.length > 0);
  }, [filtered]);

  async function select(name: string) {
    if (name === selected) return;
    await updateSettings({ selectedStrategy: name });
  }

  async function restartWithSelected() {
    if (!selected) return;
    setRestarting(true);
    try {
      await api.stopZapret();
      await api.startZapret(selected);
      await refreshStatus();
      toast(t("common.saved"), "ok");
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setRestarting(false);
    }
  }

  function groupLabel(id: StrategyGroup) {
    switch (id) {
      case "base":
        return t("strategies.groupBase");
      case "alt":
        return t("strategies.groupAlt");
      case "fakeTls":
        return t("strategies.groupFakeTls");
      case "simpleFake":
        return t("strategies.groupSimpleFake");
      default: {
        const _exhaustive: never = id;
        return _exhaustive;
      }
    }
  }

  return (
    <div className="mx-auto max-w-3xl">
      <PageHeader title={t("strategies.title")} description={t("strategies.description")} />

      {selected && (
        <Card className="mb-4 flex items-center justify-between gap-4 bg-accent-soft border-accent-soft">
          <div className="min-w-0">
            <div className="text-xs font-semibold uppercase tracking-wide text-accent">
              {t("strategies.selected")}
            </div>
            <div className="mt-0.5 truncate text-lg font-bold text-slate-900 dark:text-white">
              {prettyName(selected)}
            </div>
          </div>
          <Badge tone="ok">{t("strategies.current")}</Badge>
        </Card>
      )}

      {zapretRunning && status?.zapret.currentStrategy !== selected && (
        <div className="mb-4 flex items-center justify-between gap-3 rounded-xl bg-amber-500/10 px-4 py-3 text-sm text-amber-800 ring-1 ring-amber-500/25 dark:text-amber-200">
          <span>{t("strategies.restartHint")}</span>
          <Button variant="primary" onClick={restartWithSelected} disabled={restarting}>
            {restarting ? <Spinner /> : null}
            {t("strategies.restartNow")}
          </Button>
        </div>
      )}

      {strategies.length === 0 ? (
        <Card>
          <p className="text-sm text-slate-500">{t("strategies.empty")}</p>
        </Card>
      ) : (
        <>
          <div className="mb-4 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <span className="text-sm text-slate-500">{t("strategies.count", { count: strategies.length })}</span>
            <input
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t("strategies.search")}
              className="w-full rounded-xl border-0 bg-[rgb(var(--surface-elevated))] px-4 py-2.5 text-sm text-slate-800 ring-1 ring-[rgb(var(--border)/0.65)] placeholder:text-slate-400 focus:outline-none focus:ring-2 focus:ring-accent sm:max-w-xs"
            />
          </div>

          {grouped.length === 0 ? (
            <Card>
              <p className="text-sm text-slate-500">{t("strategies.noResults")}</p>
            </Card>
          ) : (
            <div className="space-y-5">
              {grouped.map(({ id, items }) => (
                <section key={id}>
                  <h3 className="mb-2 px-1 text-xs font-bold uppercase tracking-wider text-slate-400">
                    {groupLabel(id)}
                    <span className="ml-1.5 font-normal text-slate-300 dark:text-slate-600">
                      ({items.length})
                    </span>
                  </h3>
                  <div className="overflow-hidden rounded-3xl bg-[rgb(var(--surface-elevated))] ring-1 ring-[rgb(var(--border)/0.55)]">
                    {items.map((name, i) => {
                      const isSelected = name === selected;
                      const label = prettyName(name);
                      return (
                        <button
                          key={name}
                          type="button"
                          onClick={() => select(name)}
                          className={cn(
                            "flex w-full items-center gap-3 px-4 py-3.5 text-left transition-colors",
                            i > 0 && "border-t border-slate-100 dark:border-slate-800",
                            isSelected
                              ? "bg-accent-soft"
                              : "hover:bg-slate-50 dark:hover:bg-slate-800/50",
                          )}
                        >
                          <span
                            className={cn(
                              "flex h-5 w-5 shrink-0 items-center justify-center rounded-full border-2 transition-colors",
                              isSelected
                                ? "border-[rgb(var(--accent))] bg-accent text-[rgb(var(--accent-fg))]"
                                : "border-slate-300 dark:border-slate-600",
                            )}
                            aria-hidden
                          >
                            {isSelected && (
                              <svg className="h-3 w-3" viewBox="0 0 12 12" fill="currentColor">
                                <path d="M4.5 9L1.5 6l1-1 2 2 4-4 1 1z" />
                              </svg>
                            )}
                          </span>
                          <span
                            className={cn(
                              "min-w-0 flex-1 truncate text-sm",
                              isSelected
                                ? "font-semibold text-accent"
                                : "font-medium text-slate-700 dark:text-slate-200",
                            )}
                          >
                            {label}
                          </span>
                        </button>
                      );
                    })}
                  </div>
                </section>
              ))}
            </div>
          )}
        </>
      )}

      <div className="mt-5">
        <Note tone="info">{t("strategies.editorSoon")}</Note>
      </div>
    </div>
  );
}
