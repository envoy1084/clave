import { motion } from "motion/react";

import { EASE_OUT } from "@/lib/motion";

import { INK, Illustration, Label, PAPER, SIGNAL } from "./frame";

// A 3-of-5 vault: three members sign, one at a time; at the third signature the vault opens and its proof lands
// on the chain. Steps: 1-3 signatures, 4 the vault opens, 5 the proof travels, 6 the block is added.
const DELAYS = [700, 800, 800, 800, 700, 800] as const;
const MEMBERS = [0, 1, 2, 3, 4].map((i) => ({ x: 90, y: 50 + i * 55 }));
const SIGNERS = [0, 2, 3];
const VAULT = { x: 300, y: 100, size: 120 };
const VAULT_CENTER = { x: VAULT.x + VAULT.size / 2, y: VAULT.y + VAULT.size / 2 };
const BLOCKS = [540, 590, 640] as const;
/** Centre of the newest block, where the proof lands. */
const LANDING = { x: 640 + 17, y: 160 };
const ARCH =
  "M7 31V20A13 13 0 0 1 18 7.15V13.82A6.5 6.5 0 0 0 13.5 20V31Z M33 31V20A13 13 0 0 0 22 7.15V13.82A6.5 6.5 0 0 1 26.5 20V31Z M4 33.5H36V37.5H4Z";
const ease = { duration: 0.6, ease: EASE_OUT };

export function SpendingIllustration() {
  return (
    <Illustration
      label="Three of five members sign; the vault opens and its proof is added to the chain."
      caption="Three of five members sign. Only then does the vault accept the transaction and send its proof to Aztec."
      delays={DELAYS}
      viewBox="0 0 720 320"
    >
      {(step) => {
        const signed = Math.min(step, 3);
        const open = step >= 4;
        return (
          <>
            {MEMBERS.map((m, i) => {
              const order = SIGNERS.indexOf(i);
              const hasSigned = order >= 0 && step > order;
              return (
                <g key={i}>
                  <motion.circle
                    cx={m.x}
                    cy={m.y}
                    r={17}
                    stroke={INK}
                    strokeWidth={1.5}
                    initial={false}
                    animate={{ fill: hasSigned ? INK : PAPER }}
                    transition={ease}
                  />
                  {order >= 0 && (
                    <motion.rect
                      width={12}
                      height={12}
                      x={m.x - 6}
                      y={m.y - 6}
                      fill={SIGNAL}
                      stroke={INK}
                      strokeWidth={1}
                      initial={false}
                      animate={
                        hasSigned
                          ? {
                              x: VAULT_CENTER.x - 6 + (order - 1) * 18,
                              y: VAULT.y + 88,
                              opacity: 1,
                            }
                          : { x: m.x - 6, y: m.y - 6, opacity: 0 }
                      }
                      transition={{ duration: 0.7, ease: EASE_OUT }}
                    />
                  )}
                </g>
              );
            })}
            <Label x={90} y={316}>
              members
            </Label>

            <motion.rect
              x={VAULT.x}
              y={VAULT.y}
              width={VAULT.size}
              height={VAULT.size}
              rx={22}
              stroke={INK}
              strokeWidth={1.5}
              initial={false}
              animate={{ fill: open ? SIGNAL : PAPER }}
              transition={ease}
            />
            <path
              d={ARCH}
              fill={INK}
              transform={`translate(${VAULT.x + 36} ${VAULT.y + 30}) scale(1.2)`}
              opacity={0.85}
            />
            <text
              x={VAULT_CENTER.x}
              y={VAULT.y + VAULT.size + 30}
              textAnchor="middle"
              fontSize={15}
              fill={INK}
              fontFamily="Geist Variable, sans-serif"
            >
              {signed} of 3 signatures
            </text>

            <motion.rect
              width={22}
              height={22}
              rx={4}
              fill={INK}
              initial={false}
              animate={
                step >= 5
                  ? { x: LANDING.x - 11, y: LANDING.y - 11, opacity: step >= 6 ? 0 : 1 }
                  : { x: VAULT_CENTER.x - 11, y: VAULT_CENTER.y - 11, opacity: 0 }
              }
              transition={{ duration: 0.8, ease: EASE_OUT }}
            />
            {BLOCKS.map((x, i) => {
              const last = i === BLOCKS.length - 1;
              return (
                <motion.rect
                  key={x}
                  x={x}
                  y={143}
                  width={34}
                  height={34}
                  rx={6}
                  stroke={INK}
                  strokeWidth={1.5}
                  initial={false}
                  animate={{
                    fill: last && step >= 6 ? SIGNAL : PAPER,
                    opacity: last && step < 6 ? 0.35 : 1,
                  }}
                  transition={ease}
                />
              );
            })}
            <line x1={574} x2={590} y1={160} y2={160} stroke={INK} strokeWidth={1.5} />
            <line x1={624} x2={640} y1={160} y2={160} stroke={INK} strokeWidth={1.5} />
            <Label x={607} y={316}>
              Aztec
            </Label>
          </>
        );
      }}
    </Illustration>
  );
}
