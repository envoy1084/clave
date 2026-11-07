import { useMemo } from "react";

import { motion, useReducedMotion } from "motion/react";

import { Terrain } from "@/components/brand/terrain";
import { ButtonLink } from "@/components/ui/button";
import { Container } from "@/components/ui/section";
import { LINKS } from "@/lib/content";
import { EASE_OUT } from "@/lib/motion";

// The mosaic is the brand's privacy motif (the same pixels hide values in the network view below). On load a field of
// pixels resolves over the landscape once, the way a note resolves for a quorum; then everything stays still.
const PIXELS = ["#C8F500", "#A7A9FF", "#5BABFF", "#E6D5FF", "#7FD8FF"];
const CELL = 18;

function usePixels(count: number) {
  return useMemo(() => {
    let s = 4111;
    const random = () => (s = (s * 16807) % 2147483647) / 2147483647;
    return Array.from({ length: count }, (_, i) => {
      // Clustered towards the right two thirds, where the landscape is brightest and no text sits.
      const col = Math.floor(28 + random() * 70);
      const row = Math.floor(8 + random() * 80);
      return {
        key: i,
        left: `${col}%`,
        top: `${row}%`,
        color: PIXELS[Math.floor(random() * PIXELS.length)] ?? "#A7A9FF",
        opacity: 0.35 + random() * 0.55,
        delay: 0.3 + random() * 1.4,
      };
    });
  }, [count]);
}

export function Hero() {
  const reduce = useReducedMotion();
  const pixels = usePixels(46);
  const rise = (delay: number) => ({
    initial: { opacity: 0, y: reduce ? 0 : 14 },
    animate: { opacity: 1, y: 0 },
    transition: { duration: 0.7, ease: EASE_OUT, delay },
  });
  return (
    <section className="relative isolate -mt-16 overflow-hidden bg-navy text-paper">
      <Terrain
        seed={9}
        shade="left"
        priority
        clusters={[
          [420, 60, 12, 8],
          [590, 210, 8, 7],
          [470, 330, 6, 5],
        ]}
        className="absolute inset-0 -z-20"
      />
      <div aria-hidden className="absolute inset-0 -z-10">
        {pixels.map((p) => (
          <motion.span
            key={p.key}
            className="absolute hidden sm:block"
            style={{
              left: p.left,
              top: p.top,
              width: CELL,
              height: CELL,
              backgroundColor: p.color,
            }}
            initial={{ opacity: 0 }}
            animate={{ opacity: p.opacity }}
            transition={{ duration: reduce ? 0 : 0.5, ease: EASE_OUT, delay: reduce ? 0 : p.delay }}
          />
        ))}
      </div>
      {/* Keeps the navigation readable where the landscape runs under it. */}
      <div
        aria-hidden
        className="absolute inset-x-0 top-0 -z-10 h-40 bg-[linear-gradient(to_bottom,rgb(14_21_48/0.75),transparent)]"
      />
      {/* Keeps the text column readable over the brightest parts of the landscape. */}
      <div
        aria-hidden
        className="absolute inset-0 -z-10 bg-[linear-gradient(to_top,var(--color-navy)_8%,transparent_70%)] sm:bg-[linear-gradient(to_right,var(--color-navy)_22%,rgb(14_21_48/0.55)_55%,transparent_85%)]"
      />
      <Container wide className="flex min-h-[100svh] flex-col justify-end pt-32 pb-12 sm:pb-16">
        <motion.h1
          {...rise(0)}
          className="serif-display max-w-[13ch] text-[3rem] leading-[0.98] sm:text-[5.4rem]"
        >
          Shared custody, <em className="serif-em text-signal">private by default.</em>
        </motion.h1>
        <motion.p
          {...rise(0.1)}
          className="mt-6 max-w-[46ch] text-[1.05rem] leading-relaxed text-paper/80 sm:text-[1.15rem]"
        >
          Clave is a multisig vault on Aztec. Payments need k of n signatures, and only t of n
          members together can read what the vault holds.
        </motion.p>
        <motion.div {...rise(0.18)} className="mt-9 flex flex-wrap gap-3">
          <ButtonLink href="#how" variant="signal">
            See how it works
          </ButtonLink>
          <ButtonLink href={LINKS.repo} variant="outline-dark">
            Read the source
          </ButtonLink>
        </motion.div>
      </Container>
    </section>
  );
}
