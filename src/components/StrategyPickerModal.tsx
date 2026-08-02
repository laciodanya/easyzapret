import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../lib/api";
import { errText } from "../lib/errors";
import { toast } from "../lib/toast";
import { useStore } from "../lib/store";
import { Button, Modal, Spinner, cn } from "./ui";

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

export function StrategyPickerModal({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const { strategies, settings, status, updateSettings, refreshStatus } = useStore();
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);

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

  async function pick(name: string) {
    if (name === selected) {
      onClose();
      return;
    }
    setBusy(true);
    try {
      await updateSettings({ selectedStrategy: name });
      if (zapretRunning) {
        await api.stopZapret();
        await api.startZapret(name);
        await refreshStatus();
      }
      toast(t("common.saved"), "ok");
      onClose();
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal open={open} onClose={busy ? undefined : onClose} title={t("home.pickStrategy")} wide>
      <div className="mb-3">
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t("strategies.search")}
          className="w-full rounded-xl border border-[rgb(var(--border)/0.7)] bg-[rgb(var(--surface))] px-3 py-2 text-sm outline-none ring-accent focus:ring-2"
        />
      </div>
      <div className="max-h-[50vh] space-y-4 overflow-y-auto pr-1">
        {grouped.map((group) => (
          <div key={group.id}>
            <div className="mb-1.5 text-[11px] font-semibold uppercase tracking-wide text-[rgb(var(--text-secondary))]">
              {groupLabel(group.id)}
            </div>
            <div className="space-y-1">
              {group.items.map((name) => {
                const active = name === selected;
                return (
                  <button
                    key={name}
                    type="button"
                    disabled={busy}
                    onClick={() => pick(name)}
                    className={cn(
                      "flex w-full items-center justify-between rounded-xl px-3 py-2.5 text-left text-sm transition-colors",
                      active
                        ? "bg-accent-soft text-accent ring-1 ring-[rgb(var(--accent)/0.25)]"
                        : "hover:bg-[rgb(var(--accent)/0.08)]",
                    )}
                  >
                    <span className="truncate font-medium">{prettyName(name)}</span>
                    {active && <span className="text-xs font-semibold">{t("strategies.current")}</span>}
                  </button>
                );
              })}
            </div>
          </div>
        ))}
        {grouped.length === 0 && (
          <div className="py-8 text-center text-sm text-[rgb(var(--text-secondary))]">{t("strategies.empty")}</div>
        )}
      </div>
      <div className="mt-4 flex justify-end">
        <Button onClick={onClose} disabled={busy}>
          {busy ? <Spinner /> : null}
          {t("common.later")}
        </Button>
      </div>
    </Modal>
  );
}
