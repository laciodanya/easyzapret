import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../lib/api";
import { errText } from "../lib/errors";
import { toast } from "../lib/toast";
import { useStore } from "../lib/store";
import { Icon } from "./icons";
import { Empty, Input, Modal, Spinner, cn } from "./ui";

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
    <Modal
      open={open}
      onClose={busy ? undefined : onClose}
      title={t("home.pickStrategy")}
      size="lg"
    >
      <Input
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder={t("strategies.search")}
        className="mb-4 w-full"
        autoFocus
      />
      <div className="max-h-[52vh] space-y-5 overflow-y-auto pr-1">
        {busy && (
          <div className="flex items-center justify-center gap-2 py-6">
            <Spinner />
            <span>{t("common.loading")}</span>
          </div>
        )}
        {!busy &&
          grouped.map((group) => (
            <div key={group.id}>
              <div className="mb-1.5 text-xs font-medium text-faint">{groupLabel(group.id)}</div>
              <div className="grid grid-cols-2 gap-1">
                {group.items.map((name) => {
                  const active = name === selected;
                  return (
                    <button
                      key={name}
                      type="button"
                      disabled={busy}
                      onClick={() => void pick(name)}
                      className={cn(
                        "flex h-9 items-center justify-between gap-2 rounded-control px-3 text-left text-[13px] transition-colors",
                        active
                          ? "bg-accent-soft font-medium text-accent-text"
                          : "text-fg hover:bg-raised",
                      )}
                    >
                      <span className="truncate">{prettyName(name)}</span>
                      {active && <Icon name="check" size={15} strokeWidth={2.25} />}
                    </button>
                  );
                })}
              </div>
            </div>
          ))}
        {!busy && grouped.length === 0 && <Empty>{t("strategies.empty")}</Empty>}
      </div>
    </Modal>
  );
}
