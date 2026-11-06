import type { AnchorHTMLAttributes } from "react";

import { cn } from "@/lib/cn";

type Variant = "primary" | "signal" | "ghost" | "outline";

const VARIANTS: Record<Variant, string> = {
  primary: "bg-ink text-paper hover-fine:bg-ink-2",
  signal: "bg-signal text-ink hover-fine:bg-[#bde800]",
  ghost: "text-ink hover-fine:bg-paper-2",
  outline: "text-ink shadow-[inset_0_0_0_1px_var(--color-line)] hover-fine:bg-paper-2",
};

/** A link styled as a button: every action on this page navigates somewhere real. */
export function ButtonLink({
  variant = "primary",
  className,
  ...props
}: AnchorHTMLAttributes<HTMLAnchorElement> & { variant?: Variant }) {
  return (
    <a
      {...props}
      className={cn(
        "inline-flex h-11 items-center justify-center gap-2 rounded-xl px-5 text-[0.92rem] font-medium transition-[transform,background-color] duration-150 ease-out active:scale-[0.97]",
        VARIANTS[variant],
        className,
      )}
    />
  );
}
