import type { KeyboardEvent, ReactNode } from "react";

import { AnimatePresence, motion } from "motion/react";

import { Dots, Line, Solid } from "@/components/iso/draw";
import { EASE_IN_OUT, EASE_OUT } from "@/lib/motion";

import {
  HEIGHT,
  NOTE_ART,
  PEDESTAL_ART,
  PEDESTAL_DEPTH,
  TABLE_ART,
  WIDTH,
  depthAt,
  isoOf,
  landingOf,
  postArt,
  type Post,
} from "./geometry";

export type PostState = "online" | "offline" | "gone";

export interface PostView {
  post: Post;
  state: PostState;
  beam: boolean;
  /** Lifted posts float above the table (leaving or arriving during a reshare). */
  lift?: number;
}

function PostSolid({
  view,
  onToggle,
}: {
  view: PostView;
  onToggle?: ((id: number) => void) | undefined;
}) {
  const art = postArt(view.post);
  const online = view.state === "online";
  const [hx, hy] = isoOf(art.top);
  const onKey = (event: KeyboardEvent) => {
    if (!onToggle || (event.key !== "Enter" && event.key !== " ")) return;
    event.preventDefault();
    onToggle(view.post.id);
  };
  return (
    <motion.g
      initial={false}
      animate={{ opacity: view.state === "gone" ? 0 : 1, y: -(view.lift ?? 0) }}
      transition={{ duration: 0.6, ease: EASE_IN_OUT }}
    >
      <Solid paths={art.foot} tone="mid" />
      <Dots points={art.footScrews} size={0.5} tone="mid" />
      <Solid paths={art.column} tone="mid" />
      <Solid paths={art.collar} tone="mid" inner={<Line d={art.collarKnurl} tone="faint" />} />
      <Solid paths={art.neck} tone="mid" />
      <Solid
        paths={art.head}
        tone={online ? "hi" : "mid"}
        inner={<Line d={art.headSeam} tone="lo" />}
      />
      <Solid paths={art.lens} tone="hi" className={online ? "accent-signal" : undefined} />
      <Line d={art.lensRing} tone={online ? "hi" : "lo"} />
      {onToggle && (
        <ellipse
          cx={hx}
          cy={hy + 24}
          rx={16}
          ry={36}
          fill="transparent"
          role="button"
          tabIndex={0}
          aria-pressed={online}
          aria-label={`Keeper ${view.post.id}: ${online ? "online" : "offline"}`}
          onClick={() => onToggle(view.post.id)}
          onKeyDown={onKey}
          className="cursor-pointer outline-none focus-visible:stroke-olive focus-visible:stroke-[1.5]"
        />
      )}
    </motion.g>
  );
}

function Beam({ post }: { post: Post }) {
  const from = isoOf(postArt(post).top);
  const to = isoOf(landingOf(post));
  return (
    <motion.g
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={{ duration: 0.25 }}
    >
      <motion.path
        d={`M${from[0]} ${from[1]}L${to[0]} ${to[1]}`}
        className="iso-line"
        data-tone="signal"
        style={{ strokeWidth: 1.4, vectorEffect: "none" }}
        initial={{ pathLength: 0 }}
        animate={{ pathLength: 1 }}
        transition={{ duration: 0.5, ease: EASE_OUT }}
      />
      <circle cx={to[0]} cy={to[1]} r={1.6} fill="#c8f500" stroke="#7f9b00" strokeWidth={0.6} />
    </motion.g>
  );
}

/** The table, its posts in depth order around the PXE pedestal, the beams that are lit, and the note on the lens. */
export function KeeperTable({
  views,
  open,
  label,
  onToggle,
  children,
}: {
  views: PostView[];
  open: boolean;
  label: string;
  onToggle?: (id: number) => void;
  children?: ReactNode;
}) {
  const sorted = views.toSorted(
    (a, b) => depthAt([a.post.x, a.post.y, 0]) - depthAt([b.post.x, b.post.y, 0]),
  );
  const back = sorted.filter((v) => depthAt([v.post.x, v.post.y, 0]) < PEDESTAL_DEPTH);
  const front = sorted.filter((v) => depthAt([v.post.x, v.post.y, 0]) >= PEDESTAL_DEPTH);
  return (
    <svg className="iso-svg" viewBox={`0 0 ${WIDTH} ${HEIGHT}`} role="group" aria-label={label}>
      <path className="iso-halo" d={TABLE_ART.halo} />
      {TABLE_ART.feet.map((paths, i) => (
        <Solid key={i} paths={paths} tone="lo" />
      ))}
      <Solid paths={TABLE_ART.top} tone="mid" inner={<Line d={TABLE_ART.band} tone="faint" />} />
      <Line d={TABLE_ART.groove} tone="lo" />
      <Line d={TABLE_ART.ticks.minor} tone="lo" />
      <Line d={TABLE_ART.ticks.major} tone="mid" />
      <Line d={TABLE_ART.inner} tone="faint" />
      {children}

      {back.map((view) => (
        <PostSolid key={view.post.id} view={view} onToggle={onToggle} />
      ))}

      <Solid paths={PEDESTAL_ART.foot} tone="mid" />
      <Line d={PEDESTAL_ART.footRing} tone="faint" />
      <Solid paths={PEDESTAL_ART.column} tone="mid" />
      <Solid paths={PEDESTAL_ART.yoke} tone="mid" />
      <Solid
        paths={PEDESTAL_ART.housing}
        tone="hi"
        inner={<Line d={PEDESTAL_ART.housingKnurl} tone="faint" />}
      />
      <Solid paths={PEDESTAL_ART.glass} tone="mid" className="accent-peri" crease="none" />
      <path d={NOTE_ART.card} fill="#fffdf8" stroke="#15180f" strokeWidth={0.6} />
      <g>
        {NOTE_ART.cells.map((cell, i) => (
          <motion.path
            key={i}
            d={cell.d}
            fill={cell.fill}
            initial={false}
            animate={{ opacity: open ? 0 : cell.opacity }}
            transition={{ duration: 0.3, delay: open ? (i % 7) * 0.035 : 0, ease: EASE_OUT }}
          />
        ))}
      </g>
      <motion.g
        initial={false}
        animate={{ opacity: open ? 1 : 0 }}
        transition={{ duration: 0.3, delay: open ? 0.25 : 0 }}
      >
        {NOTE_ART.rules.map((d, i) => (
          <Line key={i} d={d} tone="hi" free />
        ))}
      </motion.g>

      <AnimatePresence>
        {views
          .filter((v) => v.beam && v.state === "online")
          .map((v) => (
            <Beam key={v.post.id} post={v.post} />
          ))}
      </AnimatePresence>

      {front.map((view) => (
        <PostSolid key={view.post.id} view={view} onToggle={onToggle} />
      ))}
    </svg>
  );
}
