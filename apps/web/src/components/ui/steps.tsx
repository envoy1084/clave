import type { ReactNode } from "react";

import { Reveal } from "@/components/motion/reveal";
import { cn } from "@/lib/cn";

export interface Step {
  title: string;
  body: ReactNode;
}

/** A real process in as many steps as it has: a rule above each, the number in the serif, then plain words. */
export function Steps({ steps, className }: { steps: readonly Step[]; className?: string }) {
  return (
    <ol
      className={cn(
        "grid gap-x-8 gap-y-10 sm:grid-cols-2",
        steps.length % 4 === 0 && "lg:grid-cols-4",
        className,
      )}
    >
      {steps.map((step, i) => (
        <li key={step.title} className="border-t border-ink/15 pt-5">
          <Reveal delay={i * 0.06}>
            <span className="serif-display text-[1.6rem] leading-none text-ink/40">{i + 1}</span>
            <h3 className="mt-4 text-[1.05rem] font-medium">{step.title}</h3>
            <p className="mt-2 text-[0.93rem] leading-relaxed text-ink-soft">{step.body}</p>
          </Reveal>
        </li>
      ))}
    </ol>
  );
}
