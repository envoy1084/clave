import { useRef } from "react";

import { motion, useInView, useReducedMotion } from "motion/react";

import { GATES } from "@/lib/content";
import { EASE_OUT } from "@/lib/motion";

const MAX = Math.max(...GATES.map((g) => g.tecpan));
const fmt = (n: number) => n.toLocaleString("en-US");

function Bar({
  value,
  tone,
  grow,
  delay,
}: {
  value: number;
  tone: string;
  grow: boolean;
  delay: number;
}) {
  return (
    <div className="h-2.5 overflow-hidden rounded-full bg-paper-2">
      <motion.div
        className={`h-full origin-left rounded-full ${tone}`}
        style={{ width: `${(value / MAX) * 100}%` }}
        initial={false}
        animate={{ scaleX: grow ? 1 : 0 }}
        transition={{ duration: 0.9, ease: EASE_OUT, delay }}
      />
    </div>
  );
}

/** Gate counts per circuit, Tecpan's 8-slot account against Aztec's single-key Schnorr account. */
export function GateChart() {
  const ref = useRef<HTMLDivElement>(null);
  const inView = useInView(ref, { once: true, amount: 0.4 });
  const reduce = useReducedMotion();
  const grow = inView || Boolean(reduce);
  return (
    <div ref={ref}>
      <div className="mb-5 flex gap-5 text-[0.78rem] text-ink-soft" aria-hidden>
        <span className="inline-flex items-center gap-2">
          <span className="size-2.5 rounded-full bg-ink" /> Tecpan, 8 slots
        </span>
        <span className="inline-flex items-center gap-2">
          <span className="size-2.5 rounded-full bg-peri" /> Schnorr account
        </span>
      </div>
      <table className="w-full text-left">
        <caption className="sr-only">Circuit gate counts</caption>
        <thead className="sr-only">
          <tr>
            <th scope="col">Circuit</th>
            <th scope="col">Tecpan and Schnorr account</th>
          </tr>
        </thead>
        <tbody>
          {GATES.map((g, i) => (
            <tr
              key={g.circuit}
              className="block py-3 sm:grid sm:grid-cols-[12rem_1fr] sm:items-center sm:gap-6"
            >
              <th scope="row" className="block font-mono text-[0.82rem] font-normal">
                {g.circuit}
              </th>
              <td className="mt-2 block space-y-1.5 sm:mt-0">
                <div className="grid grid-cols-[1fr_4.5rem] items-center gap-3">
                  <Bar value={g.tecpan} tone="bg-ink" grow={grow} delay={i * 0.08} />
                  <span className="tabular text-right font-mono text-[0.78rem]">
                    {fmt(g.tecpan)}
                  </span>
                </div>
                <div className="grid grid-cols-[1fr_4.5rem] items-center gap-3">
                  {g.schnorr === null ? (
                    <span className="text-[0.75rem] text-muted">no equivalent</span>
                  ) : (
                    <Bar value={g.schnorr} tone="bg-peri" grow={grow} delay={i * 0.08 + 0.05} />
                  )}
                  <span className="tabular text-right font-mono text-[0.78rem] text-muted">
                    {g.schnorr === null ? "" : fmt(g.schnorr)}
                  </span>
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
