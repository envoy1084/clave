import type { ReactNode } from "react";

import { Wordmark } from "@/components/brand/mark";
import { Terrain } from "@/components/brand/terrain";
import { cn } from "@/lib/utils";

/**
 * First-run layout. Mounted once for every onboarding step, so the art panel never re-renders between steps; only the
 * right column changes. The top strip drags the window on macOS.
 */
export function OnboardingFrame({
  children,
  step,
  steps,
}: {
  children: ReactNode;
  step: number;
  steps: number;
}) {
  return (
    <div className="grid h-screen grid-cols-[minmax(340px,40%)_1fr] overflow-hidden bg-paper">
      <aside className="relative overflow-hidden bg-[#0E1530] text-white">
        <Terrain
          seed={33}
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
      </aside>
      <main className="relative flex min-w-0 flex-col">
        <div className="app-drag h-14 shrink-0" />
        <ol
          className="absolute top-6 right-8 flex items-center gap-1.5"
          aria-label={`Step ${step} of ${steps}`}
        >
          {Array.from({ length: steps }, (_, i) => (
            <li
              key={i}
              className={cn(
                "h-1 rounded-full transition-[width,background-color] duration-300 ease-(--ease-out)",
                i < step ? "w-6 bg-ink" : "w-3 bg-[#dcd7cb]",
              )}
            />
          ))}
        </ol>
        <div className="quiet-scroll relative flex flex-1 items-center overflow-x-hidden overflow-y-auto px-12 pb-14">
          <div className="relative mx-auto w-full max-w-[460px]">{children}</div>
        </div>
      </main>
    </div>
  );
}
