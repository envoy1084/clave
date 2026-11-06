import { Fr } from "@aztec-labs/foundation/curves/bn254";
import { Point } from "@aztec-labs/foundation/curves/grumpkin";
import type { Point as NapiPoint } from "@tecpan/napi";

/** A Grumpkin point in the form `@tecpan/napi` takes. */
export const toNapiPoint = (point: Point): NapiPoint => ({
  x: point.x.toString(),
  y: point.y.toString(),
});

/** A Grumpkin point returned by `@tecpan/napi`. */
export const fromNapiPoint = (point: NapiPoint): Point =>
  new Point(Fr.fromString(point.x), Fr.fromString(point.y));
