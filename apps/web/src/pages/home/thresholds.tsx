import type { ReactNode } from "react";

import { Link } from "@tanstack/react-router";

import { Reveal } from "@/components/motion/reveal";
import { Section } from "@/components/ui/section";

type To = "/spending" | "/viewing" | "/membership";

const CARDS: readonly {
  to: To;
  rule: ReactNode;
  title: string;
  body: string;
  detail: string;
  cta: string;
}[] = [
  {
    to: "/spending",
    rule: (
      <>
        k <em className="serif-em">of</em> n
      </>
    ),
    title: "to move funds",
    body: "A payment leaves the vault only when enough members sign it. The check runs inside the vault's own account contract, as part of a zero-knowledge proof, so no one can skip it.",
    detail: "entrypoint verifies k Schnorr signatures over Poseidon2(approval.v0, …)",
    cta: "How spending works",
  },
  {
    to: "/viewing",
    rule: (
      <>
        t <em className="serif-em">of</em> n
      </>
    ),
    title: "to see what it holds",
    body: "On Aztec, funds are encrypted notes. Tecpan splits the key that reads them across the members' keepers. Any t of them can help a wallet read a note; fewer learn nothing.",
    detail: "threshold ECDH with DLEQ proofs; the incoming viewing secret is never assembled",
    cta: "How viewing works",
  },
] as const;

export function Thresholds() {
  return (
    <Section
      id="thresholds"
      title={
        <>
          Two thresholds, <em className="serif-em">two different jobs.</em>
        </>
      }
      lede="A normal multisig protects spending and leaves everything else public. On a private chain the harder problem is who can read the vault, so Tecpan puts a threshold on that too."
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
              <p className="mt-4 text-[0.95rem] leading-relaxed text-ink-soft">{card.body}</p>
              <p className="mt-6 font-mono text-[0.74rem] leading-relaxed text-muted">
                {card.detail}
              </p>
              <span className="mt-8 text-[0.9rem] font-medium text-olive underline-offset-4 group-hover:underline">
                {card.cta}
              </span>
            </Link>
          </Reveal>
        ))}
      </div>
      <Reveal delay={0.1}>
        <Link
          to="/membership"
          className="group mt-5 flex flex-col gap-2 rounded-3xl p-6 shadow-[inset_0_0_0_1px_var(--color-line)] transition-colors duration-150 hover-fine:bg-paper-2 sm:flex-row sm:items-center sm:justify-between sm:p-8"
        >
          <span>
            <span className="block text-[1.1rem] font-medium">
              Members change. The address does not.
            </span>
            <span className="mt-1 block text-[0.92rem] text-ink-soft">
              Removing someone reshares the viewing key, so they cannot read anything that arrives
              afterwards.
            </span>
          </span>
          <span className="shrink-0 text-[0.9rem] font-medium text-olive underline-offset-4 group-hover:underline">
            How membership works
          </span>
        </Link>
      </Reveal>
    </Section>
  );
}
