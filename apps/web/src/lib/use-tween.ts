import { useEffect, useRef, useState } from "react";

import { useReducedMotion } from "motion/react";

/**
 * A number that follows `target` with exponential easing (time constant `seconds`), for parts whose geometry must be
 * rebuilt while they move. Re-renders only the component that uses it, and jumps under reduced motion.
 */
export function useTween(target: number, seconds = 0.18) {
  const reduce = useReducedMotion();
  const [value, setValue] = useState(target);
  const current = useRef(target);

  useEffect(() => {
    if (reduce) {
      current.current = target;
      setValue(target);
      return;
    }
    let frame = 0;
    let last = performance.now();
    const tick = (now: number) => {
      const dt = Math.min(0.05, (now - last) / 1000);
      last = now;
      const next = target + (current.current - target) * Math.exp(-dt / seconds);
      current.current = Math.abs(next - target) < 0.01 ? target : next;
      setValue(current.current);
      if (current.current !== target) frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [target, seconds, reduce]);

  return value;
}
