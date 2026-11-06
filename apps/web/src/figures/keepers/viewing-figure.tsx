import { useState } from "react";

import { AnimatePresence, motion } from "motion/react";

import { FigureCard } from "@/components/iso/figure-card";
import { quick } from "@/lib/motion";

import { postsAt } from "./geometry";
import { KeeperTable } from "./keeper-table";
import { lagrange } from "./lagrange";

// A 3-of-5 viewing committee. The front of the table is left open so the lens stays in view.
const POSTS = postsAt([108, 168, 228, 288, 348]);
const T_V = 3;

/**
 * Threshold viewing, interactive: select posts to take keepers offline. The first t_v answering keepers form S (as
 * `verify_and_combine` does) and their real Lagrange coefficients at zero are shown under the figure.
 */
export function ViewingFigure({ fig = "Fig. 2" }: { fig?: string }) {
  const [online, setOnline] = useState<ReadonlySet<number>>(() => new Set([1, 3, 4, 5]));
  const answering = POSTS.filter((p) => online.has(p.id)).map((p) => p.id);
  const set = answering.slice(0, T_V);
  const open = set.length === T_V;
  const toggle = (id: number) =>
    setOnline((current) => {
      const next = new Set(current);
      if (!next.delete(id)) next.add(id);
      return next;
    });

  return (
    <FigureCard
      fig={fig}
      title={`Threshold ECDH · t_v ${T_V} of ${POSTS.length}`}
      hint="Select a keeper to take it offline"
      readout={
        <>
          {answering.length} answering ·{" "}
          <span className={open ? "text-olive" : "text-coral"}>
            {open ? "note readable" : "note sealed"}
          </span>
        </>
      }
      caption={
        <span className="block font-mono text-[0.78rem] text-ink-soft">
          <AnimatePresence mode="wait" initial={false}>
            <motion.span
              key={open ? set.join() : "short"}
              className="block"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={quick}
            >
              {open ? (
                <>
                  S = {"{"}
                  {set.join(", ")}
                  {"}"} · {set.map((m) => `λ${m} = ${lagrange(m, set)}`).join(", ")} ·{" "}
                  <span className="mt-1 block text-olive">
                    {set.map((m) => `λ${m}·D${m}`).join(" + ")} = address_secret · P
                  </span>
                </>
              ) : (
                <>
                  {answering.length} of {T_V} partials. Too few to combine; the lens learns nothing
                  about the secret.
                </>
              )}
            </motion.span>
          </AnimatePresence>
        </span>
      }
    >
      <KeeperTable
        views={POSTS.map((post) => ({
          post,
          state: online.has(post.id) ? "online" : "offline",
          beam: set.includes(post.id),
        }))}
        open={open}
        onToggle={toggle}
        label={`Five keeper posts around the PXE lens. ${answering.length} are lit; ${T_V} beams are needed to read the note.`}
      />
    </FigureCard>
  );
}
