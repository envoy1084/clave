// Motion tokens: one set of curves and timings for the whole app. Only transform and opacity animate (compositor-only);
// filters and layout properties never do.
import type { Transition, Variants } from "motion/react";

/** Strong ease-out: instant response, soft landing. Entrances and anything the user triggered. */
export const EASE_OUT = [0.23, 1, 0.32, 1] as const;
/** Strong ease-in-out: things moving across the screen. */
export const EASE_IN_OUT = [0.77, 0, 0.175, 1] as const;

export const quick: Transition = { duration: 0.18, ease: EASE_OUT };
export const settle: Transition = { duration: 0.28, ease: EASE_OUT };

/** Staggered entrance for a screen's first paint. */
export const rise: Variants = {
  hidden: { opacity: 0, y: 8 },
  shown: { opacity: 1, y: 0, transition: { duration: 0.32, ease: EASE_OUT } },
};

/**
 * Step-to-step transitions (onboarding, founding): the new step slides in from the side it comes from, the old one
 * leaves faster than the new one arrives. `custom` is the direction: 1 forward, -1 back.
 */
export const step: Variants = {
  enter: (direction: number) => ({ opacity: 0, x: 28 * direction }),
  center: { opacity: 1, x: 0, transition: { duration: 0.3, ease: EASE_OUT } },
  exit: (direction: number) => ({
    opacity: 0,
    x: -20 * direction,
    transition: { duration: 0.16, ease: EASE_OUT },
  }),
};
