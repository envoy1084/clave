// Motion tokens: one set of curves and timings for the whole app, so every transition feels like the same product.
import type { Transition, Variants } from "motion/react";

/** Strong ease-out: instant response, soft landing. Entrances and anything the user triggered. */
export const EASE_OUT = [0.23, 1, 0.32, 1] as const;
/** Strong ease-in-out: things moving across the screen. */
export const EASE_IN_OUT = [0.77, 0, 0.175, 1] as const;

export const quick: Transition = { duration: 0.18, ease: EASE_OUT };
export const settle: Transition = { duration: 0.32, ease: EASE_OUT };

/** A short rise with a touch of blur, so content swaps read as one motion. */
export const rise: Variants = {
  hidden: { opacity: 0, y: 10, filter: "blur(3px)" },
  shown: { opacity: 1, y: 0, filter: "blur(0px)", transition: { duration: 0.45, ease: EASE_OUT } },
};
