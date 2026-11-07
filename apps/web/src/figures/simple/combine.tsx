import { motion } from "motion/react";

import { EASE_OUT } from "@/lib/motion";

import { DarkDiagram, INK, PAPER, PERI, SIGNAL } from "./frame";

// Three keepers each turn the note's key into a piece using only their own share; the pieces combine into the value
// that opens the note. The full viewing key appears nowhere. Steps: 1-3 pieces arrive, 4 combine, 5 the note opens.
const DELAYS = [450, 450, 450, 600, 600] as const;
const KEEPERS = [0, 1, 2].map((i) => ({ x: 90, y: 40 + i * 70 }));
const JOIN = { x: 430, y: 110 };
const OUT = { x: 560, y: 85, w: 200, h: 50 };
const FONT = "Geist Variable, sans-serif";

export function CombineDiagram({ className }: { className?: string }) {
  return (
    <DarkDiagram
      className={className}
      label="Three keepers send pieces made from their own shares; the pieces combine to open the note, and the full key never appears."
      delays={DELAYS}
      viewBox="0 0 800 220"
    >
      {(step) => (
        <>
          {KEEPERS.map((k, i) => {
            const sent = step > i;
            return (
              <g key={i}>
                <circle cx={k.x} cy={k.y} r={18} fill="none" stroke={PAPER} strokeOpacity={0.6} />
                <text
                  x={k.x - 34}
                  y={k.y + 5}
                  textAnchor="end"
                  fontSize={13}
                  fill={PAPER}
                  fillOpacity={0.75}
                  fontFamily={FONT}
                >
                  keeper {i + 1}
                </text>
                <line
                  x1={k.x + 18}
                  y1={k.y}
                  x2={JOIN.x - 22}
                  y2={JOIN.y}
                  stroke={PAPER}
                  strokeOpacity={0.2}
                />
                <motion.g
                  initial={false}
                  animate={
                    sent
                      ? {
                          x: JOIN.x - 26 - 86 - (k.x + 30),
                          y: JOIN.y - k.y,
                          opacity: step >= 5 ? 0 : 1,
                        }
                      : { x: 0, y: 0, opacity: 0 }
                  }
                  transition={{ duration: 0.7, ease: EASE_OUT }}
                >
                  <rect x={k.x + 30} y={k.y - 13} width={86} height={26} rx={8} fill={PERI} />
                  <text
                    x={k.x + 73}
                    y={k.y + 5}
                    textAnchor="middle"
                    fontSize={13}
                    fill={INK}
                    fontFamily={FONT}
                  >
                    piece {i + 1}
                  </text>
                </motion.g>
              </g>
            );
          })}
          <motion.circle
            cx={JOIN.x}
            cy={JOIN.y}
            r={22}
            stroke={PAPER}
            strokeOpacity={0.6}
            initial={false}
            animate={{ fill: step >= 4 ? SIGNAL : "rgb(251 248 242 / 0)" }}
            transition={{ duration: 0.5, ease: EASE_OUT }}
          />
          <text
            x={JOIN.x}
            y={JOIN.y + 6}
            textAnchor="middle"
            fontSize={18}
            fill={step >= 4 ? INK : PAPER}
            fontFamily={FONT}
          >
            +
          </text>
          <motion.g
            initial={false}
            animate={{ opacity: step >= 5 ? 1 : 0 }}
            transition={{ duration: 0.5, ease: EASE_OUT }}
          >
            <line
              x1={JOIN.x + 22}
              y1={JOIN.y}
              x2={OUT.x}
              y2={JOIN.y}
              stroke={PAPER}
              strokeOpacity={0.5}
            />
            <rect
              x={OUT.x}
              y={OUT.y}
              width={OUT.w}
              height={OUT.h}
              rx={12}
              fill="none"
              stroke={SIGNAL}
              strokeWidth={1.5}
            />
            <text
              x={OUT.x + OUT.w / 2}
              y={OUT.y + 31}
              textAnchor="middle"
              fontSize={15}
              fill={PAPER}
              fontFamily={FONT}
            >
              note decrypted
            </text>
          </motion.g>
          <text
            x={JOIN.x}
            y={205}
            textAnchor="middle"
            fontSize={13}
            fill={PAPER}
            fillOpacity={0.75}
            fontFamily={FONT}
          >
            the full viewing key is never rebuilt
          </text>
        </>
      )}
    </DarkDiagram>
  );
}
