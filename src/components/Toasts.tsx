import { useToasts, type ToastItem } from "../lib/toast";
import { StatusDot, type Tone } from "./ui";

const TONES: Record<ToastItem["tone"], Tone> = { ok: "ok", fail: "fail", info: "info" };

export function Toasts() {
  const { toasts, remove } = useToasts();
  if (toasts.length === 0) return null;
  return (
    <div
      className="pointer-events-none fixed bottom-5 right-5 z-[60] flex w-[340px] flex-col gap-2"
      aria-live="polite"
    >
      {toasts.map((item) => (
        <button
          key={item.id}
          type="button"
          onClick={() => remove(item.id)}
          className="ez-toast pointer-events-auto flex items-start gap-3 rounded-control bg-lift px-4 py-3 text-left text-[13px] leading-snug text-fg shadow-[var(--shadow-modal)]"
        >
          <span className="mt-[6px] flex">
            <StatusDot tone={TONES[item.tone]} />
          </span>
          <span className="min-w-0 flex-1">{item.message}</span>
        </button>
      ))}
    </div>
  );
}
