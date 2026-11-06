import type { ReactNode } from "react";

import { cn } from "@/lib/cn";

/** Inline code: identifiers, function names, formulas. */
export function Code({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <code
      className={cn(
        "rounded-md bg-paper-2 px-1.5 py-0.5 font-mono text-[0.82em] text-ink",
        className,
      )}
    >
      {children}
    </code>
  );
}

/** A block of commands or a formula, selectable. */
export function CodeBlock({ children, className }: { children: string; className?: string }) {
  return (
    <pre
      className={cn(
        "overflow-x-auto rounded-2xl bg-ink p-5 font-mono text-[0.8rem] leading-relaxed text-paper/90 sm:text-[0.85rem]",
        className,
      )}
    >
      <code>{children}</code>
    </pre>
  );
}
