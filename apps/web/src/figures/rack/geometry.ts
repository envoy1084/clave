// Fig. 3, the system as a rack: four trays on guide pillars, bottom to top the Aztec network, the relay, the member's
// keeper and the member's device. Each tray carries what runs there: blocks and the vault contract; ciphertext
// mailboxes; the keeper's server with its viewing share; a laptop running the PXE beside the signing key.
import {
  axisVector,
  boxCorners,
  corners,
  cylinder,
  depthOf,
  dotGrid,
  fitProjection,
  insetPlan,
  iso,
  lineOnTop,
  pathOf,
  planOutline,
  ring,
  slabOf,
  topMatrix,
  type Plan,
  type Point3,
  type SolidPaths,
} from "@/components/iso/kit";
import { partsFor } from "@/components/iso/parts";

export const WIDTH = 560;
export const HEIGHT = 820;
const BASE: Plan = { x: -118, y: -86, w: 236, d: 172, r: 10 };
const TRAY: Plan = { x: -104, y: -74, w: 208, d: 148, r: 7 };
const TRAY_H = 5;
export const LEVELS = [16, 112, 208, 304] as const;
const PILLAR_TOP = 376;
export const LIFT = 14;

export const P = fitProjection(
  [...boxCorners(BASE, -14, 0), ...boxCorners(TRAY, 0, PILLAR_TOP + LIFT)],
  WIDTH,
  HEIGHT,
  {
    pad: 20,
  },
);
const at = (point: Point3) => iso(point, P);
const { base } = partsFor(P);

export const BASE_ART = base(BASE, 10, { x: -40, y: BASE.y + BASE.d - 16, w: 60, d: 10, r: 1.5 });

const PILLARS = corners(TRAY, 12).toSorted(
  (a, b) => depthOf([a[0], a[1], 0], P) - depthOf([b[0], b[1], 0], P),
);

/** Pillar pieces between z0 and z1, back to front. */
export const pillarsBetween = (z0: number, z1: number) =>
  PILLARS.map(([x, y]) => cylinder(x, y, 3.4, z0, z1 - z0, P, 20));
export const PILLAR_CAPS = PILLARS.map(([x, y]) => cylinder(x, y, 5, PILLAR_TOP, 3, P, 24, 0.4));
export const PILLAR_FEET = PILLARS.map(([x, y]) => cylinder(x, y, 7, 0, 4, P, 28, 0.4));

export interface TrayArt {
  slab: SolidPaths;
  groove: string;
  bushings: SolidPaths[];
  anchor: [number, number];
  parts: { paths: SolidPaths; accent?: boolean; inner?: string }[];
  lines: string[];
  dots: [number, number][];
  label?: { transform: string; text: string };
}

function tray(z: number): Pick<TrayArt, "slab" | "groove" | "bushings" | "anchor"> {
  return {
    slab: slabOf(TRAY, z, TRAY_H, P, 8, 0.8),
    groove: planOutline(insetPlan(TRAY, 6), z + TRAY_H, P),
    bushings: PILLARS.map(([x, y]) => cylinder(x, y, 7, z - 2, TRAY_H + 6, P, 28, 0.4)),
    anchor: at([TRAY.x + TRAY.w, TRAY.y + TRAY.d, z + TRAY_H]),
  };
}

const box = (plan: Plan, z: number, h: number, bevel = 0.4) => slabOf(plan, z, h, P, 4, bevel);
const lines = (rows: [number, number, number, number][], z: number) =>
  rows.map(([x0, y0, x1, y1]) => lineOnTop([x0, y0], [x1, y1], z, P)).join("");

// 0 · Aztec network: a lattice of blocks and, on its own pad, the vault contract.
function network(): TrayArt {
  const z = LEVELS[0] + TRAY_H;
  const parts: TrayArt["parts"] = [];
  const order: [number, number][] = [];
  for (let i = 0; i < 4; i++) for (let j = 0; j < 3; j++) order.push([-86 + i * 26, -14 + j * 26]);
  order.sort((a, b) => a[0] + a[1] - (b[0] + b[1]));
  for (const [x, y] of order) parts.push({ paths: box({ x, y, w: 20, d: 20, r: 2 }, z, 7) });
  const chip: Plan = { x: 26, y: 2, w: 56, d: 50, r: 4 };
  parts.push({ paths: box(insetPlan(chip, -4), z, 2) });
  parts.push({ paths: box(chip, z + 2, 6, 0.6), accent: true });
  return {
    ...tray(LEVELS[0]),
    parts,
    lines: [planOutline(insetPlan(chip, 5), z + 8, P)],
    dots: dotGrid(chip.x + 9, chip.y + 9, 6, 5, 8, z + 8, P),
    label: {
      transform: topMatrix([chip.x + 8, chip.y + chip.d - 4], z + 8, P),
      text: "ClaveVault",
    },
  };
}

// 1 · Relay: a row of mailboxes, each a box with a slot; it stores envelopes it cannot open.
function relay(): TrayArt {
  const z = LEVELS[1] + TRAY_H;
  const parts: TrayArt["parts"] = [];
  const slots: [number, number, number, number][] = [];
  const xs = [-86, -50, -14, 22, 58];
  for (const x of xs) {
    parts.push({ paths: box({ x, y: -6, w: 28, d: 22, r: 2 }, z, 16, 0.6), accent: true });
    slots.push([x + 6, 5, x + 22, 5]);
  }
  const envelopes: [number, number][] = [
    [-70, 38],
    [-30, 46],
    [18, 38],
  ];
  for (const [x, y] of envelopes)
    parts.push({ paths: box({ x, y, w: 26, d: 16, r: 1 }, z, 1.2, 0) });
  return {
    ...tray(LEVELS[1]),
    parts,
    lines: [
      lines(slots, z + 16),
      ...envelopes.map(([x, y]) =>
        pathOf([at([x, y, z + 1.2]), at([x + 13, y + 8, z + 1.2]), at([x + 26, y, z + 1.2])]),
      ),
    ],
    dots: [],
  };
}

// 2 · Keeper: the member's server with vents, status lights and, on top, the chip holding the viewing share.
function keeper(): TrayArt {
  const z = LEVELS[2] + TRAY_H;
  const server: Plan = { x: -74, y: -16, w: 116, d: 70, r: 4 };
  const chip: Plan = { x: -10, y: 0, w: 30, d: 30, r: 2 };
  const vents: [number, number, number, number][] = [];
  for (let k = 0; k < 7; k++)
    vents.push([server.x + 10, server.y + 10 + k * 7, server.x + 52, server.y + 10 + k * 7]);
  return {
    ...tray(LEVELS[2]),
    parts: [
      { paths: box(server, z, 20, 0.8) },
      { paths: box(insetPlan(chip, -3), z + 20, 1.4) },
      { paths: box(chip, z + 21.4, 4, 0.4), accent: true },
      { paths: box({ x: 56, y: 10, w: 26, d: 40, r: 3 }, z, 10, 0.4) },
    ],
    lines: [
      lines(vents, z + 20),
      ring(chip.x + chip.w / 2, chip.y + chip.d / 2, 8, z + 25.4, P, 28),
    ],
    dots: [
      ...[0, 1, 2].map((k) => at([server.x + server.w - 6, server.y + 14 + k * 8, z + 20])),
      ...dotGrid(62, 16, 3, 5, 7, z + 10, P),
    ],
    label: {
      transform: topMatrix([server.x + 8, server.y + server.d - 6], z + 20, P),
      text: "clave-keeper",
    },
  };
}

// 3 · Member device: a laptop running the PXE, and the signing key that never leaves it.
function device(): TrayArt {
  const z = LEVELS[3] + TRAY_H;
  const deck: Plan = { x: -80, y: -40, w: 110, d: 76, r: 4 };
  const screenY = deck.y;
  const screen = box({ x: deck.x + 2, y: screenY - 3, w: deck.w - 4, d: 3, r: 1 }, z + 3, 62, 0.3);
  const codeLines = [0, 1, 2, 3, 4, 5].map((k) => {
    const zz = z + 54 - k * 7.5;
    const len = [70, 52, 84, 40, 62, 30][k] ?? 40;
    return pathOf([at([deck.x + 12, screenY, zz]), at([deck.x + 12 + len, screenY, zz])]);
  });
  const fob: Plan = { x: 52, y: -6, w: 26, d: 40, r: 9 };
  return {
    ...tray(LEVELS[3]),
    parts: [
      { paths: box(deck, z, 3, 0.6) },
      { paths: screen },
      { paths: box(insetPlan(fob, -2), z, 1.2) },
      { paths: box(fob, z + 1.2, 7, 0.6), accent: true },
    ],
    lines: [
      planOutline({ x: deck.x + 40, y: deck.y + 50, w: 32, d: 20, r: 2 }, z + 3, P),
      ...codeLines,
      ring(fob.x + fob.w / 2, fob.y + 9, 4, z + 8.2, P, 24),
    ],
    dots: dotGrid(deck.x + 10, deck.y + 10, 12, 4, 8, z + 3, P),
  };
}

export const TRAYS: TrayArt[] = [network(), relay(), keeper(), device()];
export const liftOffset = axisVector(P, [0, 0, LIFT]);
export const PILLAR_RUNS = [
  pillarsBetween(4, LEVELS[0]),
  pillarsBetween(LEVELS[0] + TRAY_H, LEVELS[1]),
  pillarsBetween(LEVELS[1] + TRAY_H, LEVELS[2]),
  pillarsBetween(LEVELS[2] + TRAY_H, LEVELS[3]),
  pillarsBetween(LEVELS[3] + TRAY_H, PILLAR_TOP),
];
