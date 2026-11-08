import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import { api } from "../lib/api";
import { copyText } from "../lib/clipboard";
import { errText } from "../lib/errors";
import { toast } from "../lib/toast";
import { useStore } from "../lib/store";
import {
  Button,
  Chip,
  ConfirmModal,
  Input,
  Modal,
  PageHeader,
  Spinner,
  Switch,
  cn,
} from "../components/ui";

const TABS = ["app", "autopilot", "zapret", "tgproxy", "warp", "tests"] as const;
type Tab = (typeof TABS)[number];

const ISSUES_URL = "https://github.com/laciodanya/easyzapret/issues/new";

type LineLevel = "info" | "warn" | "error";

function levelOf(line: string): LineLevel {
  const match = /^\[[^\]]+\]\s+(WARN|ERROR)\b/.exec(line);
  if (!match) return "info";
  return match[1] === "ERROR" ? "error" : "warn";
}

function ReportModal({ open, onClose }: { open: boolean; onClose: () => void }) {
  const { t } = useTranslation();
  const version = useStore((s) => s.appInfo?.version);
  const [report, setReport] = useState<string | null>(null);
  const [savedPath, setSavedPath] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!open) return;
    let active = true;
    setReport(null);
    setSavedPath(null);
    api
      .getLogReport(250)
      .then((text) => active && setReport(text))
      .catch((e) => {
        toast(errText(t, e), "fail");
        onClose();
      });
    return () => {
      active = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  async function copy() {
    if (!report) return;
    const ok = await copyText(report);
    toast(ok ? t("logs.reportCopied") : t("common.copyFailed"), ok ? "ok" : "fail");
    return ok;
  }

  async function save() {
    setBusy(true);
    try {
      const path = await api.saveLogReport();
      setSavedPath(path);
      toast(t("logs.reportSaved"), "ok");
    } catch (e) {
      toast(errText(t, e), "fail");
    } finally {
      setBusy(false);
    }
  }

  async function createIssue() {
    // The report is far too long for a URL, so it goes through the clipboard.
    const copied = await copy();
    if (!copied) return;
    const params = new URLSearchParams({
      title: `[v${version ?? "?"}] `,
      body: t("logs.issueBody"),
    });
    await openUrl(`${ISSUES_URL}?${params.toString()}`).catch(() => {});
  }

  const lineCount = report ? report.split("\n").length : 0;

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={t("logs.reportTitle")}
      size="lg"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t("common.close")}
          </Button>
          <Button disabled={!report || busy} onClick={() => void save()}>
            {busy && <Spinner size={14} />}
            {t("logs.reportSave")}
          </Button>
          <Button disabled={!report} onClick={() => void createIssue()}>
            {t("logs.reportIssue")}
          </Button>
          <Button variant="primary" icon="copy" disabled={!report} onClick={() => void copy()}>
            {t("logs.reportCopy")}
          </Button>
        </>
      }
    >
      <p className="mb-3">{t("logs.reportIntro")}</p>
      {report === null ? (
        <div className="flex h-56 items-center justify-center">
          <Spinner />
        </div>
      ) : (
        <>
          <textarea
            readOnly
            value={report}
            spellCheck={false}
            className="h-64 w-full resize-none rounded-control border border-line bg-sunken p-3 font-mono text-[11px] leading-relaxed text-fg"
          />
          <div className="mt-2 text-xs text-faint">
            {t("logs.reportStats", { lines: lineCount, kb: Math.max(1, Math.round(report.length / 1024)) })}
          </div>
        </>
      )}
      {savedPath && (
        <div className="mt-3 flex items-center justify-between gap-3 rounded-control bg-raised px-3 py-2 text-[13px]">
          <span className="selectable min-w-0 truncate font-mono text-xs" title={savedPath}>
            {savedPath}
          </span>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => openPath(savedPath.replace(/[\\/][^\\/]+$/, "")).catch(() => {})}
          >
            {t("logs.openFolder")}
          </Button>
        </div>
      )}
    </Modal>
  );
}

export function LogsPage() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<Tab>("app");
  const [content, setContent] = useState("");
  const [autoScroll, setAutoScroll] = useState(true);
  const [problemsOnly, setProblemsOnly] = useState(false);
  const [query, setQuery] = useState("");
  const [confirmClear, setConfirmClear] = useState(false);
  const [reportOpen, setReportOpen] = useState(false);
  const viewRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let active = true;
    async function tick() {
      try {
        // The TG tab shows the proxy's own log if present, else our launcher log.
        let text = await api.readLog(tab === "tgproxy" ? "tgproxy-own" : tab, 800);
        if (tab === "tgproxy" && !text) text = await api.readLog("tgproxy", 800);
        if (active) setContent(text);
      } catch {
        /* the next tick retries */
      }
    }
    void tick();
    const interval = window.setInterval(() => void tick(), 1500);
    return () => {
      active = false;
      window.clearInterval(interval);
    };
  }, [tab]);

  const lines = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return content
      .split("\n")
      .filter((l) => l.length > 0)
      .map((text) => ({ text, level: levelOf(text) }))
      .filter((l) => (!problemsOnly || l.level !== "info") && (!needle || l.text.toLowerCase().includes(needle)));
  }, [content, problemsOnly, query]);

  useEffect(() => {
    if (autoScroll && viewRef.current) viewRef.current.scrollTop = viewRef.current.scrollHeight;
  }, [lines, autoScroll]);

  async function clear() {
    setConfirmClear(false);
    await api.clearLog(tab);
    setContent("");
  }

  async function copyView() {
    const ok = await copyText(lines.map((l) => l.text).join("\n"));
    toast(ok ? t("logs.copied") : t("common.copyFailed"), ok ? "ok" : "fail");
  }

  return (
    <div className="flex h-[calc(100vh-9rem)] min-h-[420px] flex-col">
      <PageHeader
        title={t("logs.title")}
        description={t("logs.description")}
        action={
          <Button variant="primary" icon="file" onClick={() => setReportOpen(true)}>
            {t("logs.reportButton")}
          </Button>
        }
      />

      <div role="tablist" className="mb-3 flex flex-wrap gap-1">
        {TABS.map((id) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={tab === id}
            onClick={() => setTab(id)}
            className={cn(
              "h-8 rounded-control px-3 text-[13px] font-medium transition-colors",
              tab === id ? "bg-accent-soft text-accent-text" : "text-muted hover:bg-raised hover:text-fg",
            )}
          >
            {t(`logs.${id}`)}
          </button>
        ))}
      </div>

      <div className="mb-3 flex flex-wrap items-center gap-2">
        <Input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t("logs.search")}
          className="w-44"
        />
        <Chip active={problemsOnly} onClick={() => setProblemsOnly((v) => !v)}>
          {t("logs.problemsOnly")}
        </Chip>
        <div className="ml-auto flex items-center gap-2">
          <label className="mr-2 flex items-center gap-2 text-[13px] text-muted">
            {t("logs.autoScroll")}
            <Switch label={t("logs.autoScroll")} checked={autoScroll} onChange={setAutoScroll} />
          </label>
          <Button size="sm" variant="ghost" disabled={lines.length === 0} onClick={() => void copyView()}>
            {t("logs.copy")}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => void api.logsDirPath().then((dir) => openPath(dir)).catch(() => {})}
          >
            {t("logs.openFolder")}
          </Button>
          <Button size="sm" variant="danger" onClick={() => setConfirmClear(true)} disabled={!content}>
            {t("logs.clear")}
          </Button>
        </div>
      </div>

      <div
        ref={viewRef}
        className="selectable min-h-0 flex-1 overflow-auto rounded-panel border border-line bg-sunken p-4 font-mono text-[11.5px] leading-[1.65]"
      >
        {lines.length === 0 ? (
          <span className="font-sans text-[13px] text-faint">{t("logs.empty")}</span>
        ) : (
          lines.map((l, i) => (
            <div
              key={i}
              className={cn(
                "whitespace-pre-wrap break-all",
                l.level === "error" && "text-fail",
                l.level === "warn" && "text-warn",
                l.level === "info" && "text-muted",
              )}
            >
              {l.text}
            </div>
          ))
        )}
      </div>

      <ConfirmModal
        open={confirmClear}
        title={t("logs.clear")}
        tone="danger"
        confirmLabel={t("common.confirm")}
        onClose={() => setConfirmClear(false)}
        onConfirm={() => void clear()}
      >
        {t("logs.clearConfirm", { name: t(`logs.${tab}`) })}
      </ConfirmModal>

      <ReportModal open={reportOpen} onClose={() => setReportOpen(false)} />
    </div>
  );
}
