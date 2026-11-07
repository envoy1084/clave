import type { Point } from "@aztec-labs/foundation/curves/grumpkin";
import type { AztecAddress } from "@aztec-labs/stdlib/aztec-address";
import { type MemberPartials, type Point as NapiPoint, verifyAndCombine } from "@clave/napi";

import type { EcdhProvider, EcdhPurpose } from "./ecdh.ts";
import { fromNapiPoint, toNapiPoint } from "./points.ts";

/** Public data of a vault's viewing committee for one epoch. */
export interface ViewingCommittee {
  readonly vault: AztecAddress;
  /** Committee epoch: the vault config version the shares belong to. */
  readonly epoch: number;
  readonly preaddress: string;
  /** Group incoming viewing key `Ivpk`. */
  readonly groupKey: NapiPoint;
  /** Verification shares `Y_i` in member order. */
  readonly verificationShares: readonly NapiPoint[];
  /** Viewing threshold `t_v`. */
  readonly threshold: number;
}

/** One threshold ECDH request, as sent to members' keepers. */
export interface PartialsRequest {
  readonly vault: AztecAddress;
  readonly epoch: number;
  readonly purpose: EcdhPurpose;
  readonly points: readonly NapiPoint[];
}

/** Where partials come from: members' keepers (Phase 8) or in-process shares (tests). */
export interface PartialsSource {
  /** Responses from at least `t_v` members, each with one partial per point in request order. */
  requestPartials(request: PartialsRequest): Promise<MemberPartials[]>;
}

/**
 * {@link EcdhProvider} for a threshold account: asks a {@link PartialsSource} for partials, verifies every DLEQ proof
 * and combines `t_v` valid partials per point (in Rust, through `@clave/napi`). Members whose partials fail are
 * reported to `onInvalidMembers` and ignored.
 */
export class ThresholdEcdhProvider implements EcdhProvider {
  readonly #committee: ViewingCommittee;
  readonly #source: PartialsSource;
  readonly #onInvalidMembers: ((members: number[]) => void) | undefined;

  constructor(
    committee: ViewingCommittee,
    source: PartialsSource,
    onInvalidMembers?: (members: number[]) => void,
  ) {
    this.#committee = committee;
    this.#source = source;
    this.#onInvalidMembers = onInvalidMembers;
  }

  async mul(points: readonly Point[], purpose: EcdhPurpose): Promise<Point[]> {
    const inputs = points.map(toNapiPoint);
    const { vault, epoch, preaddress, groupKey, verificationShares, threshold } = this.#committee;
    const responses = await this.#source.requestPartials({ vault, epoch, purpose, points: inputs });
    const result = verifyAndCombine({
      epoch,
      preaddress,
      ivpk: groupKey,
      verificationShares: [...verificationShares],
      threshold,
      inputs,
      responses,
    });
    if (result.invalidMembers.length > 0) this.#onInvalidMembers?.(result.invalidMembers);
    return result.points.map(fromNapiPoint);
  }
}
