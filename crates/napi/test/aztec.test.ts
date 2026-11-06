// Cross-checks the bindings against live @aztec-labs 6.0.0 code: addresses, address-secret sign, threshold ECDH,
// Schnorr approvals and the contract's wire vectors.
import { Grumpkin } from "@aztec-labs/foundation/crypto/grumpkin";
import { Schnorr } from "@aztec-labs/foundation/crypto/schnorr";
import { Fq, Fr } from "@aztec-labs/foundation/curves/bn254";
import type { Point as AztecPoint } from "@aztec-labs/foundation/curves/grumpkin";
import {
  computeAddress,
  computeAddressSecret,
  computePreaddress,
  deriveKeys,
  derivePublicKeyFromSecretKey,
} from "@aztec-labs/stdlib/keys";
import { deriveEcdhSharedSecretPoint } from "@aztec-labs/stdlib/logs";
import { describe, expect, it } from "vitest";

import * as napi from "../dist/index.js";
import * as testing from "../dist/testing/index.js";

const point = (p: AztecPoint): napi.Point => ({ x: p.x.toString(), y: p.y.toString() });
const seed = (n: number) => Buffer.alloc(32, n);
const at = <T>(items: readonly T[], index: number): T => {
  const item = items[index];
  if (item === undefined) throw new Error(`missing item ${index}`);
  return item;
};

async function account(n: bigint) {
  const { masterIncomingViewingSecretKey: ivsk, publicKeys } = await deriveKeys(
    new Fr(n * 0x1234567n),
  );
  const partialAddress = new Fr(n * 0x9abcdefn + 11n);
  const preaddress = await computePreaddress(await publicKeys.hash(), partialAddress);
  const addressSecret = await computeAddressSecret(preaddress, ivsk);
  // A note's ephemeral key and the shared point Aztec's PXE derives for it.
  const ephemeral = await derivePublicKeyFromSecretKey(new Fq(n * 0x55555n + 3n));
  return {
    ivsk,
    ivpk: point(publicKeys.ivpkM),
    preaddress,
    address: await computeAddress(publicKeys, partialAddress),
    addressSecret,
    ephemeral: point(ephemeral),
    sharedPoint: point(await deriveEcdhSharedSecretPoint(addressSecret, ephemeral)),
  };
}

const accounts = await Promise.all(Array.from({ length: 8 }, (_, i) => account(BigInt(i + 1))));

describe("address derivation", () => {
  it("matches Aztec's address and address-secret sign", () => {
    const signs = new Set<number>();
    for (const a of accounts) {
      const context = napi.addressContext(a.preaddress.toString(), a.ivpk);
      expect(context.address).toBe(a.address.toString());
      const plain =
        a.ivsk.add(new Fq(a.preaddress.toBigInt())).toBigInt() === a.addressSecret.toBigInt();
      expect(context.sign).toBe(plain ? 1 : -1);
      signs.add(context.sign);
    }
    // Both branches of the negation rule are exercised.
    expect(signs).toEqual(new Set([1, -1]));
  });

  it("matches Aztec's preaddress", async () => {
    const { publicKeys } = await deriveKeys(new Fr(99n));
    const hash = await publicKeys.hash();
    expect(napi.preaddress(hash.toString(), new Fr(5n).toString())).toBe(
      (await computePreaddress(hash, new Fr(5n))).toString(),
    );
  });
});

describe("threshold ECDH", () => {
  it("2-of-3 partials combine to Aztec's shared secret for both address-secret signs", () => {
    for (const a of accounts) {
      const committee = testing.testingDealShares(2, 3, seed(1), a.ivsk.toString());
      expect(committee.groupKey).toEqual(a.ivpk);
      const inputs = [a.ephemeral];
      const request = (members: number[]) => ({
        epoch: 3,
        preaddress: a.preaddress.toString(),
        ivpk: a.ivpk,
        verificationShares: committee.shares.map((s) => s.verificationShare),
        threshold: 2,
        inputs,
        responses: members.map((index) => ({
          member: index,
          partials: testing.testingPartials(
            index,
            at(committee.shares, index - 1).share,
            3,
            a.preaddress.toString(),
            a.ivpk,
            inputs,
            seed(index),
          ),
        })),
      });
      for (const members of [
        [1, 2],
        [1, 3],
        [2, 3],
      ]) {
        const result = napi.verifyAndCombine(request(members));
        expect(result.points).toEqual([a.sharedPoint]);
        expect(result.invalidMembers).toEqual([]);
      }
    }
  });

  it("rejects partials bound to another epoch and reports the member", () => {
    const a = at(accounts, 0);
    const committee = testing.testingDealShares(2, 3, seed(2), a.ivsk.toString());
    const inputs = [at(committee.shares, 0).verificationShare];
    const partials = (index: number, epoch: number) =>
      testing.testingPartials(
        index,
        at(committee.shares, index - 1).share,
        epoch,
        a.preaddress.toString(),
        a.ivpk,
        inputs,
        seed(index),
      );
    const request = {
      epoch: 3,
      preaddress: a.preaddress.toString(),
      ivpk: a.ivpk,
      verificationShares: committee.shares.map((s) => s.verificationShare),
      threshold: 2,
      inputs,
      responses: [
        { member: 1, partials: partials(1, 3) },
        { member: 2, partials: partials(2, 2) },
        { member: 3, partials: partials(3, 3) },
      ],
    };
    expect(napi.verifyAndCombine(request).invalidMembers).toEqual([2]);
    expect(() =>
      napi.verifyAndCombine({ ...request, responses: request.responses.slice(0, 2) }),
    ).toThrow();
  });
});

describe("approvals", () => {
  const schnorr = new Schnorr();
  const vault = new Fr(0x1234n).toString();
  const proposal: napi.Proposal = {
    configVersion: 3,
    outerHash: new Fr(0x5678n).toString(),
    expiresAt: 1_700_000_000,
    txNonce: new Fr(1n).toString(),
    feeMethod: 1,
    payload: Buffer.from([1, 2, 3]),
    summary: "transfer 10",
    proposer: 1,
  };

  it("verifies Aztec Schnorr signatures over the approval message", async () => {
    const key = new Fq(0xfeedn);
    const publicKey = point(await schnorr.computePublicKey(key));
    const message = napi.proposalApprovalMessage(proposal, vault);
    expect(message).toBe(napi.approvalMessage(vault, proposal.outerHash, 3, 1_700_000_000));
    const signature = await schnorr.constructSignature(Fr.fromString(message), key);
    const limbs = napi.signatureLimbs(signature.toBuffer());
    expect(limbs).toEqual(signature.toLimbFields().map((f) => f.toString()));
    expect(napi.verifySignature(publicKey, message, limbs)).toBe(true);
    expect(napi.verifySignature(publicKey, new Fr(1n).toString(), limbs)).toBe(false);

    const approval = { proposal: napi.proposalId(proposal), slot: 0, signature: limbs };
    expect(napi.verifyProposalApproval(proposal, approval, vault, publicKey)).toBe(true);
    expect(napi.verifyProposalApproval(proposal, approval, new Fr(1n).toString(), publicKey)).toBe(
      false,
    );
    expect(
      napi.verifyProposalApproval({ ...proposal, summary: "other" }, approval, vault, publicKey),
    ).toBe(false);
  });

  it("accepts the test signer's signatures in Aztec's verifier", async () => {
    const secret = new Fq(0xabcn);
    const message = new Fr(42n).toString();
    const limbs = testing.testingSign(secret.toString(), message, seed(3));
    const publicKey = testing.testingPublicKey(secret.toString());
    expect(publicKey).toEqual(point(await schnorr.computePublicKey(secret)));
    expect(napi.verifySignature(publicKey, message, limbs)).toBe(true);
  });

  it("round-trips proposal and approval bodies", () => {
    const decoded = napi.decodeProposal(napi.encodeProposal(proposal));
    expect({ ...decoded, payload: [...decoded.payload] }).toEqual({
      ...proposal,
      payload: [1, 2, 3],
    });
    expect(napi.proposalId(decoded)).toBe(napi.proposalId(proposal));
    const approval = {
      proposal: napi.proposalId(proposal),
      slot: 2,
      signature: ["0x01", "0x02", "0x03", "0x04"].map((v) => new Fr(BigInt(v)).toString()),
    };
    expect(napi.decodeApproval(napi.encodeApproval(approval))).toEqual(approval);
    expect(() => napi.decodeApproval(napi.encodeProposal(proposal))).toThrow();
    expect(() =>
      napi.decodeProposal(Buffer.concat([napi.encodeProposal(proposal), Buffer.from([0])])),
    ).toThrow();
    expect(napi.wireVersion()).toBe(0);
  });

  it("verifies Aztec Schnorr rejections, which never pass as approvals", async () => {
    const key = new Fq(0xbeefn);
    const publicKey = point(await schnorr.computePublicKey(key));
    const id = napi.proposalId(proposal);
    const message = napi.proposalRejectionMessage(id, vault);
    expect(message).not.toBe(napi.proposalApprovalMessage(proposal, vault));
    const signature = await schnorr.constructSignature(Fr.fromString(message), key);
    const rejection: napi.Rejection = {
      proposal: id,
      slot: 1,
      reason: "wrong recipient",
      signature: signature.toLimbFields().map((f) => f.toString()),
    };
    expect(napi.verifyProposalRejection(rejection, vault, publicKey)).toBe(true);
    expect(napi.verifyProposalRejection(rejection, new Fr(1n).toString(), publicKey)).toBe(false);
    expect(napi.verifyProposalApproval(proposal, rejection, vault, publicKey)).toBe(false);
    expect(napi.decodeRejection(napi.encodeRejection(rejection))).toEqual(rejection);
    expect(() => napi.encodeRejection({ ...rejection, reason: "x".repeat(281) })).toThrow();
  });
});

/** `count` points `(from + i) · G`, matching the Noir and Rust `points(count, seed)` test helpers. */
const multiples = (count: number, from: number) =>
  Promise.all(
    Array.from({ length: count }, async (_, i) =>
      point(await derivePublicKeyFromSecretKey(new Fq(BigInt(from + i)))),
    ),
  );

// Same inputs and values as contracts/vault_contract/src/wire_vectors.nr and crates/crypto/tests/vault_wire.rs.
describe("contract wire vectors", () => {
  it("matches the Noir commitments and approval message", async () => {
    expect(napi.signerSetCommitment(await multiples(3, 1))).toBe(
      "0x1814a2b2e68b9355116626143fe5d6549a64c5618c702464c1ccc982b659fe40",
    );
    expect(napi.committeeCommitment(2, await multiples(3, 1000))).toBe(
      "0x2bafa2a18e587afb28f4f7a6b48a24e94e39ca2b1d7c9caf151a03afe750f1a8",
    );
    expect(
      napi.approvalMessage(
        new Fr(0x1234n).toString(),
        new Fr(0x5678n).toString(),
        3,
        1_700_000_000,
      ),
    ).toBe("0x1705887c43c37444a89a18523d5dd6729415b781cc5946946c9983f5a7371251");
  });

  it("lays out the 49-field witness", async () => {
    const keys = await multiples(2, 1);
    const sig = ["0x01", "0x02", "0x03", "0x04"].map((v) => new Fr(BigInt(v)).toString());
    const witness = napi.encodeWitness(keys, [null, sig], 1_700_000_000);
    expect(witness).toHaveLength(49);
    expect(witness.slice(0, 4)).toEqual([
      at(keys, 0).x,
      at(keys, 0).y,
      at(keys, 1).x,
      at(keys, 1).y,
    ]);
    expect(witness.slice(16, 20)).toEqual(Array(4).fill(new Fr(0n).toString()));
    expect(witness.slice(20, 24)).toEqual(sig);
    expect(witness[48]).toBe(new Fr(1_700_000_000n).toString());
  });

  it("rejects off-curve points and non-canonical fields", () => {
    expect(() =>
      napi.signerSetCommitment([{ x: new Fr(1n).toString(), y: new Fr(1n).toString() }]),
    ).toThrow();
    expect(() =>
      napi.approvalMessage(`0x${"ff".repeat(32)}`, new Fr(1n).toString(), 0, 0),
    ).toThrow();
    expect(() => napi.approvalMessage("0x1234", new Fr(1n).toString(), 0, 0)).toThrow();
    expect(Grumpkin.generator.x.toString()).toBe(testing.testingPublicKey(new Fq(1n).toString()).x);
  });
});
