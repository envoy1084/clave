// The keeper table, shared by the viewing and membership figures. A round table carries keeper posts; each post is a
// lamp whose share stays inside it. A lit lamp throws its partial (D_i = x_i · P) onto the PXE's lens at the centre,
// where a sealed note lies. The note resolves only when t_v beams land: the lens combines light, never shares.
import {
  circleRing,
  cylinder,
  depthOf,
  fitProjection,
  haloOf,
  iso,
  knurl,
  onTop,
  pathOf,
  radialTicks,
  ring,
  sideArc,
  type Flat,
  type Point3,
  type Projection,
} from "@/components/iso/kit";

export const WIDTH = 640;
export const HEIGHT = 460;
const TABLE_R = 122;
const TABLE_H = 12;
export const POST_RING = 90;
const LENS_Z = 47;
const LENS_R = 22;
const HEAD_TOP = 68;

export const P: Projection = fitProjection(
  [
    ...circleRing(0, 0, TABLE_R, 24).map(([x, y]) => [x, y, -TABLE_H - 4] as Point3),
    ...circleRing(0, 0, POST_RING, 24).map(([x, y]) => [x, y, HEAD_TOP + 6] as Point3),
  ],
  WIDTH,
  HEIGHT,
  { pad: 24 },
);
const isoOf = (point: Point3) => iso(point, P);

const FEET = [45, 135, 225, 315].map((deg) => {
  const a = (deg * Math.PI) / 180;
  return [Math.cos(a) * 84, Math.sin(a) * 84] as const;
});

export const TABLE_ART = {
  halo: haloOf(
    { x: -TABLE_R, y: -TABLE_R, w: TABLE_R * 2, d: TABLE_R * 2, r: TABLE_R },
    -TABLE_H - 4,
    TABLE_H,
    P,
  ),
  feet: FEET.map(([x, y]) => cylinder(x, y, 9, -TABLE_H - 4, 4, P, 28)),
  top: cylinder(0, 0, TABLE_R, -TABLE_H, TABLE_H, P, 120, 1.4),
  band: sideArc(0, 0, TABLE_R, -TABLE_H / 2, P, 64),
  groove: ring(0, 0, TABLE_R - 6, 0, P, 120),
  inner: ring(0, 0, 56, 0, P, 90),
  ticks: radialTicks(0, 0, TABLE_R - 7, 72, 6, 0, [2.4, 5], P),
};

export interface Post {
  id: number;
  x: number;
  y: number;
  angle: number;
}

/** Posts at `angles` (degrees, 0 = +x) on the post ring. */
export function postsAt(angles: readonly number[]): Post[] {
  return angles.map((deg, i) => {
    const angle = (deg * Math.PI) / 180;
    return { id: i + 1, angle, x: Math.cos(angle) * POST_RING, y: Math.sin(angle) * POST_RING };
  });
}

export function postArt({ x, y, angle }: Post) {
  const screws: Point3[] = [0, 1, 2].map((k) => {
    const a = angle + (k * Math.PI * 2) / 3 + 0.5;
    return [x + Math.cos(a) * 9.5, y + Math.sin(a) * 9.5, 2.4];
  });
  return {
    foot: cylinder(x, y, 13, 0, 2.4, P, 40, 0.5),
    footScrews: screws.map(isoOf),
    column: cylinder(x, y, 4.6, 2.4, 52, P, 24),
    collar: cylinder(x, y, 7, 24, 5, P, 32, 0.4),
    collarKnurl: knurl(x, y, 7, 24, 29, 28, P),
    neck: cylinder(x, y, 6, 52, 3, P, 28),
    head: cylinder(x, y, 11, 55, 11, P, 40, 0.6),
    headSeam: sideArc(x, y, 11, 59, P, 28),
    lens: cylinder(x, y, 7.2, 66, 1.6, P, 36),
    lensRing: ring(x, y, 4.4, HEAD_TOP - 0.4, P, 28),
    top: [x, y, HEAD_TOP] as Point3,
  };
}

export const PEDESTAL_ART = {
  foot: cylinder(0, 0, 28, 0, 5, P, 72, 0.8),
  footRing: ring(0, 0, 22, 5, P, 64),
  column: cylinder(0, 0, 10, 5, LENS_Z - 12, P, 40),
  yoke: cylinder(0, 0, 15, LENS_Z - 12, 5, P, 48, 0.4),
  housing: cylinder(0, 0, LENS_R + 4, LENS_Z - 7, 7, P, 72, 0.8),
  housingKnurl: knurl(0, 0, LENS_R + 4, LENS_Z - 7, LENS_Z, 64, P),
  glass: cylinder(0, 0, LENS_R, LENS_Z, 0.8, P, 64),
};

// The note on the lens: a card whose face is a pixel mosaic until it can be read, then rule lines.
const CARD = { x: -13, y: -9, w: 26, d: 18 };
const CARD_Z = LENS_Z + 1.4;
const corner = (x: number, y: number): Flat => isoOf([x, y, CARD_Z]);
export const NOTE_ART = (() => {
  const card = pathOf(
    [
      corner(CARD.x, CARD.y),
      corner(CARD.x + CARD.w, CARD.y),
      corner(CARD.x + CARD.w, CARD.y + CARD.d),
      corner(CARD.x, CARD.y + CARD.d),
    ],
    true,
  );
  const cells: { d: string; fill: string; opacity: number }[] = [];
  const colors = ["#15180F", "#3b2f7a", "#A7A9FF", "#5BABFF", "#C8F500", "#2b2f4a"];
  let seed = 11;
  const random = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
  const size = 2.6;
  for (let i = 0; i < 8; i++)
    for (let j = 0; j < 5; j++) {
      const x = CARD.x + 2.6 + i * size;
      const y = CARD.y + 2.2 + j * size;
      cells.push({
        d: pathOf(
          [corner(x, y), corner(x + size, y), corner(x + size, y + size), corner(x, y + size)],
          true,
        ),
        fill: colors[Math.floor(random() * colors.length)] ?? "#15180F",
        opacity: 0.55 + random() * 0.45,
      });
    }
  const rules = [0, 1, 2, 3].map((row) =>
    onTop(
      [
        [CARD.x + 3, CARD.y + 3.6 + row * 3.6],
        [CARD.x + (row === 0 ? 14 : row === 3 ? 17 : 22), CARD.y + 3.6 + row * 3.6],
      ],
      CARD_Z,
      P,
    ),
  );
  return { card, cells, rules };
})();

/** The point on the lens rim facing a post, where its beam lands. */
export function landingOf(post: Post): Point3 {
  return [Math.cos(post.angle) * (LENS_R - 3), Math.sin(post.angle) * (LENS_R - 3), LENS_Z + 0.8];
}

export const depthAt = (point: Point3) => depthOf(point, P);
export const PEDESTAL_DEPTH = depthOf([0, 0, 0], P);
export { isoOf };
