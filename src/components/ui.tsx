import React from "react";

export function cn(...classes: (string | false | null | undefined)[]) {
  return classes.filter(Boolean).join(" ");
}

/* ---------- Button ---------- */

type ButtonVariant = "primary" | "secondary" | "danger" | "ghost";

export function Button({
  variant = "secondary",
  className,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement> & { variant?: ButtonVariant }) {
  const variants: Record<ButtonVariant, string> = {
    primary:
      "bg-accent text-[rgb(var(--accent-fg))] hover:opacity-90 active:opacity-80 disabled:bg-slate-300 dark:disabled:bg-slate-700",
    secondary:
      "bg-white text-slate-700 ring-1 ring-slate-200 hover:bg-slate-50 dark:bg-[rgb(var(--surface-elevated))] dark:text-[rgb(var(--text))] dark:ring-[rgb(var(--border)/0.65)] dark:hover:bg-[rgb(var(--accent)/0.08)]",
    danger:
      "bg-red-600/10 text-red-600 ring-1 ring-red-600/30 hover:bg-red-600/20 dark:text-red-400",
    ghost: "text-slate-600 hover:bg-slate-100 dark:text-[rgb(var(--text-secondary))] dark:hover:bg-[rgb(var(--accent)/0.08)]",
  };
  return (
    <button
      className={cn(
        "inline-flex items-center justify-center gap-2 rounded-xl px-4 py-2 text-sm font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-60",
        variants[variant],
        className,
      )}
      {...props}
    />
  );
}

/* ---------- Card ---------- */

export function Card({
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn(
        "rounded-3xl bg-[rgb(var(--surface-elevated)/0.92)] p-5 shadow-sm ring-1 ring-[rgb(var(--border)/0.55)] backdrop-blur-md",
        className,
      )}
      {...props}
    />
  );
}

/* ---------- Switch ---------- */

export function Switch({
  checked,
  onChange,
  disabled,
  size = "md",
}: {
  checked: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
  size?: "md" | "lg";
}) {
  const dims =
    size === "lg"
      ? { track: "h-9 w-16", thumb: "h-7 w-7", translate: "translate-x-7" }
      : { track: "h-6 w-11", thumb: "h-4.5 w-4.5", translate: "translate-x-5" };
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn(
        "relative inline-flex shrink-0 items-center rounded-full p-1 transition-colors disabled:cursor-not-allowed disabled:opacity-50",
        dims.track,
        checked ? "bg-accent" : "bg-slate-300 dark:bg-slate-700",
      )}
    >
      <span
        className={cn(
          "inline-block transform rounded-full bg-white shadow transition-transform",
          dims.thumb,
          checked ? dims.translate : "translate-x-0",
        )}
      />
    </button>
  );
}

/* ---------- Badge ---------- */

export function Badge({
  tone,
  children,
}: {
  tone: "ok" | "off" | "warn" | "fail" | "info";
  children: React.ReactNode;
}) {
  const tones = {
    ok: "bg-accent-soft text-accent",
    off: "bg-slate-500/15 text-slate-600 dark:text-slate-300",
    warn: "bg-amber-500/15 text-amber-700 dark:text-amber-300",
    fail: "bg-red-500/15 text-red-700 dark:text-red-300",
    info: "bg-blue-500/15 text-blue-700 dark:text-blue-300",
  };
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-semibold",
        tones[tone],
      )}
    >
      {children}
    </span>
  );
}

export function StatusDot({ tone }: { tone: "ok" | "off" | "warn" | "fail" }) {
  const colors = {
    ok: "bg-accent",
    off: "bg-slate-400",
    warn: "bg-amber-500",
    fail: "bg-red-500",
  };
  return <span className={cn("inline-block h-2 w-2 rounded-full", colors[tone])} />;
}

/* ---------- Spinner ---------- */

export function Spinner({ className }: { className?: string }) {
  return (
    <span
      className={cn(
        "inline-block h-4 w-4 animate-spin rounded-full border-2 border-slate-300 border-t-[rgb(var(--accent))]",
        className,
      )}
    />
  );
}

/* ---------- Segmented control ---------- */

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  disabled,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
  disabled?: boolean;
}) {
  return (
    <div className="inline-flex rounded-xl bg-slate-100 p-1 dark:bg-[rgb(var(--surface))]">
      {options.map((opt) => (
        <button
          key={opt.value}
          type="button"
          disabled={disabled}
          onClick={() => onChange(opt.value)}
          className={cn(
            "rounded-lg px-3 py-1.5 text-sm font-medium transition-colors disabled:opacity-50",
            value === opt.value
              ? "bg-[rgb(var(--surface-elevated))] text-slate-900 shadow-sm dark:text-white"
              : "text-slate-500 hover:text-slate-800 dark:text-slate-400 dark:hover:text-slate-200",
          )}
        >
          {opt.label}
        </button>
      ))}
    </div>
  );
}

/* ---------- Modal ---------- */

export function Modal({
  open,
  onClose,
  title,
  children,
  footer,
  wide,
}: {
  open: boolean;
  onClose?: () => void;
  title: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
  wide?: boolean;
}) {
  if (!open) return null;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div
        className="absolute inset-0 bg-slate-950/55 backdrop-blur-sm"
        onClick={onClose}
      />
      <div
        className={cn(
          "ez-fade-up relative z-10 max-h-[85vh] w-full overflow-y-auto rounded-3xl bg-white p-6 shadow-2xl ring-1 ring-black/5 dark:bg-[rgb(var(--surface-elevated))] dark:ring-white/5",
          wide ? "max-w-2xl" : "max-w-md",
        )}
      >
        <div className="mb-3 flex items-start justify-between gap-3">
          <h2 className="text-lg font-bold tracking-tight text-slate-900 dark:text-white">{title}</h2>
          {onClose && (
            <button
              type="button"
              aria-label="Close"
              onClick={onClose}
              className="flex h-8 w-8 shrink-0 items-center justify-center rounded-xl text-slate-400 transition hover:bg-[rgb(var(--accent)/0.1)] hover:text-[rgb(var(--text))]"
            >
              <svg className="h-4 w-4" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth={1.8}>
                <path d="M2 2l8 8M10 2L2 10" strokeLinecap="round" />
              </svg>
            </button>
          )}
        </div>
        <div className="text-sm text-slate-600 dark:text-slate-300">{children}</div>
        {footer && <div className="mt-5 flex justify-end gap-2">{footer}</div>}
      </div>
    </div>
  );
}

/* ---------- Field row (label + control) ---------- */

export function FieldRow({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-start justify-between gap-6 py-3">
      <div className="min-w-0">
        <div className="text-sm font-semibold text-slate-800 dark:text-slate-100">{title}</div>
        {description && (
          <div className="mt-0.5 text-xs leading-relaxed text-slate-500 dark:text-slate-400">
            {description}
          </div>
        )}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

/* ---------- Info note ---------- */

export function Note({
  tone = "info",
  title,
  children,
}: {
  tone?: "info" | "warn" | "fail";
  title?: string;
  children: React.ReactNode;
}) {
  const tones = {
    info: {
      panel: "border-blue-500/20 bg-blue-500/[0.07]",
      icon: "bg-blue-500/12 text-blue-600 dark:text-blue-300",
    },
    warn: {
      panel: "border-amber-500/25 bg-amber-500/[0.07]",
      icon: "bg-amber-500/12 text-amber-700 dark:text-amber-300",
    },
    fail: {
      panel: "border-red-500/25 bg-red-500/[0.07]",
      icon: "bg-red-500/12 text-red-700 dark:text-red-300",
    },
  };
  const style = tones[tone];
  return (
    <div className={cn("flex gap-3 rounded-2xl border p-4", style.panel)}>
      <div className={cn("flex h-8 w-8 shrink-0 items-center justify-center rounded-xl", style.icon)}>
        <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
          {tone === "info" ? (
            <path strokeLinecap="round" strokeLinejoin="round" d="M12 16v-4m0-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          ) : tone === "warn" ? (
            <path strokeLinecap="round" strokeLinejoin="round" d="M12 9v4m0 4h.01M10.3 3.7L2.6 17a2 2 0 001.7 3h15.4a2 2 0 001.7-3L13.7 3.7a2 2 0 00-3.4 0z" />
          ) : (
            <path strokeLinecap="round" strokeLinejoin="round" d="M12 9v3m0 4h.01M10.3 3.7L2.6 17a2 2 0 001.7 3h15.4a2 2 0 001.7-3L13.7 3.7a2 2 0 00-3.4 0z" />
          )}
        </svg>
      </div>
      <div className="min-w-0 pt-0.5 text-xs leading-relaxed text-slate-600 dark:text-[rgb(var(--text-secondary))]">
        {title && <div className="mb-1 text-sm font-semibold text-slate-900 dark:text-[rgb(var(--text))]">{title}</div>}
        {children}
      </div>
    </div>
  );
}

/* ---------- Page header ---------- */

export function PageHeader({ title, description }: { title: string; description?: string }) {
  return (
    <div className="ez-fade-up mb-6">
      <h1 className="text-2xl font-bold tracking-tight text-slate-900 dark:text-white">{title}</h1>
      {description && (
        <p className="mt-1.5 max-w-2xl text-sm leading-relaxed text-slate-500 dark:text-slate-400">
          {description}
        </p>
      )}
    </div>
  );
}
