import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { openPath } from "@tauri-apps/plugin-opener";
import { api } from "../lib/api";
import { Button, Card, Modal, PageHeader, Switch, cn } from "../components/ui";

const TABS = ["zapret", "tgproxy", "warp", "tests", "app"] as const;
type Tab = (typeof TABS)[number];

export function LogsPage() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<Tab>("zapret");
  const [content, setContent] = useState("");
  const [autoScroll, setAutoScroll] = useState(true);
  const [confirmClear, setConfirmClear] = useState(false);
  const preRef = useRef<HTMLPreElement>(null);

  useEffect(() => {
    let active = true;
    async function tick() {
      try {
        // The TG tab shows the proxy's own log if present, else our launcher log.
        let text = await api.readLog(tab === "tgproxy" ? "tgproxy-own" : tab, 800);
        if (tab === "tgproxy" && !text) {
          text = await api.readLog("tgproxy", 800);
        }
        if (active) setContent(text);
      } catch {
        /* ignore */
      }
    }
    tick();
    const interval = setInterval(tick, 1500);
    return () => {
      active = false;
      clearInterval(interval);
    };
  }, [tab]);

  useEffect(() => {
    if (autoScroll && preRef.current) {
      preRef.current.scrollTop = preRef.current.scrollHeight;
    }
  }, [content, autoScroll]);

  async function clear() {
    setConfirmClear(false);
    await api.clearLog(tab === "tgproxy" ? "tgproxy" : tab);
    setContent("");
  }

  async function openFolder() {
    const dir = await api.logsDirPath();
    await openPath(dir).catch(() => {});
  }

  return (
    <div className="mx-auto flex h-full max-w-4xl flex-col">
      <PageHeader title={t("logs.title")} />

      <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
        <div className="flex gap-1.5">
          {TABS.map((id) => (
            <button
              key={id}
              onClick={() => setTab(id)}
              className={cn(
                "rounded-lg px-3 py-1.5 text-sm font-medium transition-colors",
                tab === id
                  ? "bg-accent-soft text-accent ring-1 ring-[rgb(var(--accent)/0.2)]"
                  : "text-slate-500 hover:bg-[rgb(var(--accent)/0.08)]",
              )}
            >
              {t(`logs.${id}`)}
            </button>
          ))}
        </div>
        <div className="flex items-center gap-3">
          <label className="flex items-center gap-2 text-xs text-slate-500">
            {t("logs.autoScroll")}
            <Switch checked={autoScroll} onChange={setAutoScroll} />
          </label>
          <Button variant="ghost" onClick={openFolder}>
            {t("logs.openFolder")}
          </Button>
          <Button variant="danger" onClick={() => setConfirmClear(true)} disabled={!content}>
            {t("logs.clear")}
          </Button>
        </div>
      </div>

      <Card className="min-h-0 flex-1 p-0">
        <pre
          ref={preRef}
          className="h-full overflow-auto whitespace-pre-wrap break-all p-4 font-mono text-[11px] leading-relaxed text-slate-600 dark:text-slate-300"
        >
          {content || <span className="text-slate-400">{t("logs.empty")}</span>}
        </pre>
      </Card>

      <Modal
        open={confirmClear}
        onClose={() => setConfirmClear(false)}
        title={t("logs.clear")}
        footer={
          <>
            <Button onClick={() => setConfirmClear(false)}>{t("common.cancel")}</Button>
            <Button variant="danger" onClick={clear}>
              {t("common.confirm")}
            </Button>
          </>
        }
      >
        {t("logs.clearConfirm", { name: t(`logs.${tab}`) })}
      </Modal>
    </div>
  );
}
