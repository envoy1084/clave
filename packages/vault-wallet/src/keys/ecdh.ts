import type { Point } from "@aztec-labs/foundation/curves/grumpkin";

/** Why `address_secret · P` is requested; recorded by keepers for audit and rate limits. */
export type EcdhPurpose = "decryption" | "tagging";

/** Computes `address_secret · P` for one threshold account. */
export interface EcdhProvider {
  /**
   * Multiplies every point by the account's address secret. Points are on the curve and not the identity; the result
   * has one entry per input, in order.
   */
  mul(points: readonly Point[], purpose: EcdhPurpose): Promise<Point[]>;
}
