import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Icon } from "./icons";
import { Logo } from "./Logo";
import { UpdateButton } from "./UpdateButton";
import { cn } from "./ui";

function WinBtn({
  label,
  onClick,
  danger,
  children,
}: {
  label: string;
  onClick: () => void;
  danger?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className={cn(
        "flex h-full w-11 items-center justify-center text-muted transition-colors",
        danger ? "hover:bg-fail hover:text-white" : "hover:bg-raised hover:text-fg",
      )}
    >
      {children}
    </button>
  );
}

export function TitleBar() {
  const { t } = useTranslation();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    const win = getCurrentWindow();
    let unlisten: (() => void) | undefined;
    win
      .isMaximized()
      .then(setMaximized)
      .catch(() => {});
    win
      .onResized(() => {
        win.isMaximized().then(setMaximized).catch(() => {});
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => unlisten?.();
  }, []);

  return (
    <div
      data-tauri-drag-region
      className="flex h-9 shrink-0 items-center justify-between border-b border-line bg-app"
    >
      <div data-tauri-drag-region className="flex min-w-0 items-center gap-2.5 pl-3.5">
        <Logo size={18} />
        <span data-tauri-drag-region className="truncate text-[13px] font-semibold">
          EasyZapret
        </span>
      </div>

      <div className="flex h-full items-stretch">
        <UpdateButton />
        <span aria-hidden className="my-2.5 mx-1 w-px bg-line" />
        <WinBtn
          label={t("window.minimize")}
          onClick={() => void getCurrentWindow().minimize().catch(() => {})}
        >
          <Icon name="minus" size={14} strokeWidth={1.5} />
        </WinBtn>
        <WinBtn
          label={maximized ? t("window.restore") : t("window.maximize")}
          onClick={() => void getCurrentWindow().toggleMaximize().catch(() => {})}
        >
          <Icon name={maximized ? "restore" : "square"} size={13} strokeWidth={1.5} />
        </WinBtn>
        <WinBtn
          label={t("window.close")}
          danger
          onClick={() => void getCurrentWindow().hide().catch(() => {})}
        >
          <Icon name="x" size={14} strokeWidth={1.5} />
        </WinBtn>
      </div>
    </div>
  );
}
