import { useTranslation } from "react-i18next";
import { useStore, type Page } from "../lib/store";
import { Icon, type IconName } from "./icons";
import { StatusDot, cn, type Tone } from "./ui";

const NAV: { id: Page; icon: IconName }[][] = [
  [
    { id: "home", icon: "home" },
    { id: "zapret", icon: "shield" },
    { id: "autopilot", icon: "activity" },
  ],
  [
    { id: "telegram", icon: "send" },
    { id: "warp", icon: "cloud" },
  ],
  [
    { id: "logs", icon: "file" },
    { id: "settings", icon: "sliders" },
  ],
];

function NavButton({
  page,
  icon,
  badge,
}: {
  page: Page;
  icon: IconName;
  badge?: boolean;
}) {
  const { t } = useTranslation();
  const current = useStore((s) => s.page);
  const setPage = useStore((s) => s.setPage);
  const active = current === page;
  return (
    <button
      type="button"
      aria-current={active ? "page" : undefined}
      onClick={() => setPage(page)}
      className={cn(
        "flex h-9 w-full items-center gap-3 rounded-control px-3 text-left text-sm font-medium transition-colors",
        active
          ? "bg-accent-soft text-accent-text"
          : "text-muted hover:bg-raised hover:text-fg",
      )}
    >
      <Icon name={icon} size={18} />
      <span className="flex-1 truncate">{t(`nav.${page}`)}</span>
      {badge && <span className="h-1.5 w-1.5 rounded-full bg-accent" />}
    </button>
  );
}

function StateLine({ page, label, tone, text }: { page: Page; label: string; tone: Tone; text: string }) {
  const setPage = useStore((s) => s.setPage);
  return (
    <button
      type="button"
      onClick={() => setPage(page)}
      className="flex h-7 w-full items-center gap-2.5 rounded-control px-3 text-left text-[13px] text-muted transition-colors hover:bg-raised hover:text-fg"
    >
      <StatusDot tone={tone} size={7} />
      <span className="flex-1 truncate">{label}</span>
      <span className="text-xs text-faint">{text}</span>
    </button>
  );
}

export function Sidebar() {
  const { t } = useTranslation();
  const status = useStore((s) => s.status);
  const appInfo = useStore((s) => s.appInfo);
  const updates = useStore((s) => s.updates);
  const dataUpdates = useStore((s) => s.dataUpdates);
  const appUpdate = useStore((s) => s.appUpdate);

  const hasUpdate =
    !!appUpdate?.updateAvailable ||
    !!updates?.some((u) => u.updateAvailable) ||
    !!dataUpdates?.some((u) => u.updateAvailable);
  const on = t("sidebar.on");
  const off = t("sidebar.off");

  return (
    <aside className="flex h-full w-[220px] shrink-0 flex-col border-r border-line bg-app">
      <nav className="flex flex-1 flex-col gap-4 overflow-y-auto px-3 pt-4">
        {NAV.map((group, i) => (
          <div key={i} className="flex flex-col gap-0.5">
            {group.map((item) => (
              <NavButton
                key={item.id}
                page={item.id}
                icon={item.icon}
                badge={item.id === "settings" && hasUpdate}
              />
            ))}
          </div>
        ))}
      </nav>

      <div className="border-t border-line px-3 py-3">
        <StateLine
          page="zapret"
          label="Zapret"
          tone={status?.zapret.running ? "ok" : "off"}
          text={status?.zapret.running ? on : off}
        />
        <StateLine
          page="telegram"
          label="Telegram"
          tone={status?.tg.running ? "ok" : "off"}
          text={status?.tg.running ? on : off}
        />
        <StateLine
          page="warp"
          label="WARP"
          tone={status?.warp.connected ? "ok" : "off"}
          text={status?.warp.connected ? on : off}
        />
        <div className="mt-2 px-3 text-xs text-faint">v{appInfo?.version ?? ""}</div>
      </div>
    </aside>
  );
}
