import type { ReactNode } from "react";

import { Reveal } from "@/components/motion/reveal";
import { cn } from "@/lib/cn";

export interface Step {
  title: string;
  body: ReactNode;
  detail?: ReactNode;
}

/** A real process, in as many steps as it has. Each step: what happens in plain words, then the precise mechanism. */
export function Steps({ steps, className }: { steps: readonly Step[]; className?: string }) {
  return (
    <ol
      className={cn(
        "grid gap-px overflow-hidden rounded-3xl bg-line sm:grid-cols-2",
        steps.length % 4 === 0 && "lg:grid-cols-4",
        className,
      )}
    >
      {steps.map((step, i) => (
        <li key={step.title} className="bg-paper">
          <Reveal delay={i * 0.06} className="flex h-full flex-col p-5 sm:p-6">
            <span className="font-mono text-[0.72rem] text-muted">
              {String(i + 1).padStart(2, "0")}
            </span>
            <h3 className="mt-3 text-[1.02rem] font-medium">{step.title}</h3>
            <p className="mt-2 text-[0.9rem] leading-relaxed text-ink-soft">{step.body}</p>
            {step.detail && (
              <p className="mt-auto pt-4 font-mono text-[0.74rem] leading-relaxed text-muted">
                {step.detail}
              </p>
            )}
          </Reveal>
        </li>
      ))}
    </ol>
  );
}
