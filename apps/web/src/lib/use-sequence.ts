import { useCallback, useEffect, useState } from "react";

import { useReducedMotion } from "motion/react";

/**
 * Steps a figure through `delays.length` stages once `active` is true, one stage per delay (milliseconds after the
 * previous). Plays once; `replay` starts it again. Under reduced motion the last stage shows at once.
 */
export function useSequence(delays: readonly number[], active: boolean) {
  const reduce = useReducedMotion();
  const last = delays.length;
  const [step, setStep] = useState(0);
  const [run, setRun] = useState(0);

  useEffect(() => {
    if (!active) return;
    if (reduce) {
      setStep(last);
      return;
    }
    setStep(0);
    let at = 0;
    const timers = delays.map((delay, i) => {
      at += delay;
      return window.setTimeout(() => setStep(i + 1), at);
    });
    return () => timers.forEach((t) => window.clearTimeout(t));
    // `delays` is a module constant at every call site.
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [active, reduce, run, last]);

  const replay = useCallback(() => setRun((r) => r + 1), []);
  return { step, done: step === last, replay };
}
