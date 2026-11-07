import { motion } from "motion/react";

import { EASE_OUT } from "@/lib/motion";

import { INK, Illustration, Label, PAPER, PERI, SIGNAL } from "./frame";

// A note arrives sealed (the mosaic). The wallet asks every keeper; three of five answer with a piece, and the pieces
// together open the note. Steps: 1 the request goes out, 2-4 answers come back, 5 the note opens.
const DELAYS = [700, 900, 700, 700, 800] as const;
const NOTE = { x: 70, y: 110, w: 190, h: 100 };
const KEEPERS = [0, 1, 2, 3, 4].map((i) => ({ x: 600, y: 45 + i * 57 }));
const ANSWERING = [0, 2, 3];
const MOSAIC = ["#15180F", "#A7A9FF", "#5BABFF", "#3b2f7a", "#C8F500", "#2b2f4a"];
const CELLS = Array.from({ length: 8 * 4 }, (_, i) => ({
  x: NOTE.x + 15 + (i % 8) * 20,
  y: NOTE.y + 12 + Math.floor(i / 8) * 19,
  color: MOSAIC[(i * 7 + Math.floor(i / 8)) % MOSAIC.length] ?? INK,
  delay: ((i * 13) % 32) / 64,
}));
const ease = { duration: 0.6, ease: EASE_OUT };

export function ViewingIllustration() {
  return (
    <Illustration
      label="A sealed note is opened by combining answers from three of five keepers."
      caption="No keeper can open the note alone. Any three of the five answer with a piece, and together the pieces open it."
      delays={DELAYS}
      viewBox="0 0 720 320"
    >
      {(step) => {
        const answers = Math.max(0, Math.min(step - 1, 3));
        const open = step >= 5;
        return (
          <>
            <rect
              x={NOTE.x}
              y={NOTE.y}
              width={NOTE.w}
              height={NOTE.h}
              rx={14}
              fill={PAPER}
              stroke={INK}
              strokeWidth={1.5}
            />
            <motion.text
              x={NOTE.x + NOTE.w / 2}
              y={NOTE.y + NOTE.h / 2 + 9}
              textAnchor="middle"
              fontSize={26}
              fill={INK}
              fontFamily="Newsreader Variable, serif"
              initial={false}
              animate={{ opacity: open ? 1 : 0 }}
              transition={{ ...ease, delay: open ? 0.3 : 0 }}
            >
              100 JADE
            </motion.text>
            {CELLS.map((c, i) => (
              <motion.rect
                key={i}
                x={c.x}
                y={c.y}
                width={18}
                height={17}
                fill={c.color}
                initial={false}
                animate={{ opacity: open ? 0 : 0.85 }}
                transition={{ duration: 0.3, ease: EASE_OUT, delay: open ? c.delay : 0 }}
              />
            ))}
            <Label x={NOTE.x + NOTE.w / 2} y={NOTE.y + NOTE.h + 28}>
              {open ? "note opened" : "encrypted note"}
            </Label>

            {KEEPERS.map((k, i) => {
              const order = ANSWERING.indexOf(i);
              const answered = order >= 0 && step > order + 1;
              return (
                <g key={i}>
                  <motion.line
                    x1={NOTE.x + NOTE.w + 8}
                    y1={NOTE.y + NOTE.h / 2}
                    x2={k.x - 22}
                    y2={k.y}
                    stroke={INK}
                    strokeWidth={1}
                    strokeDasharray="4 5"
                    initial={false}
                    animate={{ opacity: step >= 1 ? 0.35 : 0 }}
                    transition={ease}
                  />
                  <motion.circle
                    cx={k.x}
                    cy={k.y}
                    r={18}
                    stroke={INK}
                    strokeWidth={1.5}
                    initial={false}
                    animate={{ fill: answered ? PERI : PAPER }}
                    transition={ease}
                  />
                  {order >= 0 && (
                    <motion.rect
                      width={14}
                      height={14}
                      rx={3}
                      fill={open ? SIGNAL : PERI}
                      stroke={INK}
                      strokeWidth={1}
                      initial={false}
                      animate={
                        answered
                          ? {
                              x: NOTE.x + NOTE.w + 14,
                              y: NOTE.y + 22 + order * 22,
                              opacity: open ? 0 : 1,
                            }
                          : { x: k.x - 7, y: k.y - 7, opacity: 0 }
                      }
                      transition={{ duration: 0.7, ease: EASE_OUT }}
                    />
                  )}
                </g>
              );
            })}
            <Label x={600} y={316}>
              keepers
            </Label>
            <text
              x={420}
              y={300}
              textAnchor="middle"
              fontSize={15}
              fill={INK}
              fontFamily="Geist Variable, sans-serif"
            >
              {answers} of 3 answers
            </text>
          </>
        );
      }}
    </Illustration>
  );
}
