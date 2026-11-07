import { motion, useReducedMotion } from "motion/react";

import { cn } from "@/lib/cn";

// The Clave mark: an arch in two halves standing on a sill. The halves meet but never join; the doorway exists only
// when both are there. Drawn on a 40-unit grid; see architecture/brand.md.
const LEFT = "M7 31V20A13 13 0 0 1 18 7.15V13.82A6.5 6.5 0 0 0 13.5 20V31Z";
const RIGHT = "M33 31V20A13 13 0 0 0 22 7.15V13.82A6.5 6.5 0 0 1 26.5 20V31Z";

export function Mark({ className, title }: { className?: string; title?: string }) {
  return (
    <svg
      viewBox="0 0 40 40"
      className={cn("size-6", className)}
      role={title ? "img" : undefined}
      aria-hidden={!title}
    >
      {title && <title>{title}</title>}
      <path d={LEFT} fill="currentColor" />
      <path d={RIGHT} fill="currentColor" />
      <rect x="4" y="33.5" width="32" height="4" fill="currentColor" />
    </svg>
  );
}

/**
 * The mark assembling piece by piece: the sill first, then each half settling into place. Used once, when a vault is
 * founded; `pieces` is how many of the three are in place.
 */
export function AssemblingMark({
  pieces,
  className,
}: {
  pieces: 0 | 1 | 2 | 3;
  className?: string;
}) {
  const reduce = useReducedMotion();
  const spring = { type: "spring" as const, duration: 0.7, bounce: 0.18 };
  const piece = (visible: boolean, from: { x?: number; y?: number }) => ({
    initial: false as const,
    animate: visible
      ? { opacity: 1, x: 0, y: 0 }
      : {
          opacity: 0.12,
          x: reduce ? 0 : (from.x ?? 0),
          y: reduce ? 0 : (from.y ?? 0),
        },
    transition: spring,
  });
  return (
    <svg viewBox="0 0 40 40" className={cn("size-24 overflow-visible", className)} aria-hidden>
      <motion.rect
        x="4"
        y="33.5"
        width="32"
        height="4"
        fill="currentColor"
        {...piece(pieces >= 1, { y: 4 })}
      />
      <motion.path d={LEFT} fill="currentColor" {...piece(pieces >= 2, { x: -5, y: -2 })} />
      <motion.path d={RIGHT} fill="currentColor" {...piece(pieces >= 3, { x: 5, y: -2 })} />
    </svg>
  );
}

export function Wordmark({ className }: { className?: string }) {
  return (
    <span
      className={cn("inline-flex items-center gap-2 font-medium tracking-[-0.02em]", className)}
    >
      <Mark className="size-[1.15em]" />
      Clave
    </span>
  );
}
