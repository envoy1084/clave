import type { ReactNode } from "react";

import { motion, useReducedMotion } from "motion/react";

import { EASE_OUT } from "@/lib/motion";

import { Container } from "./section";

/** The top of an inner page: a serif title (italics for the key phrase), a plain-language lede and one technical line. */
export function PageHeader({
  title,
  lede,
  meta,
}: {
  title: ReactNode;
  lede: ReactNode;
  meta?: ReactNode;
}) {
  const reduce = useReducedMotion();
  const rise = (delay: number) => ({
    initial: { opacity: 0, y: reduce ? 0 : 12 },
    animate: { opacity: 1, y: 0 },
    transition: { duration: 0.6, ease: EASE_OUT, delay },
  });
  return (
    <Container className="pt-14 pb-12 sm:pt-20 sm:pb-16">
      <motion.h1
        {...rise(0)}
        className="serif-display max-w-[16ch] text-[2.6rem] leading-[1.02] sm:text-[4rem]"
      >
        {title}
      </motion.h1>
      <motion.p
        {...rise(0.08)}
        className="mt-6 max-w-[60ch] text-[1.05rem] leading-relaxed text-ink-soft sm:text-[1.12rem]"
      >
        {lede}
      </motion.p>
      {meta && (
        <motion.p {...rise(0.14)} className="mt-6 font-mono text-[0.75rem] text-muted">
          {meta}
        </motion.p>
      )}
    </Container>
  );
}
