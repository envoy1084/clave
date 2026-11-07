import { useMemo } from "react";

import { cn } from "@/lib/cn";

// Generated landscape in the Aztec idiom: fractal noise gradient-mapped from deep blue through cyan and lilac to a rare
// glint of signal green, partly resolved into flat pixels. The noise is an SVG filter, which is expensive to paint, so
// it is rendered once into an <img> (decoded once, then composited like any bitmap) and nothing animates over it.

const W = 728;
const H = 520;
const CELL = 11;
const TABLE = {
  r: "0.03 0.04 0.06 0.09 0.13 0.18 0.25 0.33 0.42 0.53 0.64 0.74 0.82 0.87 0.88 0.80",
  g: "0.05 0.07 0.10 0.15 0.22 0.31 0.43 0.56 0.66 0.69 0.67 0.66 0.68 0.72 0.76 0.96",
  b: "0.14 0.20 0.28 0.38 0.52 0.68 0.82 0.93 0.99 1.00 1.00 1.00 0.98 0.96 0.94 0.10",
};
const PIXELS = ["#C8F500", "#A7A9FF", "#5BABFF", "#E6D5FF", "#0E1530", "#7FD8FF"];

type Shade = "left" | "bottom" | "none";
type Cluster = readonly [number, number, number, number];

function terrainUrl(seed: number, shade: Shade, clusters: readonly Cluster[]): string {
  let s = seed + 7;
  const random = () => (s = (s * 16807) % 2147483647) / 2147483647;
  const rects: string[] = [];
  for (const [cx, cy, w, h] of clusters)
    for (let i = 0; i < w; i++)
      for (let j = 0; j < h; j++) {
        const distance = Math.hypot(i - w / 2, j - h / 2) / (Math.max(w, h) / 2);
        if (random() < 0.25 + distance * 0.55) continue;
        const fill = PIXELS[Math.floor(random() * PIXELS.length)] ?? "#A7A9FF";
        rects.push(
          `<rect x="${cx + i * CELL}" y="${cy + j * CELL}" width="${CELL}" height="${CELL}" fill="${fill}" opacity="${(0.45 + random() * 0.5).toFixed(2)}"/>`,
        );
      }
  const left = shade === "left";
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">
<defs>
<filter id="t" x="0" y="0" width="100%" height="100%"><feTurbulence type="fractalNoise" baseFrequency="0.0062 0.012" numOctaves="6" seed="${seed}"/><feColorMatrix type="saturate" values="0"/>
<feComponentTransfer><feFuncR type="linear" slope="2.7" intercept="-0.86"/><feFuncG type="linear" slope="2.7" intercept="-0.86"/><feFuncB type="linear" slope="2.7" intercept="-0.86"/></feComponentTransfer>
<feComponentTransfer><feFuncR type="table" tableValues="${TABLE.r}"/><feFuncG type="table" tableValues="${TABLE.g}"/><feFuncB type="table" tableValues="${TABLE.b}"/><feFuncA type="linear" slope="0" intercept="1"/></feComponentTransfer></filter>
<filter id="g"><feTurbulence type="fractalNoise" baseFrequency="0.9" numOctaves="2" stitchTiles="stitch"/><feColorMatrix type="saturate" values="0"/><feComponentTransfer><feFuncA type="table" tableValues="0 0.2"/></feComponentTransfer></filter>
<linearGradient id="s" x1="0" y1="0" x2="${left ? 1 : 0}" y2="${left ? 0.3 : 1}"><stop offset="0" stop-color="#0E1530" stop-opacity="${left ? 0.85 : 0}"/><stop offset="0.55" stop-color="#0E1530" stop-opacity="${left ? 0.4 : 0.1}"/><stop offset="1" stop-color="#0E1530" stop-opacity="${shade === "bottom" ? 0.85 : 0}"/></linearGradient>
</defs>
<rect width="${W}" height="${H}" filter="url(#t)"/>${shade === "none" ? "" : `<rect width="${W}" height="${H}" fill="url(#s)"/>`}${rects.join("")}<rect width="${W}" height="${H}" filter="url(#g)"/>
</svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

const DEFAULT_CLUSTERS: readonly Cluster[] = [
  [470, 70, 10, 7],
  [600, 190, 7, 6],
  [430, 250, 5, 4],
];

export function Terrain({
  className,
  seed = 21,
  shade = "left",
  clusters = DEFAULT_CLUSTERS,
  priority = false,
}: {
  className?: string;
  seed?: number;
  shade?: Shade;
  clusters?: readonly Cluster[];
  /** The hero's image: fetched first, never deferred. */
  priority?: boolean;
}) {
  // Clusters arrive as literals; key the cache on their values, not identity.
  const key = JSON.stringify(clusters);
  const src = useMemo(
    () => terrainUrl(seed, shade, JSON.parse(key) as Cluster[]),
    [seed, shade, key],
  );
  return (
    <img
      src={src}
      alt=""
      width={W}
      height={H}
      draggable={false}
      decoding={priority ? "sync" : "async"}
      fetchPriority={priority ? "high" : "auto"}
      className={cn("pointer-events-none size-full object-cover", className)}
    />
  );
}
