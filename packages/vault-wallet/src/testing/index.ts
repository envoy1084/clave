import { Fr } from "@aztec-labs/foundation/curves/bn254";
/**
 * Test helpers that deal and use viewing shares in-process (through `@tecpan/napi/testing`). Never used by the app:
 * in production, shares exist only in members' keepers.
 */
import { GrumpkinScalar, type Point } from "@aztec-labs/foundation/curves/grumpkin";
import type { MemberPartials } from "@tecpan/napi";
import { type DealtCommittee, testingDealShares, testingPartials } from "@tecpan/napi/testing";

import { fromNapiPoint } from "../keys/points.ts";
import type { PartialsRequest, PartialsSource } from "../keys/threshold-ecdh.ts";

const seed = () => Buffer.from(Fr.random().toBuffer());

/** A trusted-dealer viewing committee (tests only; production committees come from the DKG). */
export interface TestCommittee {
  readonly groupKey: Point;
  readonly verificationShares: Point[];
  readonly dealt: DealtCommittee;
}

/** Deals a `threshold`-of-`members` sharing of `secret` (random if absent). */
export function dealTestCommittee(
  threshold: number,
  members: number,
  secret?: GrumpkinScalar,
): TestCommittee {
  const dealt = testingDealShares(threshold, members, seed(), secret?.toString());
  return {
    groupKey: fromNapiPoint(dealt.groupKey),
    verificationShares: dealt.shares.map((s) => fromNapiPoint(s.verificationShare)),
    dealt,
  };
}

/** A random signing key. */
export const randomSigningKey = (): GrumpkinScalar => GrumpkinScalar.random();

/**
 * Answers ECDH requests with partials from in-process shares, as members' keepers would (each keeper takes the vault's
 * preaddress from its own manifest, never from the request). `members` chooses who responds; `corrupt` members answer
 * with a wrong share, so their proofs fail verification. Records every request.
 */
export class InProcessPartialsSource implements PartialsSource {
  readonly requests: PartialsRequest[] = [];
  readonly #preaddress: string;
  #committee: TestCommittee;
  #members: readonly number[];
  readonly #corrupt: ReadonlySet<number>;

  constructor(
    committee: TestCommittee,
    preaddress: Fr,
    members: readonly number[],
    options: { corrupt?: readonly number[] } = {},
  ) {
    this.#committee = committee;
    this.#preaddress = preaddress.toString();
    this.#members = members;
    this.#corrupt = new Set(options.corrupt);
  }

  /** Switches to the committee of a new epoch. */
  rotate(committee: TestCommittee, members: readonly number[]): void {
    this.#committee = committee;
    this.#members = members;
  }

  requestPartials(request: PartialsRequest): Promise<MemberPartials[]> {
    this.requests.push(request);
    const groupKey = this.#committee.dealt.groupKey;
    return Promise.resolve(
      this.#members.map((member) => {
        const share = this.#committee.dealt.shares[member - 1];
        if (!share) throw new Error(`No share for member ${member}`);
        const secret = this.#corrupt.has(member) ? GrumpkinScalar.random().toString() : share.share;
        return {
          member,
          partials: testingPartials(
            member,
            secret,
            request.epoch,
            this.#preaddress,
            groupKey,
            [...request.points],
            seed(),
          ),
        };
      }),
    );
  }
}
