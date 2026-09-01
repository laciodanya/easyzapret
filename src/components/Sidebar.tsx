import { useTranslation } from "react-i18next";
import { useStore, type Page } from "../lib/store";
import { Logo } from "./Logo";
import { cn } from "./ui";

const ICONS: Record<Page, string> = {
  home: "M3 12l9-9 9 9M5 10v10a1 1 0 001 1h4v-6h4v6h4a1 1 0 001-1V10",
  strategies: "M4 6h16M4 12h16M4 18h10",
  zapret:
    "M9 12.75L11.25 15 15 9.75m-3-7.036A11.959 11.959 0 013.598 6 11.99 11.99 0 003 9.749c0 5.592 3.824 10.29 9 11.623 5.176-1.332 9-6.03 9-11.622 0-1.31-.21-2.571-.598-3.751h-.152c-3.196 0-6.1-1.248-8.25-3.285z",
  autopilot: "M13 10V3L4 14h7v7l9-11h-7z",
  telegram:
    "M21.5 4.5L2.8 11.7c-.8.3-.8 1.4.05 1.7l4.7 1.6 1.8 5.5c.25.75 1.2.9 1.7.3l2.5-2.9 4.9 3.6c.6.45 1.5.1 1.65-.65l3-15c.2-.9-.7-1.65-1.6-1.35z",
  vpn: "M12 3l7 4v5c0 5-3.5 9.4-7 10-3.5-.6-7-5-7-10V7l7-4z",
  warp: "M3 15a4 4 0 004 4h9a5 5 0 10-.1-9.999 5.002 5.002 0 10-9.78 2.096A4.001 4.001 0 003 15z",
  logs: "M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z",
  settings:
    "M12 15a3 3 0 100-6 3 3 0 000 6z M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 01-2.83 2.83l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09a1.65 1.65 0 00-1-1.51 1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06a1.65 1.65 0 00.33-1.82 1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09a1.65 1.65 0 001.51-1 1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06a1.65 1.65 0 001.82.33h0a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51h0a1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06a1.65 1.65 0 00-.33 1.82v0a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z",
};

function NavIcon({ d }: { d: string }) {
  return (
    <svg className="h-[18px] w-[18px] shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.85}>
      <path d={d} strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

export function Sidebar() {
  const { t } = useTranslation();
  const page = useStore((s) => s.page);
  const setPage = useStore((s) => s.setPage);
  const appInfo = useStore((s) => s.appInfo);
  const expanded = useStore((s) => s.sidebarExpanded);
  const setSidebarExpanded = useStore((s) => s.setSidebarExpanded);

  const items: { id: Page; label: string }[] = [
    { id: "home", label: t("nav.home") },
    { id: "autopilot", label: t("nav.autopilot") },
    { id: "strategies", label: t("nav.strategies") },
    { id: "zapret", label: t("nav.zapret") },
    { id: "telegram", label: t("nav.telegram") },
    { id: "vpn", label: t("nav.vpn") },
    { id: "warp", label: t("nav.warp") },
    { id: "logs", label: t("nav.logs") },
    { id: "settings", label: t("nav.settings") },
  ];

  return (
    <aside
      className={cn(
        "flex h-full shrink-0 flex-col border-r border-[rgb(var(--border)/0.5)] bg-[rgb(var(--surface-elevated)/0.92)] backdrop-blur-md transition-[width] duration-300 ease-out",
        expanded ? "w-56" : "w-[72px]",
      )}
    >
      <div className={cn("flex items-center gap-3 pb-4 pt-5", expanded ? "px-4" : "justify-center px-2")}>
        <Logo size={34} />
        {expanded && (
          <div className="min-w-0">
            <div className="truncate text-sm font-bold tracking-tight text-[rgb(var(--text))]">EasyZapret</div>
            <div className="truncate text-[10px] text-[rgb(var(--text-secondary))]">v{appInfo?.version ?? "0.6.0"}</div>
          </div>
        )}
      </div>

      <nav className={cn("flex-1 space-y-0.5 overflow-y-auto", expanded ? "px-2.5" : "px-2")}>
        {items.map((item) => {
          const active = page === item.id;
          return (
            <button
              key={item.id}
              title={item.label}
              onClick={() => setPage(item.id)}
              className={cn(
                "flex w-full items-center rounded-xl text-left text-[13px] font-medium transition-all",
                expanded ? "gap-2.5 px-3 py-2.5" : "justify-center px-0 py-2.5",
                active
                  ? "bg-accent-soft text-accent shadow-sm ring-1 ring-[rgb(var(--accent)/0.22)]"
                  : "text-[rgb(var(--text-secondary))] hover:bg-[rgb(var(--accent)/0.08)] hover:text-[rgb(var(--text))]",
              )}
            >
              <NavIcon d={ICONS[item.id]} />
              {expanded && <span className="truncate">{item.label}</span>}
            </button>
          );
        })}
      </nav>

      <div className={cn("border-t border-[rgb(var(--border)/0.35)] py-3", expanded ? "px-3" : "px-2")}>
        <button
          type="button"
          onClick={() => setSidebarExpanded(!expanded)}
          className={cn(
            "flex w-full items-center rounded-xl py-2 text-[12px] font-medium text-[rgb(var(--text-secondary))] transition-colors hover:bg-[rgb(var(--accent)/0.08)] hover:text-[rgb(var(--text))]",
            expanded ? "gap-2 px-3" : "justify-center",
          )}
          title={expanded ? t("nav.collapseSidebar") : t("nav.expandSidebar")}
        >
          <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            {expanded ? (
              <path strokeLinecap="round" strokeLinejoin="round" d="M15 19l-7-7 7-7" />
            ) : (
              <path strokeLinecap="round" strokeLinejoin="round" d="M9 5l7 7-7 7" />
            )}
          </svg>
          {expanded && <span>{t("nav.collapseSidebar")}</span>}
        </button>
      </div>
    </aside>
  );
}
