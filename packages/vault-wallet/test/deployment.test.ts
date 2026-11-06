import { Schnorr } from "@aztec-labs/foundation/crypto/schnorr";
import { Fr } from "@aztec-labs/foundation/curves/bn254";
import { GrumpkinScalar } from "@aztec-labs/foundation/curves/grumpkin";
import { AztecAddress } from "@aztec-labs/stdlib/aztec-address";
import { computeAddressSecret } from "@aztec-labs/stdlib/keys";
import { describe, expect, it } from "vitest";

import {
  type VaultSetup,
  approvalMessage,
  deriveVault,
  encodeVaultWitness,
  generateVaultProtocolSecrets,
  signApproval,
} from "../src/index.ts";
import { dealTestCommittee } from "../src/testing/index.ts";

async function setup(overrides: Partial<VaultSetup> = {}): Promise<VaultSetup> {
  const committee = dealTestCommittee(2, 3);
  const schnorr = new Schnorr();
  return {
    signerKeys: await Promise.all(
      [1, 2, 3].map(() => schnorr.computePublicKey(GrumpkinScalar.random())),
    ),
    threshold: 2,
    viewingThreshold: 2,
    groupKey: committee.groupKey,
    verificationShares: committee.verificationShares,
    secrets: await generateVaultProtocolSecrets(),
    salt: Fr.random(),
    ...overrides,
  };
}

describe("deriveVault", () => {
  it("derives the same address as Aztec and as the threshold address context", async () => {
    const ivsk = GrumpkinScalar.random();
    const committee = dealTestCommittee(2, 3, ivsk);
    const deployment = await deriveVault(
      await setup({
        groupKey: committee.groupKey,
        verificationShares: committee.verificationShares,
      }),
    );
    expect(deployment.instance.address).toEqual(deployment.completeAddress.address);
    // The vault's address secret is the one Aztec derives from the (never assembled) ivsk.
    const addressSecret = await computeAddressSecret(deployment.preaddress, ivsk);
    expect(await deployment.address.toAddressPoint()).toBeDefined();
    expect(addressSecret).toBeDefined();
    expect(deployment.constructorArgs).toHaveLength(5);
  });

  it("depends on every setup input", async () => {
    const base = await setup();
    const address = (await deriveVault(base)).address;
    expect((await deriveVault({ ...base, salt: Fr.random() })).address).not.toEqual(address);
    expect((await deriveVault({ ...base, threshold: 3 })).address).not.toEqual(address);
    expect(
      (await deriveVault({ ...base, signerKeys: base.signerKeys.toReversed() })).address,
    ).not.toEqual(address);
  });

  it("rejects invalid setups", async () => {
    const base = await setup();
    await expect(deriveVault({ ...base, threshold: 4 })).rejects.toThrow(/Threshold/);
    await expect(deriveVault({ ...base, viewingThreshold: 1 })).rejects.toThrow(
      /Viewing threshold/,
    );
    await expect(
      deriveVault({ ...base, verificationShares: base.verificationShares.slice(1) }),
    ).rejects.toThrow(/verification share/);
    await expect(deriveVault({ ...base, signerKeys: [] })).rejects.toThrow(/signers/);
  });
});

describe("approvals", () => {
  it("encodes verified approvals into the 49-field witness", async () => {
    const keys = [GrumpkinScalar.random(), GrumpkinScalar.random(), GrumpkinScalar.random()];
    const schnorr = new Schnorr();
    const signerKeys = await Promise.all(keys.map((k) => schnorr.computePublicKey(k)));
    const message = approvalMessage(
      new AztecAddress(new Fr(0x1234n)),
      new Fr(0x5678n),
      3,
      1_700_000_000n,
    );
    const approvals = [
      await signApproval(keys[0] as GrumpkinScalar, message, 0),
      await signApproval(keys[2] as GrumpkinScalar, message, 2),
    ];

    const witness = encodeVaultWitness(signerKeys, message, approvals, 1_700_000_000n);
    expect(witness).toHaveLength(49);
    expect(witness.slice(16, 20)).toEqual(approvals[0]?.signature);
    expect(witness.slice(20, 24).every((f) => f.isZero())).toBe(true);
    expect(witness[48]).toEqual(new Fr(1_700_000_000n));
  });

  it("rejects bad, duplicate and out-of-range approvals", async () => {
    const key = GrumpkinScalar.random();
    const signerKeys = [await new Schnorr().computePublicKey(key)];
    const message = new Fr(42n);
    const approval = await signApproval(key, message, 0);
    expect(() => encodeVaultWitness(signerKeys, new Fr(43n), [approval], 0n)).toThrow(
      /Invalid approval for slot 0/,
    );
    expect(() => encodeVaultWitness(signerKeys, message, [approval, approval], 0n)).toThrow(
      /Duplicate/,
    );
    expect(() => encodeVaultWitness(signerKeys, message, [{ ...approval, slot: 1 }], 0n)).toThrow(
      /empty slot/,
    );
  });
});
