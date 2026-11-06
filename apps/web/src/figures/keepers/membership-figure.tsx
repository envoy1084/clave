import { useRef } from "react";

import { useInView } from "motion/react";

import { FigureCard } from "@/components/iso/figure-card";
import { useSequence } from "@/lib/use-sequence";

import { postsAt } from "./geometry";
import { KeeperTable, type PostView } from "./keeper-table";

// A 2-of-3 committee A, B, C; C leaves and D joins. Post ids: 1 A, 2 B, 3 C, 4 D.
const POSTS = postsAt([168, 228, 288, 348]);
const DELAYS = [900, 1300, 1300, 1200] as const;

const STAGES = [
  {
    readout: "epoch 1 · A B C",
    text: "Epoch 1. A, B and C each hold a share; any two light the note.",
  },
  {
    readout: "rotation proposed",
    text: "The members approve rotate_membership: remove C, add D. At most t_v − 1 leave per rotation.",
  },
  {
    readout: "A, B deal",
    text: "A and B deal sub-shares of the same secret on a fresh polynomial. D checks every dealer before accepting.",
  },
  {
    readout: "commitment on-chain",
    text: "The new committee commitment lands in the config note; keepers switch to epoch 2 together.",
  },
  {
    readout: "epoch 2 · A B D",
    text: "C's old share no longer combines with anything. Same secret, same address, new committee.",
  },
] as const;

function viewsAt(step: number): PostView[] {
  return POSTS.map((post): PostView => {
    if (post.id <= 2) return { post, state: "online", beam: step !== 1 && step !== 3 };
    if (post.id === 3)
      return {
        post,
        state: step >= 3 ? "gone" : step >= 1 ? "offline" : "online",
        beam: step === 0,
        lift: step >= 3 ? 46 : step >= 1 ? 10 : 0,
      };
    return {
      post,
      state: step >= 3 ? "online" : "gone",
      beam: step >= 4,
      lift: step >= 3 ? 0 : 46,
    };
  });
}

export function MembershipFigure({ fig = "Fig. 4" }: { fig?: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const inView = useInView(ref, { once: true, amount: 0.5 });
  const { step, done, replay } = useSequence(DELAYS, inView);
  const stage = STAGES[step] ?? STAGES[0];
  return (
    <div ref={ref}>
      <FigureCard
        fig={fig}
        title="rotate_membership · t_v 2 of 3"
        hint={
          <button
            type="button"
            onClick={replay}
            disabled={!done}
            className="pointer-events-auto rounded-md px-2 py-1 uppercase shadow-[inset_0_0_0_1px_var(--color-line)] transition-opacity duration-150 disabled:opacity-30 hover-fine:bg-paper-2"
          >
            Replay
          </button>
        }
        readout={stage.readout}
        caption={stage.text}
      >
        <KeeperTable
          views={viewsAt(step)}
          open
          label={`Keeper posts A, B, C and D around the lens. ${stage.text}`}
        />
      </FigureCard>
    </div>
  );
}
