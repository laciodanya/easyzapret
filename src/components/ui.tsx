import React, { useEffect, useId, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { copyText } from "../lib/clipboard";
import { usePresence } from "../lib/presence";
import { toast } from "../lib/toast";
import { useSlider } from "../lib/useSlider";
import { Icon, type IconName } from "./icons";

export function cn(...classes: (string | false | null | undefined)[]) {
  return classes.filter(Boolean).join(" ");
}

export type Tone = "ok" | "off" | "warn" | "fail" | "info";

/* ---------- Button ---------- */

type ButtonVariant = "primary" | "secondary" | "danger" | "ghost";
type ButtonSize = "lg" | "md" | "sm";

const BUTTON_VARIANTS: Record<ButtonVariant, string> = {
  primary: "bg-accent text-accent-fg hover:brightness-110 active:brightness-95",
  secondary:
    "border border-line-strong bg-panel text-fg hover:bg-raised active:bg-sunken dark:bg-raised dark:hover:bg-lift",
  danger:
    "border border-fail/30 bg-fail-soft text-fail hover:border-fail/50 active:brightness-95",
  ghost: "text-muted hover:bg-raised hover:text-fg",
};

const BUTTON_SIZES: Record<ButtonSize, string> = {
  lg: "h-10 px-5 text-sm",
  md: "h-9 px-3.5 text-sm",
  sm: "h-8 px-3 text-[13px]",
};

export function Button({
  variant = "secondary",
  size = "md",
  icon,
  className,
  children,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
  size?: ButtonSize;
  icon?: IconName;
}) {
  return (
    <button
      type="button"
      className={cn(
        "inline-flex shrink-0 items-center justify-center gap-2 whitespace-nowrap rounded-control font-medium transition-colors disabled:pointer-events-none disabled:opacity-45",
        BUTTON_VARIANTS[variant],
        BUTTON_SIZES[size],
        className,
      )}
      {...props}
    >
      {icon && <Icon name={icon} size={size === "sm" ? 15 : 16} />}
      {children}
    </button>
  );
}

export function IconButton({
  icon,
  label,
  className,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement> & { icon: IconName; label: string }) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className={cn(
        "inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-control text-muted transition-colors hover:bg-raised hover:text-fg disabled:pointer-events-none disabled:opacity-45",
        className,
      )}
      {...props}
    >
      <Icon name={icon} size={16} />
    </button>
  );
}

/* ---------- Panel: a titled group of rows ---------- */

export function Panel({
  title,
  description,
  action,
  children,
  className,
  padded,
}: {
  title?: string;
  description?: string;
  action?: React.ReactNode;
  children?: React.ReactNode;
  className?: string;
  /** Free-form content instead of divided rows. */
  padded?: boolean;
}) {
  return (
    <section className={cn("rounded-panel border border-line bg-panel", className)}>
      {(title || action) && (
        <header className="flex items-center justify-between gap-4 px-5 py-4">
          <div className="min-w-0">
            {title && <h2 className="text-[15px] font-semibold leading-tight">{title}</h2>}
            {description && (
              <p className="mt-1 max-w-[62ch] text-[13px] leading-snug text-muted">{description}</p>
            )}
          </div>
          {action && <div className="flex shrink-0 items-center gap-2">{action}</div>}
        </header>
      )}
      {children && (
        <div
          className={cn(
            title || action ? "border-t border-line" : "",
            padded ? "p-5" : "divide-y divide-line px-5",
          )}
        >
          {children}
        </div>
      )}
    </section>
  );
}

/* ---------- Row: label on the left, control on the right ---------- */

export function Row({
  title,
  description,
  children,
  align = "center",
}: {
  title: React.ReactNode;
  description?: React.ReactNode;
  children?: React.ReactNode;
  align?: "center" | "start";
}) {
  return (
    <div
      className={cn(
        "flex justify-between gap-6 py-3.5",
        align === "center" ? "items-center" : "items-start",
      )}
    >
      <div className="min-w-0">
        <div className="text-sm font-medium">{title}</div>
        {description && (
          <div className="mt-0.5 max-w-[60ch] text-[13px] leading-snug text-muted">{description}</div>
        )}
      </div>
      {children && <div className="flex shrink-0 items-center gap-2">{children}</div>}
    </div>
  );
}

/* ---------- Switch ---------- */

export function Switch({
  checked,
  onChange,
  disabled,
  label,
}: {
  checked: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
  label?: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn(
        "relative inline-flex h-[22px] w-10 shrink-0 items-center rounded-full transition-colors duration-300 disabled:pointer-events-none disabled:opacity-45",
        checked ? "bg-accent" : "bg-line-strong",
      )}
    >
      <span
        className={cn(
          "absolute left-[3px] h-4 w-4 rounded-full bg-white shadow-sm transition-transform duration-300",
          checked ? "translate-x-[18px]" : "translate-x-0",
        )}
      />
    </button>
  );
}

/* ---------- Status ---------- */

const DOT_COLORS: Record<Tone, string> = {
  ok: "bg-ok",
  off: "bg-faint",
  warn: "bg-warn",
  fail: "bg-fail",
  info: "bg-accent",
};

export function StatusDot({ tone, size = 8 }: { tone: Tone; size?: number }) {
  return (
    <span
      className={cn("inline-block shrink-0 rounded-full", DOT_COLORS[tone])}
      style={{ width: size, height: size }}
    />
  );
}

const BADGE_TONES: Record<Tone, string> = {
  ok: "bg-ok-soft text-ok",
  off: "bg-sunken text-muted",
  warn: "bg-warn-soft text-warn",
  fail: "bg-fail-soft text-fail",
  info: "bg-accent-soft text-accent-text",
};

export function Badge({ tone, children }: { tone: Tone; children: React.ReactNode }) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 rounded-md px-2 py-0.5 text-xs font-medium",
        BADGE_TONES[tone],
      )}
    >
      {children}
    </span>
  );
}

export function Spinner({ className, size = 16 }: { className?: string; size?: number }) {
  return (
    <span
      className={cn(
        "ez-spin inline-block shrink-0 rounded-full border-2 border-line-strong border-t-accent",
        className,
      )}
      style={{ width: size, height: size }}
    />
  );
}

export function ProgressBar({ value }: { value?: number }) {
  return (
    <div className="h-1.5 w-full overflow-hidden rounded-full bg-sunken">
      {value === undefined ? (
        <div className="ez-indeterminate h-full w-2/5 rounded-full bg-accent" />
      ) : (
        <div
          className="h-full rounded-full bg-accent transition-[width] duration-300"
          style={{ width: `${Math.max(0, Math.min(100, value))}%` }}
        />
      )}
    </div>
  );
}

/* ---------- Segmented control ---------- */

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  disabled,
  size = "md",
}: {
  value: T | null;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
  disabled?: boolean;
  size?: "md" | "sm";
}) {
  const ref = useRef<HTMLDivElement>(null);
  const activeIndex = options.findIndex((o) => o.value === value);
  const { box, animate } = useSlider(ref, "[role=radio]", activeIndex, [options.length, size]);

  return (
    <div
      ref={ref}
      role="radiogroup"
      className="relative inline-flex rounded-control border border-line bg-sunken p-0.5"
    >
      {box && activeIndex >= 0 && (
        <span
          aria-hidden
          className={cn(
            "absolute bottom-0.5 top-0.5 rounded-[6px] bg-lift shadow-[0_1px_2px_rgb(0_0_0/0.12)]",
            animate ? "transition-[left,width] duration-300 ease-[var(--ease)]" : "transition-none",
          )}
          style={{ left: box.left, width: box.width }}
        />
      )}
      {options.map((opt) => (
        <button
          key={opt.value}
          type="button"
          role="radio"
          aria-checked={value === opt.value}
          disabled={disabled}
          onClick={() => onChange(opt.value)}
          className={cn(
            "relative rounded-[6px] font-medium transition-colors disabled:pointer-events-none disabled:opacity-45",
            size === "sm" ? "h-7 px-2.5 text-[13px]" : "h-8 px-3 text-[13px]",
            value === opt.value ? "text-fg" : "text-muted hover:text-fg",
          )}
        >
          {opt.label}
        </button>
      ))}
    </div>
  );
}

/* ---------- Tabs (page-level) ---------- */

export function Tabs<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const activeIndex = options.findIndex((o) => o.value === value);
  const { box, animate } = useSlider(ref, "[role=tab]", activeIndex, [options.length]);

  return (
    <div ref={ref} role="tablist" className="relative mb-6 flex gap-1 border-b border-line">
      {box && (
        <span
          aria-hidden
          className={cn(
            "absolute -bottom-px h-0.5 rounded-full bg-accent",
            animate ? "transition-[left,width] duration-300 ease-[var(--ease)]" : "transition-none",
          )}
          style={{ left: box.left, width: box.width }}
        />
      )}
      {options.map((opt) => (
        <button
          key={opt.value}
          type="button"
          role="tab"
          aria-selected={value === opt.value}
          onClick={() => onChange(opt.value)}
          className={cn(
            "h-10 px-3 text-sm font-medium transition-colors",
            value === opt.value ? "text-fg" : "text-muted hover:text-fg",
          )}
        >
          {opt.label}
        </button>
      ))}
    </div>
  );
}

/* ---------- Form controls ---------- */

const FIELD =
  "h-9 rounded-control border border-line-strong bg-sunken px-3 text-sm text-fg transition-colors placeholder:text-faint hover:border-faint focus:border-accent disabled:opacity-50";

export function Input({ className, ...props }: React.InputHTMLAttributes<HTMLInputElement>) {
  return <input className={cn(FIELD, className)} {...props} />;
}

export function Select({
  className,
  children,
  ...props
}: React.SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <select className={cn(FIELD, "pr-8", className)} {...props}>
      {children}
    </select>
  );
}

export function Chip({
  active,
  onClick,
  disabled,
  children,
}: {
  active: boolean;
  onClick: () => void;
  disabled?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      aria-pressed={active}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "inline-flex h-8 items-center gap-1.5 rounded-control border px-3 text-[13px] font-medium transition-colors disabled:pointer-events-none disabled:opacity-45",
        active
          ? "border-accent/50 bg-accent-soft text-accent-text"
          : "border-line-strong bg-panel text-muted hover:text-fg dark:bg-raised",
      )}
    >
      {active && <Icon name="check" size={14} strokeWidth={2.25} />}
      {children}
    </button>
  );
}

/** A read-only value people actually need to copy: address, port, key. */
export function CopyField({
  label,
  value,
  secret,
  mono = true,
}: {
  label: string;
  value: string;
  secret?: boolean;
  mono?: boolean;
}) {
  const { t } = useTranslation();
  const [revealed, setRevealed] = useState(false);
  const hidden = secret && !revealed;

  async function copy() {
    const ok = await copyText(value);
    toast(ok ? t("common.copied", { what: label }) : t("common.copyFailed"), ok ? "ok" : "fail");
  }

  return (
    <div className="flex items-center gap-4 py-3">
      <div className="w-24 shrink-0 text-[13px] text-muted">{label}</div>
      <div
        className={cn(
          "selectable h-9 min-w-0 flex-1 truncate rounded-control border border-line bg-sunken px-3 text-[13px] leading-9",
          mono && "font-mono",
        )}
        title={hidden ? undefined : value}
      >
        {hidden ? "•".repeat(Math.min(value.length, 32)) : value}
      </div>
      <div className="flex shrink-0 gap-1">
        {secret && (
          <IconButton
            icon={revealed ? "eyeOff" : "eye"}
            label={revealed ? t("common.hide") : t("common.show")}
            onClick={() => setRevealed((v) => !v)}
          />
        )}
        <IconButton icon="copy" label={t("common.copy")} onClick={() => void copy()} />
      </div>
    </div>
  );
}

/* ---------- Modal (rendered into <body> so it is always viewport-centred) ---------- */

export function Modal({
  open,
  onClose,
  title,
  children,
  footer,
  size = "md",
}: {
  open: boolean;
  onClose?: () => void;
  title: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
  size?: "sm" | "md" | "lg";
}) {
  const { t } = useTranslation();
  const dialogRef = useRef<HTMLDivElement>(null);
  const onCloseRef = useRef(onClose);
  const titleId = useId();
  const { mounted, state } = usePresence(open, 160);

  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  useEffect(() => {
    if (!open) return;
    const previousFocus = document.activeElement;
    const dialog = dialogRef.current;
    const topmostDialog = () => {
      const dialogs = document.querySelectorAll<HTMLElement>(
        '[data-ez-modal="true"][data-state="open"]',
      );
      return dialogs.item(dialogs.length - 1);
    };
    const focusable = () =>
      Array.from(
        dialog?.querySelectorAll<HTMLElement>(
          'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [contenteditable="true"], [tabindex]:not([tabindex="-1"])',
        ) ?? [],
      ).filter((element) => !element.hasAttribute("hidden") && element.offsetParent !== null);

    const frame = window.requestAnimationFrame(() => {
      if (!dialog || topmostDialog() !== dialog) return;
      if (document.activeElement instanceof HTMLElement && dialog.contains(document.activeElement)) {
        return;
      }
      (focusable()[0] ?? dialog).focus();
    });
    const onKey = (e: KeyboardEvent) => {
      if (!dialog || topmostDialog() !== dialog) return;
      if (e.key === "Escape") {
        if (onCloseRef.current) {
          e.preventDefault();
          onCloseRef.current();
        }
        return;
      }
      if (e.key !== "Tab") return;
      const items = focusable();
      if (items.length === 0) {
        e.preventDefault();
        dialog.focus();
        return;
      }
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement;
      if (e.shiftKey && (active === first || !dialog.contains(active))) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && (active === last || !dialog.contains(active))) {
        e.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => {
      window.cancelAnimationFrame(frame);
      document.removeEventListener("keydown", onKey);
      if (previousFocus instanceof HTMLElement && previousFocus.isConnected) {
        previousFocus.focus();
      }
    };
  }, [open]);

  if (!mounted) return null;

  const width = { sm: "max-w-sm", md: "max-w-md", lg: "max-w-2xl" }[size];

  return createPortal(
    <div
      className={cn(
        "fixed inset-0 z-50 flex items-center justify-center p-6",
        state === "closed" && "pointer-events-none",
      )}
      role="presentation"
    >
      <div
        data-state={state}
        className="ez-scrim absolute inset-0 bg-[var(--scrim)]"
        onClick={onClose}
      />
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
        data-ez-modal="true"
        data-state={state}
        className={cn(
          "ez-modal relative flex max-h-[calc(100vh-48px)] w-full flex-col rounded-modal bg-panel shadow-[var(--shadow-modal)]",
          width,
        )}
      >
        <div className="flex items-start justify-between gap-4 px-6 pt-5">
          <h2 id={titleId} className="text-lg font-semibold leading-snug">{title}</h2>
          {onClose && (
            <IconButton
              icon="x"
              label={t("common.close")}
              onClick={onClose}
              className="-mr-2 -mt-1"
            />
          )}
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto px-6 py-3 text-sm leading-relaxed text-muted">
          {children}
        </div>
        {footer && <div className="flex justify-end gap-2 px-6 pb-5 pt-2">{footer}</div>}
      </div>
    </div>,
    document.body,
  );
}

export function ConfirmModal({
  open,
  title,
  children,
  confirmLabel,
  tone = "primary",
  busy,
  onConfirm,
  onClose,
}: {
  open: boolean;
  title: string;
  children: React.ReactNode;
  confirmLabel: string;
  tone?: "primary" | "danger";
  busy?: boolean;
  onConfirm: () => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  return (
    <Modal
      open={open}
      onClose={busy ? undefined : onClose}
      title={title}
      size="sm"
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={busy}>
            {t("common.cancel")}
          </Button>
          <Button variant={tone} onClick={onConfirm} disabled={busy}>
            {busy && <Spinner size={14} />}
            {confirmLabel}
          </Button>
        </>
      }
    >
      {children}
    </Modal>
  );
}

/* ---------- Note ---------- */

const NOTE_TONES = {
  info: "border-line bg-raised",
  warn: "border-warn/30 bg-warn-soft",
  fail: "border-fail/30 bg-fail-soft",
} as const;

const NOTE_ICONS = { info: "info", warn: "alert", fail: "alert" } as const;
const NOTE_ICON_COLORS = { info: "text-faint", warn: "text-warn", fail: "text-fail" } as const;

export function Note({
  tone = "info",
  title,
  children,
  action,
}: {
  tone?: "info" | "warn" | "fail";
  title?: string;
  children?: React.ReactNode;
  action?: React.ReactNode;
}) {
  return (
    <div className={cn("flex gap-3 rounded-control border px-4 py-3", NOTE_TONES[tone])}>
      <Icon
        name={NOTE_ICONS[tone]}
        size={16}
        className={cn("mt-0.5 shrink-0", NOTE_ICON_COLORS[tone])}
      />
      <div className="min-w-0 flex-1 text-[13px] leading-relaxed text-muted">
        {title && <div className="font-medium text-fg">{title}</div>}
        {children}
      </div>
      {action && <div className="shrink-0 self-center">{action}</div>}
    </div>
  );
}

/* ---------- Page header ---------- */

export function PageHeader({
  title,
  description,
  action,
}: {
  title: string;
  description?: string;
  action?: React.ReactNode;
}) {
  return (
    <div className="mb-6 flex items-start justify-between gap-6">
      <div>
        <h1 className="text-2xl font-semibold leading-tight">{title}</h1>
        {description && (
          <p className="mt-1.5 max-w-[62ch] text-sm leading-relaxed text-muted">{description}</p>
        )}
      </div>
      {action && <div className="flex shrink-0 items-center gap-2 pt-1">{action}</div>}
    </div>
  );
}

export function Empty({ children }: { children: React.ReactNode }) {
  return <div className="py-8 text-center text-[13px] text-faint">{children}</div>;
}
