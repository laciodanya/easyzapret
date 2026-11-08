import { useEffect, useState } from "react";

/**
 * Keeps an element mounted for `exitMs` after `open` turns false so its exit
 * animation can play. `state` goes on a `data-state` attribute.
 */
export function usePresence(open: boolean, exitMs: number) {
  const [lingering, setLingering] = useState(open);

  useEffect(() => {
    if (open) {
      setLingering(true);
      return;
    }
    const timer = window.setTimeout(() => setLingering(false), exitMs);
    return () => window.clearTimeout(timer);
  }, [open, exitMs]);

  return { mounted: open || lingering, state: open ? "open" : "closed" } as const;
}
