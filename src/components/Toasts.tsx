import { useToasts } from "../lib/toast";
import { cn } from "./ui";

export function Toasts() {
  const { toasts, remove } = useToasts();
  if (toasts.length === 0) return null;
  return (
    <div className="pointer-events-none fixed bottom-5 right-5 z-[300] flex w-[min(20rem,calc(100vw-2.5rem))] flex-col gap-2">
      {toasts.map((t) => (
        <button
          key={t.id}
          onClick={() => remove(t.id)}
          className={cn(
            "pointer-events-auto rounded-xl px-4 py-3 text-left text-sm font-medium shadow-lg ring-1 backdrop-blur",
            t.tone === "ok" &&
              "bg-teal-50/95 text-teal-800 ring-teal-500/30 dark:bg-teal-950/90 dark:text-teal-200",
            t.tone === "fail" &&
              "bg-red-50/95 text-red-800 ring-red-500/30 dark:bg-red-950/90 dark:text-red-200",
            t.tone === "info" &&
              "bg-white/95 text-slate-700 ring-slate-200 dark:bg-slate-800/95 dark:text-slate-200 dark:ring-slate-700",
          )}
        >
          {t.message}
        </button>
      ))}
    </div>
  );
}
