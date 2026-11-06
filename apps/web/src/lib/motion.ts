// Motion tokens shared by every section. Only transform, opacity and clip-path animate (compositor-only).
import type { Transition } from "motion/react";

export const EASE_OUT = [0.23, 1, 0.32, 1] as const;
export const EASE_IN_OUT = [0.77, 0, 0.175, 1] as const;

export const enter: Transition = { duration: 0.6, ease: EASE_OUT };
export const quick: Transition = { duration: 0.22, ease: EASE_OUT };

/** Scroll-triggered sequences start once, when the element is a third into the viewport. */
export const inViewOnce = { once: true, amount: 0.35 } as const;
