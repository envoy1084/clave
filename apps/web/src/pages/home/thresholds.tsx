import type { ReactNode } from "react";

import { Link } from "@tanstack/react-router";

import { Reveal } from "@/components/motion/reveal";
import { Section } from "@/components/ui/section";

type To = "/spending" | "/viewing";

// Two parallel rules, so two cards of equal weight.
const CARDS: readonly { to: To; rule: ReactNode; title: string; body: string; cta: string }[] = [
  {
    to: "/spending",
    rule: (
      <>
        k <em className="serif-em">of</em> n
      </>
    ),
    title: "signatures to move funds",
    body: "A payment runs only when enough members sign it. The vault's own contract checks the signatures inside the proof, so nobody can skip the rule.",
    cta: "How spending works",
  },
  {
    to: "/viewing",
    rule: (
      <>
        t <em className="serif-em">of</em> n
      </>
    ),
    title: "keepers to read the vault",
    body: "On Aztec, funds are encrypted notes. Clave splits the key that reads them across the members, so any t can read together and fewer learn nothing.",
    cta: "How viewing works",
  },
];

export function Thresholds() {
  return (
    <Section
      id="thresholds"
      title={
        <>
          One threshold to spend. <em className="serif-em">One to read.</em>
        </>
      }
      lede="Spending needs k of n member signatures, checked by the vault's contract. Reading needs t of n keepers, because the vault's viewing key is split between them."
    >
      <div className="grid gap-5 md:grid-cols-2">
        {CARDS.map((card, i) => (
          <Reveal key={card.to} delay={i * 0.08}>
            <Link
              to={card.to}
              className="group flex h-full flex-col rounded-3xl bg-paper-2 p-6 transition-transform duration-200 active:scale-[0.99] sm:p-8"
            >
              <p className="serif-display text-[3.6rem] leading-none sm:text-[4.6rem]">
                {card.rule}
              </p>
              <p className="mt-2 text-[1.1rem] font-medium">{card.title}</p>
              <p className="mt-4 max-w-[46ch] text-[0.96rem] leading-relaxed text-ink-soft">
                {card.body}
              </p>
              <span className="mt-auto pt-8 text-[0.92rem] font-medium text-olive underline-offset-4 group-hover:underline">
                {card.cta}
              </span>
            </Link>
          </Reveal>
        ))}
      </div>
    </Section>
  );
}
