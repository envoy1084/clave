import { useId, useMemo } from "react";

import { cn } from "@/lib/utils";

// Generated landscape in the Aztec idiom: fractal noise gradient-mapped from deep blue through cyan and lilac to a rare
// glint of signal green, partly resolved into flat pixels (privacy, rendered). Decorative only.

const TABLE = {
  r: "0.03 0.04 0.06 0.09 0.13 0.18 0.25 0.33 0.42 0.53 0.64 0.74 0.82 0.87 0.88 0.80",
  g: "0.05 0.07 0.10 0.15 0.22 0.31 0.43 0.56 0.66 0.69 0.67 0.66 0.68 0.72 0.76 0.96",
  b: "0.14 0.20 0.28 0.38 0.52 0.68 0.82 0.93 0.99 1.00 1.00 1.00 0.98 0.96 0.94 0.10",
};
const PIXELS = ["#C8F500", "#A7A9FF", "#5BABFF", "#E6D5FF", "#0E1530", "#7FD8FF"];

function mosaic(seed: number, clusters: readonly [number, number, number, number][], cell: number) {
  let s = seed;
  const random = () => (s = (s * 16807) % 2147483647) / 2147483647;
  const out: { x: number; y: number; fill: string; opacity: number; delay: number }[] = [];
  for (const [cx, cy, w, h] of clusters)
    for (let i = 0; i < w; i++)
      for (let j = 0; j < h; j++) {
        const distance = Math.hypot(i - w / 2, j - h / 2) / (Math.max(w, h) / 2);
        if (random() < 0.25 + distance * 0.55) continue;
        out.push({
          x: cx + i * cell,
          y: cy + j * cell,
          fill: PIXELS[Math.floor(random() * PIXELS.length)] ?? "#A7A9FF",
          opacity: 0.45 + random() * 0.5,
          delay: random() * 9,
        });
      }
  return out;
}

export function Terrain({
  className,
  seed = 21,
  shade = "left",
  clusters = [
    [470, 70, 10, 7],
    [600, 190, 7, 6],
    [430, 250, 5, 4],
  ],
}: {
  className?: string;
  seed?: number;
  shade?: "left" | "bottom" | "none";
  clusters?: readonly [number, number, number, number][];
}) {
  const id = useId().replace(/:/gu, "");
  const pixels = useMemo(() => mosaic(seed + 7, clusters, 11), [seed, clusters]);
  return (
    <svg
      className={cn("pointer-events-none size-full", className)}
      viewBox="0 0 728 520"
      preserveAspectRatio="xMidYMid slice"
      aria-hidden
    >
      <defs>
        <filter id={`t${id}`} x="0" y="0" width="100%" height="100%">
          <feTurbulence
            type="fractalNoise"
            baseFrequency="0.0062 0.012"
            numOctaves="7"
            seed={seed}
          />
          <feColorMatrix type="saturate" values="0" />
          <feComponentTransfer>
            <feFuncR type="linear" slope="2.7" intercept="-0.86" />
            <feFuncG type="linear" slope="2.7" intercept="-0.86" />
            <feFuncB type="linear" slope="2.7" intercept="-0.86" />
          </feComponentTransfer>
          <feComponentTransfer>
            <feFuncR type="table" tableValues={TABLE.r} />
            <feFuncG type="table" tableValues={TABLE.g} />
            <feFuncB type="table" tableValues={TABLE.b} />
            <feFuncA type="linear" slope="0" intercept="1" />
          </feComponentTransfer>
        </filter>
        <filter id={`g${id}`}>
          <feTurbulence
            type="fractalNoise"
            baseFrequency="0.9"
            numOctaves="2"
            stitchTiles="stitch"
          />
          <feColorMatrix type="saturate" values="0" />
          <feComponentTransfer>
            <feFuncA type="table" tableValues="0 0.2" />
          </feComponentTransfer>
        </filter>
        <linearGradient
          id={`s${id}`}
          x1="0"
          y1="0"
          x2={shade === "left" ? 1 : 0}
          y2={shade === "left" ? 0.3 : 1}
        >
          <stop offset="0" stopColor="#0E1530" stopOpacity={shade === "left" ? 0.85 : 0} />
          <stop offset="0.55" stopColor="#0E1530" stopOpacity={shade === "left" ? 0.4 : 0.1} />
          <stop offset="1" stopColor="#0E1530" stopOpacity={shade === "bottom" ? 0.85 : 0} />
        </linearGradient>
      </defs>
      <rect width="728" height="520" filter={`url(#t${id})`} />
      {shade !== "none" && <rect width="728" height="520" fill={`url(#s${id})`} />}
      <g>
        {pixels.map((p) => (
          <rect
            key={`${p.x}-${p.y}`}
            x={p.x}
            y={p.y}
            width="11"
            height="11"
            fill={p.fill}
            opacity={p.opacity}
            className="motion-safe:animate-[pixel_9s_ease-in-out_infinite]"
            style={{ animationDelay: `${p.delay}s` }}
          />
        ))}
      </g>
      <rect width="728" height="520" filter={`url(#g${id})`} />
    </svg>
  );
}
