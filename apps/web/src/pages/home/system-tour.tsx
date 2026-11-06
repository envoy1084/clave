import { useEffect, useRef, useState } from "react";

import { AnimatePresence, motion, useInView } from "motion/react";

import { FigureCard } from "@/components/iso/figure-card";
import { Container } from "@/components/ui/section";
import { HEIGHT, TRAYS, WIDTH, liftOffset } from "@/figures/rack/geometry";
import { RackFigure } from "@/figures/rack/rack-figure";
import { cn } from "@/lib/cn";
import { EASE_OUT, quick } from "@/lib/motion";

// Read top to bottom: the rack's top tray first. `tray` indexes TRAYS (0 is the bottom tray).
const STOPS = [
  {
    tray: 3,
    place: "Your device",
    holds: "Your signing key",
    body: "Your wallet builds and proves every transaction on your own machine. Before you sign, it rebuilds the proposal from its calls and shows you what you are approving.",
    detail:
      "vault-wallet: PXE host, vault account class, witness encoder, patched key store. tecpan CLI in development.",
  },
  {
    tray: 2,
    place: "Your keeper",
    holds: "One share of the viewing key",
    body: "A small daemon you run on your laptop or your own server. It keeps your share of the viewing key and answers decryption requests from other members' wallets, each answer with a proof.",
    detail:
      "tecpan-keeper: DKG and resharing, ECDH responder with DLEQ proofs, proposal replication over libp2p with Noise.",
  },
  {
    tray: 1,
    place: "A relay, optional",
    holds: "Sealed envelopes only",
    body: "Passes messages between keepers that cannot reach each other and holds them for members who are offline. Anyone can run one; it cannot read, forge or link what it stores.",
    detail:
      "tecpan-relay: circuit relay and HPKE-sealed mailboxes. It can drop or delay traffic, nothing more.",
  },
  {
    tray: 0,
    place: "Aztec network",
    holds: "Proofs and commitments",
    body: "Checks the proof and records the result. Who the members are, the thresholds and the amounts stay inside encrypted notes that only the vault can read.",
    detail:
      "TecpanVault account contract; config in private notes; fees through a sponsored fee payment contract.",
  },
] as const;

function Stop({
  index,
  onActive,
  children,
}: {
  index: number;
  onActive: (i: number) => void;
  children: React.ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const inView = useInView(ref, { amount: 0.6 });
  useEffect(() => {
    if (inView) onActive(index);
  }, [inView, index, onActive]);
  return (
    <div ref={ref} className="flex min-h-[46vh] items-center lg:min-h-[78vh]">
      {children}
    </div>
  );
}

/**
 * The system, scroll-driven: a sticky rack whose trays lift one at a time while the text beside it walks from the
 * member's device down to the chain. A callout pinned to the lifted tray names what it holds.
 */
export function SystemTour() {
  const [active, setActive] = useState(0);
  const stop = STOPS[active] ?? STOPS[0];
  const anchor = TRAYS[stop.tray]?.anchor ?? [0, 0];
  const left = ((anchor[0] + liftOffset[0]) / WIDTH) * 100;
  const top = ((anchor[1] + liftOffset[1]) / HEIGHT) * 100;

  return (
    <section id="how" aria-labelledby="how-title" className="py-16 sm:py-24">
      <Container>
        <div className="max-w-[640px]">
          <h2 id="how-title" className="serif-display text-[2.2rem] leading-[1.06] sm:text-[3rem]">
            Four places, <em className="serif-em">none of them in charge.</em>
          </h2>
          <p className="mt-4 text-[1.02rem] leading-relaxed text-ink-soft">
            There is no Tecpan server. Each member runs their own wallet and keeper; relays are
            optional and untrusted; the chain only sees proofs.
          </p>
        </div>

        <div className="mt-10 grid gap-8 lg:grid-cols-[minmax(0,0.85fr)_minmax(0,1.15fr)] lg:gap-14">
          <div className="sticky top-16 z-10 -mx-5 bg-paper/95 px-5 pb-2 lg:order-2 lg:top-20 lg:mx-0 lg:self-start lg:bg-transparent lg:px-0">
            <FigureCard
              fig="Fig. 3"
              title="The system"
              readout={stop.holds}
              className="mx-auto max-w-[520px] lg:max-w-none"
            >
              <div
                className="relative mx-auto h-[34vh] max-w-full lg:h-[calc(100vh-13rem)]"
                style={{ aspectRatio: `${WIDTH} / ${HEIGHT}` }}
              >
                <RackFigure active={stop.tray} />
                <AnimatePresence initial={false}>
                  <motion.div
                    key={stop.tray}
                    className="pointer-events-none absolute flex items-center gap-2"
                    style={{ left: `${left}%`, top: `${top}%` }}
                    initial={{ opacity: 0, x: -6 }}
                    animate={{ opacity: 1, x: 0 }}
                    exit={{ opacity: 0 }}
                    transition={{ duration: 0.35, ease: EASE_OUT, delay: 0.15 }}
                  >
                    <span className="-ml-1 size-2 shrink-0 rounded-full bg-ink ring-4 ring-signal/70" />
                    <span className="h-px w-6 bg-ink/50" />
                    <span className="rounded-md bg-ink px-2 py-1 font-mono text-[0.66rem] whitespace-nowrap text-paper uppercase">
                      {String(active + 1).padStart(2, "0")} {stop.place}
                    </span>
                  </motion.div>
                </AnimatePresence>
              </div>
            </FigureCard>
          </div>

          <div className="lg:order-1">
            {STOPS.map((s, i) => (
              <Stop key={s.place} index={i} onActive={setActive}>
                <motion.div
                  animate={{ opacity: active === i ? 1 : 0.38 }}
                  transition={quick}
                  className={cn(
                    "border-l-2 py-2 pl-5 transition-colors duration-300",
                    active === i ? "border-ink" : "border-line",
                  )}
                >
                  <p className="font-mono text-[0.72rem] text-muted">
                    {String(i + 1).padStart(2, "0")} · holds {s.holds.toLowerCase()}
                  </p>
                  <h3 className="serif-display mt-2 text-[1.9rem] leading-tight sm:text-[2.2rem]">
                    {s.place}
                  </h3>
                  <p className="mt-3 max-w-[48ch] text-[0.98rem] leading-relaxed text-ink-soft">
                    {s.body}
                  </p>
                  <p className="mt-4 max-w-[52ch] font-mono text-[0.74rem] leading-relaxed text-muted">
                    {s.detail}
                  </p>
                </motion.div>
              </Stop>
            ))}
          </div>
        </div>
      </Container>
    </section>
  );
}
