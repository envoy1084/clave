import { motion } from "motion/react";

import { EASE_OUT } from "@/lib/motion";

import { DarkDiagram, INK, PAPER, SIGNAL } from "./frame";

// What a member signs: four values hashed into one approval message. Steps: 1-4 each value enters, 5 the hash runs,
// 6 the approval appears, 7 signatures attach to it.
const DELAYS = [350, 300, 300, 300, 500, 600, 700] as const;
const FIELDS = ["vault address", "transaction hash", "config version", "deadline"];
const HASH = { x: 250, y: 95, w: 130, h: 70 };
const OUT = { x: 440, y: 105, w: 140, h: 50 };
const FONT = "Geist Variable, sans-serif";

export function ApprovalDiagram({ className }: { className?: string }) {
  return (
    <DarkDiagram
      className={className}
      label="Four values are hashed with Poseidon2 into one approval message, which k members sign."
      delays={DELAYS}
      viewBox="0 0 600 260"
    >
      {(step) => (
        <>
          {FIELDS.map((f, i) => {
            const y = 30 + i * 52;
            const shown = step > i;
            return (
              <motion.g
                key={f}
                initial={false}
                animate={{ opacity: shown ? 1 : 0, x: shown ? 0 : -12 }}
                transition={{ duration: 0.45, ease: EASE_OUT }}
              >
                <rect
                  x={10}
                  y={y}
                  width={180}
                  height={36}
                  rx={10}
                  fill="none"
                  stroke={PAPER}
                  strokeOpacity={0.35}
                />
                <text
                  x={100}
                  y={y + 23}
                  textAnchor="middle"
                  fontSize={14}
                  fill={PAPER}
                  fontFamily={FONT}
                >
                  {f}
                </text>
                <line
                  x1={190}
                  y1={y + 18}
                  x2={HASH.x}
                  y2={HASH.y + HASH.h / 2}
                  stroke={PAPER}
                  strokeOpacity={0.25}
                />
              </motion.g>
            );
          })}
          <motion.rect
            x={HASH.x}
            y={HASH.y}
            width={HASH.w}
            height={HASH.h}
            rx={14}
            stroke={PAPER}
            strokeOpacity={0.6}
            initial={false}
            animate={{ fill: step >= 5 ? "rgb(251 248 242 / 0.12)" : "rgb(251 248 242 / 0)" }}
            transition={{ duration: 0.5, ease: EASE_OUT }}
          />
          <text
            x={HASH.x + HASH.w / 2}
            y={HASH.y + 42}
            textAnchor="middle"
            fontSize={16}
            fill={PAPER}
            fontFamily={FONT}
          >
            Poseidon2
          </text>
          <motion.g
            initial={false}
            animate={{ opacity: step >= 6 ? 1 : 0 }}
            transition={{ duration: 0.5, ease: EASE_OUT }}
          >
            <line
              x1={HASH.x + HASH.w}
              y1={HASH.y + HASH.h / 2}
              x2={OUT.x}
              y2={OUT.y + OUT.h / 2}
              stroke={PAPER}
              strokeOpacity={0.5}
            />
            <rect x={OUT.x} y={OUT.y} width={OUT.w} height={OUT.h} rx={12} fill={SIGNAL} />
            <text
              x={OUT.x + OUT.w / 2}
              y={OUT.y + 31}
              textAnchor="middle"
              fontSize={16}
              fill={INK}
              fontFamily={FONT}
              fontWeight={500}
            >
              approval
            </text>
          </motion.g>
          {[0, 1, 2].map((i) => (
            <motion.rect
              key={i}
              x={OUT.x + 30 + i * 30}
              y={OUT.y + OUT.h + 16}
              width={18}
              height={18}
              rx={4}
              fill={PAPER}
              initial={false}
              animate={{ opacity: step >= 7 ? 1 : 0, y: step >= 7 ? 0 : 8 }}
              transition={{ duration: 0.4, ease: EASE_OUT, delay: step >= 7 ? i * 0.15 : 0 }}
            />
          ))}
          <motion.text
            x={OUT.x + OUT.w / 2}
            y={OUT.y + OUT.h + 58}
            textAnchor="middle"
            fontSize={13}
            fill={PAPER}
            fillOpacity={0.75}
            fontFamily={FONT}
            initial={false}
            animate={{ opacity: step >= 7 ? 1 : 0 }}
          >
            k members sign it
          </motion.text>
        </>
      )}
    </DarkDiagram>
  );
}
