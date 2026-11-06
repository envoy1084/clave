import { motion } from "motion/react";

import { Base } from "@/components/iso/base";
import { Dots, FaceText, Line, Solid } from "@/components/iso/draw";
import { EASE_OUT } from "@/lib/motion";

import {
  BASE_ART,
  HEIGHT,
  PILLAR_CAPS,
  PILLAR_FEET,
  PILLAR_RUNS,
  TRAYS,
  WIDTH,
  liftOffset,
  type TrayArt,
} from "./geometry";

const ACCENTS = ["accent-signal", "accent-peri", "accent-signal", "accent-signal"] as const;

function Tray({ art, active, accent }: { art: TrayArt; active: boolean; accent: string }) {
  return (
    <motion.g
      initial={false}
      animate={{ x: active ? liftOffset[0] : 0, y: active ? liftOffset[1] : 0 }}
      transition={{ type: "spring", duration: 0.6, bounce: 0.15 }}
    >
      <Solid paths={art.slab} tone={active ? "hi" : "mid"} halo={active} />
      <Line d={art.groove} tone="faint" />
      {art.bushings.map((paths, i) => (
        <Solid key={i} paths={paths} tone="mid" />
      ))}
      {art.parts.map((part, i) => (
        <Solid
          key={i}
          paths={part.paths}
          tone={active ? "hi" : "mid"}
          className={active && part.accent ? accent : undefined}
        />
      ))}
      {art.lines.map((d, i) => (
        <Line key={i} d={d} tone={active ? "mid" : "lo"} free />
      ))}
      <Dots points={art.dots} size={0.7} tone={active ? "hi" : "mid"} />
      {art.label && (
        <FaceText transform={art.label.transform} size={6} tone={active ? "hi" : "lo"}>
          {art.label.text}
        </FaceText>
      )}
    </motion.g>
  );
}

/**
 * The rack, with tray `active` (0 network … 3 device) lifted and in colour. Pillars are drawn a run at a time between
 * trays, so each tray covers the pillars behind it and the bushings close over the ones in front.
 */
export function RackFigure({ active }: { active: number }) {
  return (
    <svg
      className="iso-svg"
      viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
      role="img"
      aria-label="Four trays on guide pillars: the Aztec network at the bottom, then the relay, the member's keeper, and the member's device on top."
    >
      <Base art={BASE_ART} />
      {PILLAR_FEET.map((paths, i) => (
        <Solid key={i} paths={paths} tone="mid" />
      ))}
      {TRAYS.map((art, i) => (
        <g key={i}>
          {PILLAR_RUNS[i]?.map((paths, k) => (
            <Solid key={k} paths={paths} tone="mid" />
          ))}
          <Tray art={art} active={active === i} accent={ACCENTS[i] ?? "accent-signal"} />
        </g>
      ))}
      {PILLAR_RUNS[4]?.map((paths, k) => (
        <Solid key={k} paths={paths} tone="mid" />
      ))}
      {PILLAR_CAPS.map((paths, i) => (
        <motion.g key={i} initial={false} animate={{ opacity: 1 }} transition={{ ease: EASE_OUT }}>
          <Solid paths={paths} tone="hi" />
        </motion.g>
      ))}
    </svg>
  );
}
