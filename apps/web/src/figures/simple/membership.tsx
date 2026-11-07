import { motion } from "motion/react";

import { EASE_OUT } from "@/lib/motion";

import { INK, Illustration, Label, MUTED, PAPER, PERI, SIGNAL } from "./frame";

// Five members hold shares of the vault's key. One leaves; the four who stay deal new shares among themselves; the
// old share no longer fits, and the vault's address is unchanged. Steps: 1 leaves, 2 reshare, 3 new shares, 4 address.
const DELAYS = [800, 900, 900, 800] as const;
const CENTER = { x: 360, y: 150 };
const RADIUS = 115;
const MEMBERS = [0, 1, 2, 3, 4].map((i) => {
  const angle = -Math.PI / 2 + (i * 2 * Math.PI) / 5;
  return { x: CENTER.x + RADIUS * Math.cos(angle), y: CENTER.y + RADIUS * Math.sin(angle) };
});
const LEAVING = 4;
const STAYING = [0, 1, 2, 3];
const PAIRS = STAYING.flatMap((a, i) => STAYING.slice(i + 1).map((b) => [a, b] as const));
const ARCH =
  "M7 31V20A13 13 0 0 1 18 7.15V13.82A6.5 6.5 0 0 0 13.5 20V31Z M33 31V20A13 13 0 0 0 22 7.15V13.82A6.5 6.5 0 0 1 26.5 20V31Z M4 33.5H36V37.5H4Z";
const ease = { duration: 0.6, ease: EASE_OUT };

export function MembershipIllustration() {
  return (
    <Illustration
      label="One of five members leaves; the four who stay reshare the key and the vault keeps its address."
      caption="The four who stay deal new shares of the same key. The old share no longer fits, and the vault keeps its address."
      delays={DELAYS}
      viewBox="0 0 720 320"
    >
      {(step) => {
        const left = step >= 1;
        const resharing = step >= 2;
        const renewed = step >= 3;
        return (
          <>
            {PAIRS.map(([a, b]) => {
              const from = MEMBERS[a];
              const to = MEMBERS[b];
              if (!from || !to) return null;
              return (
                <motion.line
                  key={`${a}-${b}`}
                  x1={from.x}
                  y1={from.y}
                  x2={to.x}
                  y2={to.y}
                  stroke={INK}
                  strokeWidth={1}
                  strokeDasharray="4 5"
                  initial={false}
                  animate={{ opacity: resharing && !renewed ? 0.45 : 0 }}
                  transition={ease}
                />
              );
            })}

            <rect
              x={CENTER.x - 34}
              y={CENTER.y - 34}
              width={68}
              height={68}
              rx={16}
              fill={PAPER}
              stroke={INK}
              strokeWidth={1.5}
            />
            <path
              d={ARCH}
              fill={INK}
              transform={`translate(${CENTER.x - 20} ${CENTER.y - 21}) scale(1)`}
            />

            {MEMBERS.map((m, i) => {
              const leaving = i === LEAVING;
              const dx = leaving && left ? (m.x - CENTER.x) * 0.45 : 0;
              const dy = leaving && left ? (m.y - CENTER.y) * 0.45 : 0;
              const shareColor = leaving ? (left ? MUTED : PERI) : renewed ? SIGNAL : PERI;
              return (
                <motion.g
                  key={i}
                  initial={false}
                  animate={{ x: dx, y: dy, opacity: leaving && left ? 0.45 : 1 }}
                  transition={{ duration: 0.8, ease: EASE_OUT }}
                >
                  <circle cx={m.x} cy={m.y} r={18} fill={PAPER} stroke={INK} strokeWidth={1.5} />
                  <motion.rect
                    x={m.x + 12}
                    y={m.y - 22}
                    width={14}
                    height={14}
                    rx={3}
                    stroke={INK}
                    strokeWidth={1}
                    initial={false}
                    animate={{ fill: shareColor }}
                    transition={ease}
                  />
                </motion.g>
              );
            })}

            <motion.g initial={false} animate={{ opacity: step >= 4 ? 1 : 0 }} transition={ease}>
              <text
                x={CENTER.x}
                y={CENTER.y + 58}
                textAnchor="middle"
                fontSize={14}
                fill={INK}
                fontFamily="Geist Variable, sans-serif"
              >
                same address
              </text>
            </motion.g>
            <Label x={CENTER.x} y={312}>
              {renewed ? "four members, new shares" : "five members, one share each"}
            </Label>
          </>
        );
      }}
    </Illustration>
  );
}
