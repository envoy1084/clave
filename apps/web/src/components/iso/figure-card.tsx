import type { ReactNode } from "react";

import { cn } from "@/lib/cn";

import "./iso.css";

/**
 * The card every figure sits in: figure number top left, title top right, the interaction hint bottom left and the live
 * readout bottom right. Words stay in the corners and the caption, never in the drawing.
 */
export function FigureCard({
  fig,
  title,
  hint,
  readout,
  caption,
  children,
  className,
  tone = "paper",
}: {
  fig: string;
  title: string;
  hint?: ReactNode;
  readout?: ReactNode;
  caption?: ReactNode;
  children: ReactNode;
  className?: string;
  tone?: "paper" | "plain";
}) {
  return (
    <figure className={cn("iso", className)} data-theme="light">
      <div
        className={cn(
          "relative rounded-[28px] px-3 pt-12 pb-14 sm:px-6",
          tone === "paper" && "bg-[#f6f2e9] shadow-[0_0_0_1px_var(--color-line)]",
        )}
      >
        <div className="pointer-events-none absolute inset-x-5 top-5 flex justify-between gap-4 font-mono text-[0.68rem] tracking-[0.06em] text-muted uppercase sm:inset-x-6">
          <span className="text-ink-soft normal-case">{fig}</span>
          <span className="truncate text-right">{title}</span>
        </div>
        {children}
        <div className="absolute inset-x-5 bottom-5 flex items-end justify-between gap-4 font-mono text-[0.68rem] tracking-[0.04em] sm:inset-x-6">
          <span className="hidden text-muted uppercase sm:inline">{hint}</span>
          <span className="tabular text-right text-ink" aria-live="polite">
            {readout}
          </span>
        </div>
      </div>
      {caption && (
        <figcaption className="mt-3 max-w-[70ch] text-[0.82rem] leading-relaxed text-muted">
          {caption}
        </figcaption>
      )}
    </figure>
  );
}
