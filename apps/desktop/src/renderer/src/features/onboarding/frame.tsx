import type { ReactNode } from "react";

import { motion } from "motion/react";

import { Wordmark } from "@/components/brand/mark";
import { Terrain } from "@/components/brand/terrain";
import { EASE_OUT } from "@/lib/motion";
import { cn } from "@/lib/utils";

/**
 * First-run layout: the landscape on the left (with a line that changes per step), the task on the right. The top
 * strip drags the window on macOS.
 */
export function OnboardingFrame({
  children,
  caption,
  step,
  steps,
  seed,
}: {
  children: ReactNode;
  caption: ReactNode;
  step?: number;
  steps?: number;
  seed?: number;
}) {
  return (
    <div className="grid h-screen grid-cols-[minmax(360px,42%)_1fr] overflow-hidden bg-paper">
      <aside className="relative overflow-hidden bg-[#0E1530] text-white">
        <Terrain
          seed={seed}
          className="absolute inset-0"
          clusters={[
            [330, 70, 9, 6],
            [400, 250, 6, 5],
            [240, 370, 5, 4],
          ]}
        />
        <div className="app-drag absolute inset-x-0 top-0 h-14" />
        <div className="absolute top-14 left-9">
          <Wordmark className="text-lg text-white" />
        </div>
        <motion.div
          key={String(seed)}
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.5, ease: EASE_OUT }}
          className="absolute right-9 bottom-10 left-9 max-w-[30ch] font-serif text-[1.65rem] leading-[1.18] font-light tracking-[-0.015em] text-white/95"
        >
          {caption}
        </motion.div>
      </aside>
      <main className="relative flex min-w-0 flex-col">
        <div className="app-drag h-14 shrink-0" />
        {steps !== undefined && step !== undefined && (
          <div
            className="absolute top-6 right-8 flex items-center gap-1.5"
            aria-label={`Step ${step} of ${steps}`}
          >
            {Array.from({ length: steps }, (_, i) => (
              <span
                key={i}
                className={cn(
                  "h-1 rounded-full transition-[width,background-color] duration-300 ease-(--ease-out)",
                  i < step ? "w-6 bg-ink" : "w-3 bg-[#dcd7cb]",
                )}
              />
            ))}
          </div>
        )}
        <div className="quiet-scroll flex flex-1 items-center overflow-y-auto px-12 pb-14">
          <div className="mx-auto w-full max-w-[440px]">{children}</div>
        </div>
      </main>
    </div>
  );
}

/** Content entering a step: a short rise with a touch of blur, so swaps read as one motion. */
export const enter = {
  initial: { opacity: 0, y: 10, filter: "blur(3px)" },
  animate: { opacity: 1, y: 0, filter: "blur(0px)" },
  exit: { opacity: 0, y: -6, filter: "blur(2px)", transition: { duration: 0.14 } },
  transition: { duration: 0.32, ease: EASE_OUT },
} as const;
