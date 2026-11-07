import { type ReactNode, useRef } from "react";

import { AnimatePresence, motion, useInView } from "motion/react";

import { Container } from "@/components/ui/section";
import { useSequence } from "@/lib/use-sequence";

/** Colors the drawings use: the brand's ink and paper, the lime accent for the moment that matters, lavender for shares. */
export const INK = "#15180f";
export const PAPER = "#fbf8f2";
export const SIGNAL = "#c8f500";
export const PERI = "#a7a9ff";
export const MUTED = "#5e6053";

/**
 * A flat, single-idea drawing that plays its sequence once when it scrolls into view, then rests on the final frame.
 * Replay runs it again; under reduced motion the final frame shows at once.
 */
export function Illustration({
  label,
  caption,
  delays,
  viewBox,
  children,
}: {
  label: string;
  caption: ReactNode;
  delays: readonly number[];
  viewBox: string;
  children: (step: number) => ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const inView = useInView(ref, { once: true, amount: 0.4 });
  const { step, done, replay } = useSequence(delays, inView);
  return (
    <Container>
      <figure ref={ref} className="rounded-[28px] bg-mist px-4 pt-8 pb-5 sm:px-8 sm:pt-10">
        <svg
          viewBox={viewBox}
          role="img"
          aria-label={label}
          className="mx-auto block w-full max-w-[760px]"
        >
          {children(step)}
        </svg>
        <div className="mt-4 flex min-h-11 items-center justify-between gap-4">
          <figcaption className="max-w-[60ch] text-[0.9rem] leading-relaxed text-ink-soft">
            {caption}
          </figcaption>
          <AnimatePresence>
            {done && (
              <motion.button
                type="button"
                onClick={replay}
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0 }}
                className="h-11 shrink-0 rounded-xl px-4 text-[0.88rem] font-medium shadow-[inset_0_0_0_1px_var(--color-line)] transition-[transform,background-color] duration-150 hover-fine:bg-paper active:scale-[0.97]"
              >
                Replay
              </motion.button>
            )}
          </AnimatePresence>
        </div>
      </figure>
    </Container>
  );
}

/** A small label under part of a drawing. */
export function Label({ x, y, children }: { x: number; y: number; children: ReactNode }) {
  return (
    <text
      x={x}
      y={y}
      textAnchor="middle"
      fontSize={13}
      fill={MUTED}
      fontFamily="Geist Variable, sans-serif"
    >
      {children}
    </text>
  );
}

/** A diagram inside a navy chapter: no panel of its own, plays once in view, replays on demand. */
export function DarkDiagram({
  label,
  delays,
  viewBox,
  className,
  children,
}: {
  label: string;
  delays: readonly number[];
  viewBox: string;
  className?: string;
  children: (step: number) => ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const inView = useInView(ref, { once: true, amount: 0.5 });
  const { step, done, replay } = useSequence(delays, inView);
  return (
    <div ref={ref} className={className}>
      <svg viewBox={viewBox} role="img" aria-label={label} className="block w-full">
        {children(step)}
      </svg>
      <div className="mt-2 flex h-11 justify-end">
        <AnimatePresence>
          {done && (
            <motion.button
              type="button"
              onClick={replay}
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="h-11 rounded-xl px-4 text-[0.88rem] font-medium text-paper shadow-[inset_0_0_0_1px_rgb(251_248_242/0.25)] transition-[transform,background-color] duration-150 hover-fine:bg-paper/10 active:scale-[0.97]"
            >
              Replay
            </motion.button>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}
