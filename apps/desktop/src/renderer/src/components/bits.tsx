import type { ReactNode } from "react";

import { InfoIcon, MinusIcon, PlusIcon } from "lucide-react";
import { motion } from "motion/react";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { initials, memberColor } from "@/lib/format";
import { cn } from "@/lib/utils";

export type Tone = "ok" | "busy" | "warn" | "off" | "bad";

const TONES: Record<Tone, string> = {
  ok: "bg-[#6f9a00]",
  busy: "bg-peri",
  warn: "bg-[#d29a1a]",
  off: "bg-[#b9b6aa]",
  bad: "bg-coral",
};

/** A small status light; `pulse` adds a slow halo for things in progress. */
export function StatusDot({
  tone,
  pulse = false,
  className,
}: {
  tone: Tone;
  pulse?: boolean;
  className?: string;
}) {
  return (
    <span className={cn("relative inline-flex size-2 shrink-0", className)} aria-hidden>
      {pulse && (
        <span
          className={cn(
            "absolute inset-0 rounded-full opacity-60 motion-safe:animate-ping",
            TONES[tone],
          )}
        />
      )}
      <span className={cn("relative size-2 rounded-full", TONES[tone])} />
    </span>
  );
}

export function Avatar({
  name,
  identity,
  slot,
  size = "md",
  className,
}: {
  name: string;
  identity: string;
  /** Member slot (1-based) inside a vault or lobby: gives every member a distinct colour. */
  slot?: number;
  size?: "sm" | "md" | "lg";
  className?: string;
}) {
  return (
    <span
      className={cn(
        "inline-grid shrink-0 place-items-center rounded-full font-medium text-white shadow-[inset_0_0_0_1px_rgb(255_255_255/0.18)]",
        size === "sm" && "size-6 text-[0.65rem]",
        size === "md" && "size-8 text-xs",
        size === "lg" && "size-10 text-sm",
        className,
      )}
      style={{ backgroundColor: memberColor(identity, slot) }}
      aria-hidden
    >
      {initials(name)}
    </span>
  );
}

/** A term with a short plain-language explanation on hover, for the technical screens. */
export function Explain({ children, tip }: { children: ReactNode; tip: ReactNode }) {
  return (
    <span className="inline-flex items-center gap-1">
      {children}
      <Tooltip>
        <TooltipTrigger
          render={
            <button
              type="button"
              className="rounded-full text-muted-foreground/70 hover-fine:text-foreground"
            />
          }
          aria-label="What is this?"
        >
          <InfoIcon className="size-3.5" />
        </TooltipTrigger>
        <TooltipContent side="top" className="max-w-72">
          {tip}
        </TooltipContent>
      </Tooltip>
    </span>
  );
}

/** − n + control for small counts (members, thresholds). */
export function Stepper({
  value,
  min,
  max,
  onChange,
  label,
}: {
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
  label: string;
}) {
  const button =
    "grid size-9 place-items-center rounded-lg text-ink transition-[transform,background-color,opacity] duration-150 ease-out hover-fine:bg-accent active:scale-[0.94] disabled:opacity-30 disabled:pointer-events-none";
  return (
    <div
      className="inline-flex items-center gap-1 rounded-xl bg-card p-1 shadow-[0_0_0_1px_var(--border),0_1px_2px_rgb(21_24_15/0.04)]"
      role="group"
      aria-label={label}
    >
      <button
        type="button"
        className={button}
        onClick={() => onChange(value - 1)}
        disabled={value <= min}
        aria-label={`Fewer ${label}`}
      >
        <MinusIcon className="size-4" />
      </button>
      <span className="w-8 text-center font-serif text-xl tabular" aria-live="polite">
        {value}
      </span>
      <button
        type="button"
        className={button}
        onClick={() => onChange(value + 1)}
        disabled={value >= max}
        aria-label={`More ${label}`}
      >
        <PlusIcon className="size-4" />
      </button>
    </div>
  );
}

/** n seats, the first k filled: how a threshold looks. */
export function Seats({
  k,
  n,
  tone,
  className,
}: {
  k: number;
  n: number;
  tone: "olive" | "peri";
  className?: string;
}) {
  return (
    <div className={cn("flex items-center gap-1.5", className)} aria-label={`${k} of ${n}`}>
      {Array.from({ length: n }, (_, i) => (
        <motion.span
          key={i}
          layout
          initial={{ opacity: 0, scale: 0.8 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ type: "spring", duration: 0.35, bounce: 0.2 }}
          className={cn(
            "h-2 w-6 rounded-full transition-colors duration-200",
            i < k ? (tone === "olive" ? "bg-olive" : "bg-peri") : "bg-[#e3dfd3]",
          )}
        />
      ))}
    </div>
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return (
    <kbd className="rounded-md border border-border bg-card px-1.5 py-0.5 font-mono text-[0.7rem] text-muted-foreground shadow-[0_1px_0_var(--border)]">
      {children}
    </kbd>
  );
}
