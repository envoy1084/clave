import { Link } from "@tanstack/react-router";

import { motion, useReducedMotion } from "motion/react";

import { ButtonLink } from "@/components/ui/button";
import { Container } from "@/components/ui/section";
import { LockFigure } from "@/figures/lock/lock-figure";
import { LINKS } from "@/lib/content";
import { EASE_OUT } from "@/lib/motion";

export function Hero() {
  const reduce = useReducedMotion();
  const rise = (delay: number) => ({
    initial: { opacity: 0, y: reduce ? 0 : 14 },
    animate: { opacity: 1, y: 0 },
    transition: { duration: 0.7, ease: EASE_OUT, delay },
  });
  return (
    <section className="pt-10 pb-16 sm:pt-16 sm:pb-24">
      <Container className="grid items-center gap-10 lg:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)] lg:gap-12">
        <div>
          <motion.h1
            {...rise(0)}
            className="serif-display text-[2.8rem] leading-[1.02] sm:text-[4.2rem]"
          >
            A shared vault that <em className="serif-em">no one can open alone.</em>
          </motion.h1>
          <motion.p
            {...rise(0.08)}
            className="mt-6 max-w-[50ch] text-[1.05rem] leading-relaxed text-ink-soft sm:text-[1.12rem]"
          >
            Clave is a multisig account on Aztec. Moving funds takes{" "}
            <strong className="font-medium text-ink">k of n</strong> signatures. Reading the vault
            takes <strong className="font-medium text-ink">t of n</strong> members&rsquo; keepers,
            so no member, server or relay ever holds the key that decrypts it.
          </motion.p>
          <motion.div {...rise(0.16)} className="mt-8 flex flex-wrap gap-3">
            <ButtonLink href="#how">How it fits together</ButtonLink>
            <Link
              to="/developers"
              className="inline-flex h-11 items-center justify-center rounded-xl px-5 text-[0.92rem] font-medium shadow-[inset_0_0_0_1px_var(--color-line)] transition-[transform,background-color] duration-150 active:scale-[0.97] hover-fine:bg-paper-2"
            >
              Run it locally
            </Link>
          </motion.div>
          <motion.p
            {...rise(0.24)}
            className="mt-8 font-mono text-[0.75rem] leading-relaxed text-muted"
          >
            Aztec 6.0.0 · Noir account contract · Rust threshold crypto ·{" "}
            <a href={LINKS.repo} className="underline underline-offset-4 hover-fine:text-ink">
              v0, local network
            </a>
          </motion.p>
        </div>
        <motion.div {...rise(0.1)}>
          <LockFigure tour />
        </motion.div>
      </Container>
    </section>
  );
}
