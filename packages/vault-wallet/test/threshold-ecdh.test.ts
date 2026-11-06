import { Fr } from "@aztec-labs/foundation/curves/bn254";
import { GrumpkinScalar } from "@aztec-labs/foundation/curves/grumpkin";
import { AztecAddress } from "@aztec-labs/stdlib/aztec-address";
import { computeAddressSecret, derivePublicKeyFromSecretKey } from "@aztec-labs/stdlib/keys";
import { deriveEcdhSharedSecretPoint } from "@aztec-labs/stdlib/logs";
import { addressContext } from "@tecpan/napi";
import { describe, expect, it } from "vitest";

import { ThresholdEcdhProvider, toNapiPoint } from "../src/index.ts";
import { InProcessPartialsSource, dealTestCommittee } from "../src/testing/index.ts";

async function fixture(corrupt: number[] = [], members = [1, 2]) {
  const ivsk = GrumpkinScalar.random();
  const committee = dealTestCommittee(2, 3, ivsk);
  const preaddress = Fr.random();
  const vault = AztecAddress.fromStringUnsafe(
    addressContext(preaddress.toString(), toNapiPoint(committee.groupKey)).address,
  );
  const source = new InProcessPartialsSource(committee, preaddress, members, { corrupt });
  const invalid: number[][] = [];
  const provider = new ThresholdEcdhProvider(
    {
      vault,
      epoch: 0,
      preaddress: preaddress.toString(),
      groupKey: toNapiPoint(committee.groupKey),
      verificationShares: committee.verificationShares.map(toNapiPoint),
      threshold: 2,
    },
    source,
    (bad) => invalid.push(bad),
  );
  const points = await Promise.all(
    [1, 2].map(() => derivePublicKeyFromSecretKey(GrumpkinScalar.random())),
  );
  const addressSecret = await computeAddressSecret(preaddress, ivsk);
  const expected = await Promise.all(
    points.map((p) => deriveEcdhSharedSecretPoint(addressSecret, p)),
  );
  return { provider, source, points, expected, invalid };
}

describe("ThresholdEcdhProvider", () => {
  it("combines verified partials into Aztec's shared secret", async () => {
    const { provider, source, points, expected, invalid } = await fixture();
    expect(await provider.mul(points, "decryption")).toEqual(expected);
    expect(source.requests).toHaveLength(1);
    expect(source.requests[0]?.purpose).toBe("decryption");
    expect(invalid).toEqual([]);
  });

  it("reports and skips a member with bad proofs when enough others answer", async () => {
    const { provider, points, expected, invalid } = await fixture([2], [1, 2, 3]);
    expect(await provider.mul(points, "tagging")).toEqual(expected);
    expect(invalid).toEqual([[2]]);
  });

  it("fails below threshold", async () => {
    const { provider, points } = await fixture([2], [1, 2]);
    await expect(provider.mul(points, "decryption")).rejects.toThrow();
  });
});
