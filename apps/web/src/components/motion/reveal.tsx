import type { ReactNode } from "react";

import { motion, useReducedMotion } from "motion/react";

import { EASE_OUT, inViewOnce } from "@/lib/motion";

/** A block that rises into place the first time it scrolls into view. Used for section openings, not every element. */
export function Reveal({
  children,
  delay = 0,
  className,
}: {
  children: ReactNode;
  delay?: number;
  className?: string;
}) {
  const reduce = useReducedMotion();
  return (
    <motion.div
      className={className}
      initial={{ opacity: 0, y: reduce ? 0 : 18 }}
      whileInView={{ opacity: 1, y: 0 }}
      viewport={inViewOnce}
      transition={{ duration: 0.7, ease: EASE_OUT, delay }}
    >
      {children}
    </motion.div>
  );
}
