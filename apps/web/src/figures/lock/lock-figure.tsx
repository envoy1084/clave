import { useEffect, useRef, useState, type KeyboardEvent } from "react";

import { AnimatePresence, motion, useInView, useReducedMotion } from "motion/react";

import { Base } from "@/components/iso/base";
import { Dots, FaceText, Line, Solid } from "@/components/iso/draw";
import { FigureCard } from "@/components/iso/figure-card";
import { depthOf } from "@/components/iso/kit";
import { EASE_OUT } from "@/lib/motion";
import { useTween } from "@/lib/use-tween";

import {
  BASE_ART,
  BOLT_REACH,
  BOX_ART,
  HEIGHT,
  P,
  RULER_ART,
  SIGNERS,
  SLOT_ART,
  STRIKE_ART,
  THRESHOLD,
  WIDTH,
  at,
  boltOf,
  riderStep,
  slotCenter,
} from "./geometry";

const BY_DEPTH = SLOT_ART.toSorted(
  (a, b) => depthOf(slotCenter(a.slot), P) - depthOf(slotCenter(b.slot), P),
);
const TOUR = [0, 2, 3] as const;

function Bolt({ open }: { open: boolean }) {
  const out = useTween(open ? 0 : BOLT_REACH, 0.16);
  const art = boltOf(out);
  return (
    <g>
      <Solid paths={art.guide} tone="mid" />
      <Solid paths={art.bolt} tone="hi" className={open ? undefined : "accent-ink"} />
    </g>
  );
}

function Rider({ count, authorized }: { count: number; authorized: boolean }) {
  const reduce = useReducedMotion();
  return (
    <motion.g
      initial={false}
      animate={{ x: riderStep[0] * count, y: riderStep[1] * count }}
      transition={reduce ? { duration: 0 } : { type: "spring", duration: 0.5, bounce: 0.2 }}
    >
      <Solid
        paths={RULER_ART.rider}
        tone="hi"
        className={authorized ? "accent-signal" : undefined}
      />
      <Line d={RULER_ART.riderMark} tone="hi" />
    </motion.g>
  );
}

/**
 * The lock box, interactive: each occupied slot toggles its signer's approval. With `tour`, three keys turn in once the
 * figure scrolls into view, unless the reader has already touched it.
 */
export function LockFigure({ tour = false, fig = "Fig. 1" }: { tour?: boolean; fig?: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const inView = useInView(ref, { once: true, amount: 0.5 });
  const [signed, setSigned] = useState<ReadonlySet<number>>(() => new Set(tour ? [] : [0, 2]));
  const touched = useRef(false);

  useEffect(() => {
    if (!tour || !inView) return;
    const timers = TOUR.map((slot, i) =>
      window.setTimeout(
        () => {
          if (!touched.current) setSigned((s) => new Set([...s, slot]));
        },
        500 + i * 650,
      ),
    );
    return () => timers.forEach((t) => window.clearTimeout(t));
  }, [tour, inView]);

  const toggle = (index: number) => {
    touched.current = true;
    setSigned((current) => {
      const next = new Set(current);
      if (!next.delete(index)) next.add(index);
      return next;
    });
  };
  const onKey = (event: KeyboardEvent, index: number) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    toggle(index);
  };

  const count = signed.size;
  const authorized = count >= THRESHOLD;

  return (
    <div ref={ref}>
      <FigureCard
        fig={fig}
        title={`Auth witness · ${THRESHOLD} of ${SIGNERS}`}
        hint="Select a lock to sign"
        readout={
          <>
            {count} / {THRESHOLD} approvals ·{" "}
            <span className={authorized ? "text-olive" : "text-coral"}>
              {authorized ? "authorized" : "locked"}
            </span>
          </>
        }
      >
        <svg
          className="iso-svg"
          viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
          role="group"
          aria-label={`A strongbox with eight signer slots, five of them locks. ${count} of ${SIGNERS} signers have turned their key; ${THRESHOLD} are needed to withdraw the bolt.`}
        >
          <Base art={BASE_ART} />
          <Solid paths={STRIKE_ART.pad} tone="lo" />
          <Solid paths={BOX_ART.plinth} tone="mid" />
          <Solid paths={BOX_ART.body} tone="hi" inner={<Line d={BOX_ART.lidSeam} tone="lo" />} />
          <Dots points={BOX_ART.rivets} size={0.6} tone="mid" />
          {BOX_ART.guards.map((paths, i) => (
            <Solid key={i} paths={paths} tone="mid" />
          ))}
          <Line d={BOX_ART.lidGroove} tone="faint" />
          <FaceText transform={BOX_ART.name} size={5} tone="lo">
            CLAVE · MAX_SIGNERS 8
          </FaceText>

          <Solid paths={RULER_ART.strip} tone="lo" crease="none" />
          <g transform={`translate(${RULER_ART.ticksAt[0]} ${RULER_ART.ticksAt[1]})`}>
            <Line d={RULER_ART.ticks.minor} tone="lo" />
            <Line d={RULER_ART.ticks.major} tone="mid" />
          </g>
          <Solid paths={RULER_ART.threshold} tone="hi" className="accent-ink" />
          <Rider count={count} authorized={authorized} />

          {BY_DEPTH.map((art) => {
            const { slot } = art;
            if (!("bezel" in art) || !art.bezel)
              return (
                <g key={slot.index}>
                  {"cap" in art && art.cap && <Solid paths={art.cap} tone="lo" crease="none" />}
                  {"capRing" in art && <Line d={art.capRing ?? ""} tone="faint" />}
                  {"center" in art && art.center && (
                    <Dots points={[art.center]} size={0.7} tone="lo" />
                  )}
                </g>
              );
            const on = signed.has(slot.index);
            return (
              <g key={slot.index}>
                <Solid paths={art.bezel} tone="mid" inner={<Line d={art.knurl} tone="faint" />} />
                <Solid paths={art.plug} tone="hi" />
                <Line d={art.pins} tone="faint" />
                <Line d={on ? art.turned : art.keyway} tone="hi" />
                <AnimatePresence initial={false}>
                  {on && (
                    <motion.g
                      key="key"
                      initial={{ opacity: 0, y: -14 }}
                      animate={{ opacity: 1, y: 0 }}
                      exit={{ opacity: 0, y: -10 }}
                      transition={{ duration: 0.32, ease: EASE_OUT }}
                    >
                      <Solid paths={art.shank} tone="hi" />
                      <Solid
                        paths={art.bow}
                        tone="hi"
                        className="accent-signal"
                        inner={<Line d={art.hole} tone="hi" />}
                      />
                    </motion.g>
                  )}
                </AnimatePresence>
              </g>
            );
          })}

          <Bolt open={authorized} />
          <Solid
            paths={STRIKE_ART.block}
            tone="hi"
            inner={<Line d={STRIKE_ART.seam} tone="lo" />}
          />
          <Dots points={STRIKE_ART.screws} size={0.6} tone="mid" />

          {SLOT_ART.filter((art) => art.slot.occupied).map(({ slot }) => {
            const [cx, cy] = at(slotCenter(slot));
            const on = signed.has(slot.index);
            return (
              <ellipse
                key={slot.index}
                cx={cx}
                cy={cy - 4}
                rx={18}
                ry={14}
                fill="transparent"
                role="button"
                tabIndex={0}
                aria-pressed={on}
                aria-label={`Signer ${slot.index}: ${on ? "approved" : "no approval"}`}
                onClick={() => toggle(slot.index)}
                onKeyDown={(event) => onKey(event, slot.index)}
                className="cursor-pointer outline-none focus-visible:stroke-olive focus-visible:stroke-[1.5]"
              />
            );
          })}
        </svg>
      </FigureCard>
    </div>
  );
}
