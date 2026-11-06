// Fig. 1, the k-of-n lock: a strongbox whose lid carries the eight signer slots of the auth witness (MAX_SIGNERS).
// Five slots hold lock cylinders (a 3-of-5 vault), three are blank caps that the circuit still checks against the
// generator. A key turned in a cylinder is one valid approval; a rider on the lid's ruler counts them, and the bolt on
// the side withdraws from its strike only when the count reaches the threshold. One world unit is about 1 px.
import {
  axisVector,
  boxCorners,
  cylinder,
  fitProjection,
  insetPlan,
  knurl,
  lineOnTop,
  pathOf,
  planOutline,
  ring,
  sideMatrix,
  sideSeam,
  slabOf,
  topTicks,
  type Flat,
  type Plan,
  type Point3,
} from "@/components/iso/kit";
import { partsFor } from "@/components/iso/parts";

export const WIDTH = 640;
export const HEIGHT = 440;
export const MAX_SIGNERS = 8;
export const SIGNERS = 5;
export const THRESHOLD = 3;

const BASE: Plan = { x: -140, y: -88, w: 272, d: 168, r: 10 };
const BOX: Plan = { x: -116, y: -66, w: 180, d: 116, r: 7 };
const PLINTH: Plan = insetPlan(BOX, -3);
const PLINTH_H = 6;
const TOP = 60;
const LID_SEAM = 50;
const STRIKE: Plan = { x: 92, y: -14, w: 22, d: 28, r: 3 };
const BOLT_Y = -6;
const BOLT_Z = 24;
const BOLT_H = 9;
const BOLT_D = 12;
const BOX_FACE = BOX.x + BOX.w;

export const P = fitProjection(
  [...boxCorners(BASE, -14, 0), ...boxCorners(BOX, 0, TOP + 22), [STRIKE.x + STRIKE.w, 0, 40]],
  WIDTH,
  HEIGHT,
  { pad: 26 },
);
const { at, base } = partsFor(P);

export const BASE_ART = base(BASE, 10, { x: -70, y: BASE.y + BASE.d - 16, w: 62, d: 10, r: 1.5 });

// The box: plinth, body with its lid band, rivets along the visible faces, and the engraved name on the front.
const rivets: Flat[] = [];
for (let x = BOX.x + 14; x < BOX_FACE - 8; x += 16)
  rivets.push(at([x, BOX.y + BOX.d, LID_SEAM - 4]));
for (let y = BOX.y + 14; y < BOX.y + BOX.d - 8; y += 16)
  rivets.push(at([BOX_FACE, y, LID_SEAM - 4]));

export const BOX_ART = {
  plinth: slabOf(PLINTH, 0, PLINTH_H, P, 8, 0.8),
  body: slabOf(BOX, PLINTH_H, TOP - PLINTH_H, P, 8, 1.2),
  lidSeam: sideSeam(BOX, LID_SEAM, P, 8),
  rivets,
  lidGroove: planOutline(insetPlan(BOX, 6), TOP, P),
  // Guards on the three corners the viewer sees, standing proud of the body between plinth and lid band.
  guards: (
    [
      [BOX.x, BOX.y + BOX.d],
      [BOX_FACE, BOX.y + BOX.d],
      [BOX_FACE, BOX.y],
    ] as const
  ).map(([x, y]) =>
    slabOf(
      { x: x - 6, y: y - 6, w: 12, d: 12, r: 4 },
      PLINTH_H,
      LID_SEAM - PLINTH_H - 3,
      P,
      6,
      0.4,
    ),
  ),
  name: sideMatrix([BOX.x + 14, BOX.y + BOX.d], 30, P, "left"),
};

// The eight slots, two rows of four, in slot order. The ruler runs along the front of the lid.
export interface Slot {
  index: number;
  x: number;
  y: number;
  occupied: boolean;
}
const COLUMNS = [-90, -54, -18, 18];
const ROWS = [-40, -6];
export const SLOTS: Slot[] = Array.from({ length: MAX_SIGNERS }, (_, index) => ({
  index,
  x: COLUMNS[index % 4] ?? 0,
  y: ROWS[Math.floor(index / 4)] ?? 0,
  occupied: index < SIGNERS,
}));

const BEZEL_R = 11;
const PLUG_R = 6.8;
export const SLOT_ART = SLOTS.map((slot) => {
  const { x, y } = slot;
  if (!slot.occupied)
    return {
      slot,
      cap: cylinder(x, y, BEZEL_R - 1, TOP, 0.9, P, 40),
      capRing: ring(x, y, BEZEL_R - 4, TOP + 0.9, P, 32),
      center: at([x, y, TOP + 0.9]),
    };
  return {
    slot,
    bezel: cylinder(x, y, BEZEL_R, TOP, 2.6, P, 44, 0.6),
    knurl: knurl(x, y, BEZEL_R, TOP + 0.3, TOP + 2.3, 36, P),
    plug: cylinder(x, y, PLUG_R, TOP + 2.6, 1.4, P, 36),
    pins: ring(x, y, PLUG_R - 2.2, TOP + 4, P, 24),
    // The keyway runs across the plug: along y at rest, along x once a key has turned it.
    keyway: lineOnTop([x, y - PLUG_R + 1.6], [x, y + PLUG_R - 1.6], TOP + 4, P),
    turned: lineOnTop([x - PLUG_R + 1.6, y], [x + PLUG_R - 1.6, y], TOP + 4, P),
    // The key, already turned: the blade sits in the plug, the bow stands above it with its hole.
    bow: slabOf({ x: x - 8.5, y: y - 1.4, w: 17, d: 2.8, r: 1.4 }, TOP + 7, 16, P, 4, 0.4),
    shank: slabOf({ x: x - 1.6, y: y - 1.1, w: 3.2, d: 2.2, r: 0.6 }, TOP + 4, 3, P, 2),
    hole: pathOf(
      Array.from({ length: 18 }, (_, step) => {
        const angle = (step / 18) * Math.PI * 2;
        return at([x + 3.6 * Math.cos(angle), y + 1.4, TOP + 18 + 3.2 * Math.sin(angle)]);
      }),
      true,
    ),
  };
});

// Approval ruler: a strip along the lid front, one major tick per slot count (0 to 8), the threshold mark at 3.
const RULER: Plan = { x: -104, y: 22, w: 148, d: 10, r: 1.2 };
const RULE_FROM = RULER.x + 6;
export const RULE_STEP = (RULER.w - 12) / MAX_SIGNERS;
const ticks = topTicks(0, RULER.w - 12, RULE_STEP / 4, 4, RULER.y + 1, TOP + 0.6, [2, 4], P);
export const RULER_ART = {
  strip: slabOf(RULER, TOP, 0.6, P, 3),
  // Ticks are generated from 0; shift them to the strip's start by drawing them in a translated group.
  ticks,
  ticksAt: axisVector(P, [RULE_FROM, 0, 0]),
  threshold: slabOf(
    {
      x: RULE_FROM + THRESHOLD * RULE_STEP - 1.4,
      y: RULER.y + RULER.d - 3.2,
      w: 2.8,
      d: 2.6,
      r: 0.6,
    },
    TOP + 0.6,
    1.4,
    P,
    2,
  ),
  // The rider at zero; it moves by RULE_STEP per approval.
  rider: slabOf(
    { x: RULE_FROM - 4, y: RULER.y - 1.5, w: 8, d: 7, r: 1.4 },
    TOP + 0.6,
    3.4,
    P,
    3,
    0.4,
  ),
  riderMark: lineOnTop([RULE_FROM, RULER.y - 1.5], [RULE_FROM, RULER.y + 1.6], TOP + 4, P),
};
export const riderStep = axisVector(P, [RULE_STEP, 0, 0]);

// The strike on the base: a block with a bore facing the box. The bolt runs out of the box face into it.
export const STRIKE_ART = {
  pad: slabOf(insetPlan(STRIKE, -3), 0, 2, P, 4),
  block: slabOf(STRIKE, 2, 34, P, 6, 0.8),
  screws: [
    at([STRIKE.x + 5, STRIKE.y + 5, 36]),
    at([STRIKE.x + STRIKE.w - 5, STRIKE.y + STRIKE.d - 5, 36]),
  ],
  seam: sideSeam(STRIKE, 14, P, 6),
};
export const BOLT_REACH = STRIKE.x + 10 - BOX_FACE;

/** The bolt out of the box face, `out` units long (0 when withdrawn). Rebuilt while it moves. */
export function boltOf(out: number) {
  const length = Math.max(1.2, out);
  return {
    bolt: slabOf(
      { x: BOX_FACE - 0.5, y: BOLT_Y, w: length + 0.5, d: BOLT_D, r: 1.6 },
      BOLT_Z,
      BOLT_H,
      P,
      4,
      0.4,
    ),
    guide: slabOf(
      { x: BOX_FACE, y: BOLT_Y - 4, w: 3, d: BOLT_D + 8, r: 1 },
      BOLT_Z - 4,
      BOLT_H + 8,
      P,
      3,
    ),
  };
}

export const slotCenter = (slot: Slot): Point3 => [slot.x, slot.y, TOP + 4];
export { at };
