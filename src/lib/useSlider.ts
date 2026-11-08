import { useLayoutEffect, useRef, useState, type RefObject } from "react";

export interface SliderBox {
  left: number;
  width: number;
}

/**
 * Measures the active child of a container so an indicator can slide between
 * options. `animate` stays false for the first measurement so the indicator
 * doesn't fly in from the corner on mount.
 */
export function useSlider(
  containerRef: RefObject<HTMLElement | null>,
  selector: string,
  activeIndex: number,
  deps: unknown[],
) {
  const [box, setBox] = useState<SliderBox | null>(null);
  const animate = useRef(false);

  useLayoutEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const measure = () => {
      const target = container.querySelectorAll<HTMLElement>(selector)[activeIndex];
      setBox(target ? { left: target.offsetLeft, width: target.offsetWidth } : null);
    };

    measure();
    const frame = requestAnimationFrame(() => {
      animate.current = true;
    });
    const observer = new ResizeObserver(measure);
    observer.observe(container);
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeIndex, selector, ...deps]);

  return { box, animate: animate.current };
}
