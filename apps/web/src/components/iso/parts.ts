// Recipes shared by every Clave figure, bound to one projection: a base plate on feet with its groove, corner screws
// and label plate, plus the small helpers each geometry module needs. Geometry is built once at module load.
import {
  corners,
  cylinder,
  depthOf,
  haloOf,
  insetPlan,
  iso,
  lineOnTop,
  planOutline,
  ring,
  slabOf,
  type Flat,
  type Plan,
  type Plane,
  type Point3,
  type Projection,
  type SolidPaths,
} from "./kit";

export interface BaseArt {
  halo: string;
  feet: SolidPaths[][];
  slab: SolidPaths;
  groove: string;
  screws: Flat[];
  screwRings: string;
  plate: SolidPaths;
  plateLine: string;
  plateRule: string;
  plateScrews: Flat[];
}

export function partsFor(P: Projection) {
  const at = (point: Point3) => iso(point, P);
  const byDepth = <T extends { at: Point3 }>(items: T[]) =>
    items.toSorted((a, b) => depthOf(a.at, P) - depthOf(b.at, P));

  /** A plate `h` thick whose top is at z = 0, on four feet, with a groove, screws and a label plate at `label`. */
  function base(plan: Plan, h: number, label: Plan): BaseArt {
    const foot = 4;
    const feet = corners(plan, 20)
      .map(([x, y]) => ({ at: [x, y, -h - foot] as Point3 }))
      .toSorted((a, b) => depthOf(a.at, P) - depthOf(b.at, P))
      .map(({ at: [x, y] }) => [
        cylinder(x, y, 7, -h - foot, 1.4, P, 28),
        cylinder(x, y, 5.2, -h - foot + 1.4, foot - 1.4, P, 28),
      ]);
    const screws = corners(plan, 9);
    return {
      halo: haloOf(plan, -h - foot, h, P),
      feet,
      slab: slabOf(plan, -h, h, P, 8, 1.4),
      groove: planOutline(insetPlan(plan, 5), 0, P),
      screws: screws.map(([x, y]) => at([x, y, 0])),
      screwRings: screws.map(([x, y]) => ring(x, y, 2.2, 0, P, 16)).join(""),
      plate: slabOf(label, 0, 0.8, P, 4),
      plateLine: planOutline(insetPlan(label, 1.4), 0.8, P),
      plateRule: [0, 1, 2]
        .map((row) =>
          lineOnTop(
            [label.x + 7, label.y + 3 + row * 2],
            [label.x + (row === 1 ? label.w * 0.55 : label.w * 0.78), label.y + 3 + row * 2],
            0.8,
            P,
          ),
        )
        .join(""),
      plateScrews: [
        at([label.x + 3, label.y + label.d / 2, 0.8]),
        at([label.x + label.w - 3, label.y + label.d / 2, 0.8]),
      ],
    };
  }

  return { at, byDepth, base };
}

export type { Flat, Plan, Plane, Point3 };
