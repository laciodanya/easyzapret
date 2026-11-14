import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { formatClock } from "../lib/format";
import { usePresence } from "../lib/presence";
import { useStore } from "../lib/store";
import { Icon } from "./icons";
import { Button, cn } from "./ui";

type Notice = "available" | "upToDate" | "failed";

interface UpdateItem {
  name: string;
  detail: string;
}

const AUTO_CLOSE_MS = 3500;

/** Title-bar button that checks for updates and reports back in a small popover. */
export function UpdateButton() {
  const { t } = useTranslation();
  const updates = useStore((s) => s.updates);
  const dataUpdates = useStore((s) => s.dataUpdates);
  const appUpdate = useStore((s) => s.appUpdate);
  const checkedAt = useStore((s) => s.updatesCheckedAt);
  const checkUpdates = useStore((s) => s.checkUpdates);
  const openUpdatesModal = useStore((s) => s.openUpdatesModal);

  const [checking, setChecking] = useState(false);
  const [notice, setNotice] = useState<Notice | null>(null);
  const lastNotice = useRef<Notice>("upToDate");
  const seenSignature = useRef("");
  const root = useRef<HTMLDivElement>(null);

  const items: UpdateItem[] = [];
  if (appUpdate?.updateAvailable && appUpdate.latest) {
    items.push({ name: "EasyZapret", detail: `${appUpdate.current ?? "—"} → ${appUpdate.latest}` });
  }
  for (const u of updates ?? []) {
    if (!u.updateAvailable || !u.latest) continue;
    items.push({
      name: u.component === "zapret" ? "Zapret" : "Telegram Proxy",
      detail: `${u.installed ?? "—"} → ${u.latest}`,
    });
  }
  for (const update of dataUpdates ?? []) {
    if (!update.updateAvailable) continue;
    items.push({
      name: update.component === "ipset" ? t("updates.ipset") : t("updates.hosts"),
      detail: t("updates.dataUpdateAvailable"),
    });
  }
  const signature = items.map((i) => `${i.name}:${i.detail}`).join("|");
  const hasUpdates = items.length > 0;

  // Background checks: surface a new set of updates once, without being asked.
  useEffect(() => {
    if (!signature || signature === seenSignature.current) return;
    seenSignature.current = signature;
    setNotice("available");
  }, [signature]);

  // Good news and errors fade away on their own; updates wait for a decision.
  useEffect(() => {
    if (notice === null || notice === "available") return;
    const timer = window.setTimeout(() => setNotice(null), AUTO_CLOSE_MS);
    return () => window.clearTimeout(timer);
  }, [notice]);

  useEffect(() => {
    if (notice === null) return;
    const onPointer = (e: PointerEvent) => {
      if (root.current && !root.current.contains(e.target as Node)) setNotice(null);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setNotice(null);
    };
    document.addEventListener("pointerdown", onPointer);
    window.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", onPointer);
      window.removeEventListener("keydown", onKey);
    };
  }, [notice]);

  async function onClick() {
    if (checking) return;
    if (notice !== null) {
      setNotice(null);
      return;
    }
    if (hasUpdates) {
      setNotice("available");
      return;
    }
    setChecking(true);
    try {
      await checkUpdates();
    } finally {
      setChecking(false);
    }
    const { updates: nextUpdates, appUpdate: nextApp, updatesError } = useStore.getState();
    const found = !!nextApp?.updateAvailable || !!nextUpdates?.some((u) => u.updateAvailable);
    setNotice(found ? "available" : updatesError ? "failed" : "upToDate");
  }

  if (notice) lastNotice.current = notice;
  const shown = notice ?? lastNotice.current;
  const presence = usePresence(notice !== null, 140);

  const label = checking ? t("updates.checking") : t("updates.check");

  return (
    <div ref={root} className="h-full">
      <button
        type="button"
        aria-label={label}
        title={label}
        onClick={() => void onClick()}
        className={cn(
          "relative flex h-full w-11 items-center justify-center transition-colors hover:bg-raised",
          hasUpdates ? "text-accent-text" : "text-muted hover:text-fg",
        )}
      >
        <span key={checking ? "checking" : hasUpdates ? "update" : "idle"} className="ez-fade flex">
          <span className={cn("flex", checking && "ez-spin")}>
            <Icon name={hasUpdates && !checking ? "download" : "refresh"} size={15} strokeWidth={1.6} />
          </span>
        </span>
        {hasUpdates && !checking && (
          <span className="absolute right-3 top-2 h-2 w-2 rounded-full bg-accent ring-2 ring-app" />
        )}
      </button>

      {presence.mounted && (
        <div
          role="status"
          data-state={presence.state}
          className={cn(
            "ez-drop fixed right-3 top-[42px] z-40 w-80 origin-top-right rounded-panel border border-line-strong bg-lift p-4 text-[13px] shadow-[var(--shadow-modal)]",
            presence.state === "closed" && "pointer-events-none",
          )}
        >
          {shown === "available" ? (
            <>
              <div className="text-sm font-semibold">{t("updates.availableTitle")}</div>
              <ul className="mt-3 space-y-2">
                {items.map((item) => (
                  <li key={item.name} className="flex items-baseline justify-between gap-3">
                    <span className="font-medium">{item.name}</span>
                    <span className="truncate text-muted">{item.detail}</span>
                  </li>
                ))}
              </ul>
              <div className="mt-4 flex justify-end gap-2">
                <Button size="sm" variant="ghost" onClick={() => setNotice(null)}>
                  {t("common.later")}
                </Button>
                <Button
                  size="sm"
                  variant="primary"
                  onClick={() => {
                    setNotice(null);
                    openUpdatesModal();
                  }}
                >
                  {t("updates.install")}
                </Button>
              </div>
            </>
          ) : (
            <div className="flex items-center gap-3">
              <span className={shown === "failed" ? "text-fail" : "text-ok"}>
                <Icon name={shown === "failed" ? "alert" : "check"} size={18} />
              </span>
              <div className="min-w-0">
                <div className="font-medium">
                  {shown === "failed" ? t("updates.failed") : t("updates.upToDate")}
                </div>
                {checkedAt && <div className="text-xs text-muted">{formatClock(checkedAt)}</div>}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
