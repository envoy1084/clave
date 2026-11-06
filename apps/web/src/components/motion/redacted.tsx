import { useMemo } from "react";

import { motion, useReducedMotion } from "motion/react";

import { cn } from "@/lib/cn";
import { EASE_OUT } from "@/lib/motion";

const BLOCKS = ["#15180F", "#2b2f4a", "#A7A9FF", "#5BABFF", "#3b2f7a", "#C8F500"];

/**
 * Text as the network sees it (a pixel mosaic) or as a member sees it. The mosaic is the page's privacy motif: the same
 * pixels as the terrain art. Blocks dissolve in a scattered order when `revealed`, compositor-only (opacity, scale).
 */
export function Redacted({
  text,
  revealed,
  seed = 1,
  className,
}: {
  text: string;
  revealed: boolean;
  seed?: number;
  className?: string;
}) {
  const reduce = useReducedMotion();
  const blocks = useMemo(() => {
    let s = seed * 7919 + text.length;
    const random = () => (s = (s * 16807) % 2147483647) / 2147483647;
    const count = Math.max(4, Math.round(text.length * 0.9));
    return Array.from({ length: count }, (_, i) => ({
      key: i,
      color: BLOCKS[Math.floor(random() * BLOCKS.length)] ?? "#15180F",
      delay: random() * 0.35,
      opacity: 0.55 + random() * 0.45,
    }));
  }, [text, seed]);
  return (
    <span className={cn("relative inline-grid align-baseline whitespace-nowrap", className)}>
      <motion.span
        className="col-start-1 row-start-1"
        animate={{ opacity: revealed ? 1 : 0 }}
        transition={{ duration: 0.3, ease: EASE_OUT, delay: revealed ? 0.15 : 0 }}
        aria-hidden={!revealed}
      >
        {text}
      </motion.span>
      <span
        className="pointer-events-none col-start-1 row-start-1 flex overflow-hidden rounded-[2px]"
        aria-hidden={revealed}
        aria-label={revealed ? undefined : "hidden from the network"}
      >
        {blocks.map((b) => (
          <motion.span
            key={b.key}
            className="h-full flex-1"
            style={{ backgroundColor: b.color }}
            initial={false}
            animate={{ opacity: revealed ? 0 : b.opacity, scale: revealed && !reduce ? 0.4 : 1 }}
            transition={{
              duration: 0.28,
              ease: EASE_OUT,
              delay: revealed ? b.delay : b.delay * 0.5,
            }}
          />
        ))}
      </span>
    </span>
  );
}
