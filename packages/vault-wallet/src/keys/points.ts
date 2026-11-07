import { Fr } from "@aztec-labs/foundation/curves/bn254";
import { Point } from "@aztec-labs/foundation/curves/grumpkin";
import type { Point as NapiPoint } from "@clave/napi";

/** A Grumpkin point in the form `@clave/napi` takes. */
export const toNapiPoint = (point: Point): NapiPoint => ({
  x: point.x.toString(),
  y: point.y.toString(),
});

/** A Grumpkin point returned by `@clave/napi`. */
export const fromNapiPoint = (point: NapiPoint): Point =>
  new Point(Fr.fromString(point.x), Fr.fromString(point.y));
