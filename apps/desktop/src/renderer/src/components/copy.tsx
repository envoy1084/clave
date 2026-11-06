import { useEffect, useRef, useState } from "react";

import { CheckIcon, CopyIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { short } from "@/lib/format";
import { call } from "@/lib/ipc";
import { cn } from "@/lib/utils";

export function useCopy(timeout = 1400) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(timer.current), []);
  return {
    copied,
    copy: async (text: string) => {
      await call("clipboard.write", { text });
      setCopied(true);
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => setCopied(false), timeout);
    },
  };
}

/** Copy icon that morphs into a check: the change of state is the feedback. */
export function CopyIconSwap({ copied, className }: { copied: boolean; className?: string }) {
  return (
    <span className={cn("relative inline-grid size-3.5 place-items-center", className)} aria-hidden>
      <AnimatePresence initial={false} mode="popLayout">
        <motion.span
          key={copied ? "check" : "copy"}
          initial={{ opacity: 0, scale: 0.6 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.6 }}
          transition={{ type: "spring", duration: 0.3, bounce: 0 }}
          className="col-start-1 row-start-1 grid place-items-center"
        >
          {copied ? <CheckIcon className="size-3.5" /> : <CopyIcon className="size-3.5" />}
        </motion.span>
      </AnimatePresence>
    </span>
  );
}

/** A monospace identifier, shortened, copyable in one click; the full value is in the tooltip and the clipboard. */
export function CopyValue({
  value,
  head = 8,
  tail = 6,
  full = false,
  className,
  label,
}: {
  value: string;
  head?: number;
  tail?: number;
  full?: boolean;
  className?: string;
  label?: string;
}) {
  const { copied, copy } = useCopy();
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <button
            type="button"
            onClick={() => void copy(value)}
            aria-label={`Copy ${label ?? "value"}`}
            className={cn(
              "group/copy inline-flex max-w-full min-w-0 items-center gap-1.5 rounded-md px-1.5 py-0.5 -mx-1.5 font-mono text-[0.8rem] text-foreground/85 transition-colors duration-150 hover-fine:bg-accent hover-fine:text-foreground",
              className,
            )}
          />
        }
      >
        <span className={cn("min-w-0", full ? "break-all text-left" : "truncate")}>
          {full ? value : short(value, head, tail)}
        </span>
        <CopyIconSwap
          copied={copied}
          className={cn(
            "shrink-0 text-muted-foreground transition-opacity duration-150",
            !copied &&
              "opacity-0 group-hover/copy:opacity-100 group-focus-visible/copy:opacity-100",
          )}
        />
      </TooltipTrigger>
      <TooltipContent className="max-w-sm font-mono break-all">
        {copied ? "Copied" : value}
      </TooltipContent>
    </Tooltip>
  );
}
