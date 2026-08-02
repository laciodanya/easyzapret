import { useEffect, useState, type ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Logo } from "./Logo";
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
        "flex h-9 w-11 items-center justify-center text-[rgb(var(--text-secondary))] transition-colors",
        danger
          ? "hover:bg-red-500 hover:text-white"
          : "hover:bg-[rgb(var(--accent)/0.12)] hover:text-[rgb(var(--text))]",
      )}
    >
      {children}
    </button>
  );
}

export function TitleBar() {
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

  async function minimize() {
    await getCurrentWindow().minimize().catch(() => {});
  }

  async function toggleMaximize() {
    await getCurrentWindow().toggleMaximize().catch(() => {});
  }

  async function hideToTray() {
    await getCurrentWindow().hide().catch(() => {});
  }

  return (
    <div
      data-tauri-drag-region
      className="relative z-40 flex h-10 shrink-0 items-center justify-between border-b border-[rgb(var(--border)/0.45)] bg-[rgb(var(--surface-elevated)/0.92)] backdrop-blur-md select-none"
    >
      <div data-tauri-drag-region className="flex min-w-0 items-center gap-2.5 pl-3">
        <Logo size={20} />
        <span data-tauri-drag-region className="truncate text-[13px] font-semibold tracking-tight text-[rgb(var(--text))]">
          EasyZapret
        </span>
      </div>

      <div className="flex h-full items-stretch">
        <WinBtn label="Minimize" onClick={() => void minimize()}>
          <svg className="h-3.5 w-3.5" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth={1.6}>
            <path d="M2 6h8" strokeLinecap="round" />
          </svg>
        </WinBtn>
        <WinBtn label={maximized ? "Restore" : "Maximize"} onClick={() => void toggleMaximize()}>
          {maximized ? (
            <svg className="h-3.5 w-3.5" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth={1.4}>
              <path d="M3.5 4.5h5v5h-5zM4.5 3.5h4.5V8" strokeLinejoin="round" />
            </svg>
          ) : (
            <svg className="h-3.5 w-3.5" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth={1.4}>
              <rect x="2.5" y="2.5" width="7" height="7" rx="0.5" />
            </svg>
          )}
        </WinBtn>
        <WinBtn label="Close" danger onClick={() => void hideToTray()}>
          <svg className="h-3.5 w-3.5" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth={1.6}>
            <path d="M3 3l6 6M9 3L3 9" strokeLinecap="round" />
          </svg>
        </WinBtn>
      </div>
    </div>
  );
}
