import { Icon, type IconName } from "./icons";
import { Spinner, cn } from "./ui";

type PowerSize = "lg" | "md";

const SIZES: Record<PowerSize, { box: string; icon: number; spinner: number }> = {
  lg: { box: "h-32 w-32", icon: 48, spinner: 36 },
  md: { box: "h-16 w-16", icon: 26, spinner: 24 },
};

/** Round on/off button: the main control of the home screen. */
export function PowerButton({
  on,
  busy = false,
  disabled = false,
  label,
  icon = "power",
  size = "lg",
  onClick,
}: {
  on: boolean;
  busy?: boolean;
  disabled?: boolean;
  label: string;
  icon?: IconName;
  size?: PowerSize;
  onClick: () => void;
}) {
  const dims = SIZES[size];
  return (
    <button
      type="button"
      aria-pressed={on}
      aria-label={label}
      disabled={disabled || busy}
      onClick={onClick}
      className={cn(
        "flex shrink-0 items-center justify-center rounded-full transition-[background-color,color,transform,box-shadow] duration-200",
        "enabled:hover:scale-[1.03] enabled:active:scale-95 disabled:cursor-not-allowed",
        dims.box,
        on
          ? "bg-accent text-accent-fg shadow-[0_0_0_6px_var(--accent-soft)]"
          : "bg-raised text-muted shadow-[inset_0_0_0_1px_var(--line-strong)] enabled:hover:text-fg",
        disabled && !busy && "opacity-45",
      )}
    >
      {busy ? (
        <Spinner
          size={dims.spinner}
          className={on ? "border-accent-fg/30 border-t-accent-fg" : undefined}
        />
      ) : (
        <Icon name={icon} size={dims.icon} />
      )}
    </button>
  );
}
