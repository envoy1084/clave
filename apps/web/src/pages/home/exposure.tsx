import { useState } from "react";

import { motion } from "motion/react";

import { Redacted } from "@/components/motion/redacted";
import { Container } from "@/components/ui/section";
import { cn } from "@/lib/cn";
import { EASE_OUT } from "@/lib/motion";

type View = "network" | "member";

// One 3-of-5 vault as each side sees it. Values are illustrative; which rows are hidden is the design (privacy
// invariants in AGENTS.md; relay trust model in architecture/networking.md).
const ROWS = [
  {
    what: "Members",
    value: "5 signer keys",
    where: "private config and signer notes",
    hidden: true,
  },
  { what: "Spending threshold", value: "3 of 5", where: "private config note", hidden: true },
  {
    what: "Viewing committee",
    value: "t 3 of 5, epoch 2",
    where: "a commitment inside the config note",
    hidden: true,
  },
  {
    what: "Balance",
    value: "9,975 JADE",
    where: "encrypted notes, read through the keepers",
    hidden: true,
  },
  {
    what: "Pending proposal",
    value: "send 25 JADE",
    where: "sealed between keepers; relays hold ciphertext",
    hidden: true,
  },
  {
    what: "Approvals",
    value: "3 signatures",
    where: "inside the proof's private inputs",
    hidden: true,
  },
  {
    what: "Fee payment",
    value: "sponsored",
    where: "never paid from the vault in public",
    hidden: false,
  },
  {
    what: "Note hashes, nullifiers",
    value: "random-looking fields",
    where: "public, as for every Aztec transaction",
    hidden: false,
  },
] as const;

const VIEWS: readonly { id: View; label: string }[] = [
  { id: "network", label: "The network" },
  { id: "member", label: "A member" },
];

export function Exposure() {
  const [view, setView] = useState<View>("network");
  return (
    <section
      id="exposure"
      aria-labelledby="exposure-title"
      className="bg-navy py-16 text-paper sm:py-24"
    >
      <Container>
        <div className="grid gap-6 md:grid-cols-[minmax(0,1fr)_auto] md:items-end">
          <div className="max-w-[620px]">
            <h2
              id="exposure-title"
              className="serif-display text-[2.2rem] leading-[1.06] sm:text-[3rem]"
            >
              What the network sees,{" "}
              <em className="serif-em text-signal">and what it never does.</em>
            </h2>
            <p className="mt-4 text-[1.02rem] leading-relaxed text-paper/75">
              Flip the view. Members, thresholds, balances and proposals stay encrypted. The chain
              sees proofs and random-looking hashes.
            </p>
          </div>
          <div
            role="radiogroup"
            aria-label="Seen by"
            className="inline-flex rounded-xl bg-paper/10 p-1"
          >
            {VIEWS.map((v) => (
              <button
                key={v.id}
                type="button"
                role="radio"
                aria-checked={view === v.id}
                onClick={() => setView(v.id)}
                className="relative h-9 rounded-lg px-4 text-[0.86rem] font-medium"
              >
                {view === v.id && (
                  <motion.span
                    layoutId="exposure-view"
                    className="absolute inset-0 rounded-lg bg-paper"
                    transition={{ duration: 0.3, ease: EASE_OUT }}
                  />
                )}
                <span
                  className={cn(
                    "relative transition-colors duration-200",
                    view === v.id ? "text-ink" : "text-paper/70",
                  )}
                >
                  {v.label}
                </span>
              </button>
            ))}
          </div>
        </div>

        <ul className="mt-12 divide-y divide-paper/10 border-y border-paper/10">
          {ROWS.map((row, i) => (
            <li
              key={row.what}
              className="grid gap-1 py-4 sm:grid-cols-[13rem_14rem_1fr] sm:items-center sm:gap-6"
            >
              <span className="text-[0.92rem] font-medium">{row.what}</span>
              <span className="font-mono text-[0.82rem]">
                {row.hidden ? (
                  <Redacted text={row.value} revealed={view === "member"} seed={i + 2} />
                ) : (
                  row.value
                )}
              </span>
              <span className="text-[0.86rem] text-paper/70">{row.where}</span>
            </li>
          ))}
        </ul>
        <p className="mt-4 text-[0.8rem] text-paper/65">Example values.</p>
      </Container>
    </section>
  );
}
