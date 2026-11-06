import { lookupValidity } from "@aztec-labs/aztec.js/authorization";
// A 2-of-3 vault with a 2-of-3 viewing committee on a local network, operated by two members' wallets. Members' keepers
// are simulated in-process; no wallet ever holds the vault's incoming viewing secret (the key store throws if asked).
import { Contract } from "@aztec-labs/aztec.js/contracts";
import { Schnorr } from "@aztec-labs/foundation/crypto/schnorr";
import { Fr } from "@aztec-labs/foundation/curves/bn254";
import { GrumpkinScalar, type Point } from "@aztec-labs/foundation/curves/grumpkin";
import { TokenContract } from "@aztec-labs/noir-contracts.js/Token";
import type { AztecNode } from "@aztec-labs/stdlib/interfaces/client";
import { ExecutionPayload, mergeExecutionPayloads } from "@aztec-labs/stdlib/tx";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import {
  type VaultApproval,
  type VaultDeployment,
  type VaultTx,
  type VaultViewing,
  deriveVault,
  generateEpochTaggingSecret,
  generateVaultProtocolSecrets,
  signApproval,
  vaultArtifact,
} from "../../src/index.ts";
import {
  InProcessPartialsSource,
  type TestCommittee,
  dealTestCommittee,
} from "../../src/testing/index.ts";
import {
  type AztecAddress,
  type Member,
  chainTime,
  memberWallet,
  networkAvailable,
  node,
} from "./network.ts";

const available = await networkAvailable();
const schnorr = new Schnorr();

/** ECDH traffic one engine generated (`TECPAN_E2E_STATS=1`). */
const stats = (source: InProcessPartialsSource) => ({
  requests: source.requests.length,
  points: source.requests.reduce((n, r) => n + r.points.length, 0),
  tagging: source.requests.filter((r) => r.purpose === "tagging").length,
  distinctPoints: new Set(source.requests.flatMap((r) => r.points.map((p) => p.x))).size,
});

const pad = (points: readonly Point[]) =>
  Array.from({ length: 8 }, (_, i) =>
    points[i] ? { x: points[i].x, y: points[i].y } : { x: Fr.ZERO, y: Fr.ZERO },
  );

describe.skipIf(!available)("vault on a local network", () => {
  let aztecNode: AztecNode;
  let alice: Member; // member 1: deploys and executes
  let bob: Member; // member 2: an independent engine
  let signingKeys: GrumpkinScalar[];
  let ivsk: GrumpkinScalar; // known only to this test's dealer, standing in for the DKG
  let committee: TestCommittee;
  let deployment: VaultDeployment;
  let vault: AztecAddress;
  let token: TokenContract;
  let aliceSource: InProcessPartialsSource;
  let bobSource: InProcessPartialsSource;
  let epochTaggingSecret: Point;

  const viewing = (
    source: InProcessPartialsSource,
    epoch: number,
    current: TestCommittee,
  ): VaultViewing => ({
    epoch,
    verificationShares: current.verificationShares,
    viewingThreshold: 2,
    source,
    epochTaggingSecret,
  });

  const approve = async (message: Fr, slots: number[]): Promise<VaultApproval[]> =>
    Promise.all(
      slots.map((slot) => signApproval(signingKeys[slot] as GrumpkinScalar, message, slot)),
    );

  const expiry = async (seconds = 3600) => (await chainTime(aztecNode)) + BigInt(seconds);

  const balance = async (member: Member, owner: AztecAddress) =>
    (
      await TokenContract.at(token.address, member.wallet)
        .methods.balance_of_private(owner)
        .simulate({ from: owner })
    ).result as bigint;

  const transferFromVault = async (
    to: AztecAddress,
    amount: bigint,
    options: { txNonce?: Fr; seconds?: number } = {},
  ) =>
    alice.wallet.prepareVaultTx(
      vault,
      await token.methods.transfer(to, amount).request({ fee: alice.fee }),
      { expiresAt: await expiry(options.seconds), txNonce: options.txNonce },
    );

  beforeAll(async () => {
    aztecNode = node();
    [alice, bob] = await Promise.all([memberWallet(aztecNode, 0), memberWallet(aztecNode, 1)]);

    signingKeys = [GrumpkinScalar.random(), GrumpkinScalar.random(), GrumpkinScalar.random()];
    ivsk = GrumpkinScalar.random();
    committee = dealTestCommittee(2, 3, ivsk);
    epochTaggingSecret = await generateEpochTaggingSecret();
    deployment = await deriveVault({
      signerKeys: await Promise.all(signingKeys.map((k) => schnorr.computePublicKey(k))),
      threshold: 2,
      viewingThreshold: 2,
      groupKey: committee.groupKey,
      verificationShares: committee.verificationShares,
      secrets: await generateVaultProtocolSecrets(),
      salt: Fr.random(),
    });
    vault = deployment.address;

    // Each engine reaches a different qualified set of keepers.
    aliceSource = new InProcessPartialsSource(committee, deployment.preaddress, [1, 2]);
    bobSource = new InProcessPartialsSource(committee, deployment.preaddress, [2, 3]);
    await alice.wallet.registerVault(deployment, viewing(aliceSource, 0, committee));
    await bob.wallet.registerVault(deployment, viewing(bobSource, 0, committee));
    await alice.wallet.deployVault(vault, { from: alice.account, fee: alice.fee });

    ({ contract: token } = await TokenContract.deploy(
      alice.wallet,
      alice.account,
      "Tecpan",
      "TEC",
      18,
    ).send({
      from: alice.account,
      fee: alice.fee,
    }));
    const tokenInstance = await aztecNode.getContract(token.address);
    if (!tokenInstance) throw new Error("Token instance not published");
    await bob.wallet.registerContract(tokenInstance, TokenContract.artifact);
    await token.methods.mint_to_private(vault, 1000n).send({ from: alice.account, fee: alice.fee });
  });

  afterAll(async () => {
    await Promise.all([alice?.wallet.stop(), bob?.wallet.stop()]);
    if (process.env.TECPAN_E2E_STATS === "1") {
      process.stdout.write(
        `${JSON.stringify({ alice: stats(aliceSource), bob: stats(bobSource) })}\n`,
      );
    }
  });

  it("deploys and both members verify the on-chain config", async () => {
    const configs = await Promise.all(
      [alice, bob].map((member) => member.wallet.verifyVaultConfig(vault)),
    );
    for (const config of configs) {
      expect(config).toMatchObject({
        threshold: 2,
        signerCount: 3,
        viewingThreshold: 2,
        version: 0,
      });
    }
  });

  it("discovers the vault's notes through threshold ECDH in both engines", async () => {
    expect(await balance(alice, vault)).toBe(1000n);
    expect(await balance(bob, vault)).toBe(1000n);
    expect(aliceSource.requests.length).toBeGreaterThan(0);
    expect(bobSource.requests.length).toBeGreaterThan(0);
  });

  it("previews a vault transfer before any approval", async () => {
    const tx = await transferFromVault(bob.account, 1n);
    const preview = await alice.wallet.previewVaultTx(tx);
    expect(preview.gasUsed.totalGas.l2Gas).toBeGreaterThan(0);
  });

  it("rejects execution below the threshold", async () => {
    const tx = await transferFromVault(bob.account, 1n);
    await expect(
      alice.wallet.executeVaultTx(tx, await approve(tx.approvalMessage, [1])),
    ).rejects.toThrow(/assert\(valid_fn\(self\.context, message_hash\)\)/);
  });

  it("executes a 2-of-3 private transfer and both engines see the result", async () => {
    const tx = await transferFromVault(bob.account, 100n);
    const receipt = await alice.wallet.executeVaultTx(
      tx,
      await approve(tx.approvalMessage, [0, 2]),
    );
    expect(receipt.isMined()).toBe(true);
    expect(await balance(alice, vault)).toBe(900n);
    expect(await balance(bob, vault)).toBe(900n);
    expect(await balance(bob, bob.account)).toBe(100n);
  });

  it("rejects approvals past their deadline", async () => {
    const tx = await transferFromVault(bob.account, 1n, { seconds: -1 });
    await expect(
      alice.wallet.executeVaultTx(tx, await approve(tx.approvalMessage, [0, 1])),
    ).rejects.toThrow(/approval expired/);
  });

  it("cancellation burns the nonce", async () => {
    const original = await transferFromVault(bob.account, 5n);
    const originalApprovals = await approve(original.approvalMessage, [0, 1]);
    const feeOnly = mergeExecutionPayloads([
      await alice.fee.paymentMethod.getExecutionPayload(),
      ExecutionPayload.empty(),
    ]);
    const cancel = await alice.wallet.prepareVaultTx(vault, feeOnly, {
      expiresAt: await expiry(),
      txNonce: original.txNonce,
    });
    await alice.wallet.executeVaultTx(cancel, await approve(cancel.approvalMessage, [1, 2]));
    await expect(alice.wallet.executeVaultTx(original, originalApprovals)).rejects.toThrow(
      /Existing nullifier/,
    );
    expect(await balance(alice, vault)).toBe(900n);
  });

  it("authorizes transfer_in_private through verify_private_authwit", async () => {
    const authwitNonce = Fr.random();
    const action = token.methods.transfer_in_private(vault, alice.account, 10n, authwitNonce);
    const intent = { caller: alice.account, call: await action.getFunctionCall() };
    const request = await alice.wallet.prepareVaultAuthWit(vault, intent, {
      expiresAt: await expiry(),
    });
    const authwit = alice.wallet.createVaultAuthWit(
      request,
      await approve(request.approvalMessage, [0, 2]),
    );

    expect((await lookupValidity(alice.wallet, vault, intent, authwit)).isValidInPrivate).toBe(
      true,
    );
    await action.with({ authWitnesses: [authwit] }).send({
      from: alice.account,
      fee: alice.fee,
      additionalScopes: [vault],
    });
    expect((await lookupValidity(alice.wallet, vault, intent, authwit)).isValidInPrivate).toBe(
      false,
    );
    expect(await balance(alice, vault)).toBe(890n);
    expect(await balance(bob, vault)).toBe(890n);
  });

  it("invalidate_pending bumps the version and voids collected approvals", async () => {
    const pending = await transferFromVault(bob.account, 1n);
    const pendingApprovals = await approve(pending.approvalMessage, [0, 1]);
    const invalidate = Contract.at(
      vault,
      vaultArtifact(),
      alice.wallet,
    ).methods.invalidate_pending?.();
    if (!invalidate) throw new Error("invalidate_pending missing from the artifact");
    const tx = await alice.wallet.prepareVaultTx(
      vault,
      await invalidate.request({ fee: alice.fee }),
      {
        expiresAt: await expiry(),
      },
    );
    await alice.wallet.executeVaultTx(tx, await approve(tx.approvalMessage, [1, 2]));
    expect((await bob.wallet.getVaultConfig(vault)).version).toBe(1);
    await expect(alice.wallet.executeVaultTx(pending, pendingApprovals)).rejects.toThrow(
      /invalid signature/,
    );
  });

  it("rotates membership; approvals collected before rotation are rejected", async () => {
    const stale = await transferFromVault(bob.account, 1n);
    const staleApprovals = await approve(stale.approvalMessage, [0, 1]);

    // Member 3 leaves, member 4 joins; the committee reshares the same secret (the group key is unchanged).
    const newKey = GrumpkinScalar.random();
    const newCommittee = dealTestCommittee(2, 3, ivsk);
    expect(newCommittee.groupKey).toEqual(committee.groupKey);
    const current = await Promise.all(signingKeys.map((k) => schnorr.computePublicKey(k)));
    const next = [current[0] as Point, current[1] as Point, await schnorr.computePublicKey(newKey)];
    const vaultContract = Contract.at(vault, vaultArtifact(), alice.wallet);
    const rotate = vaultContract.methods.rotate_membership?.(
      pad(current),
      pad(next),
      3,
      2,
      2,
      pad(newCommittee.verificationShares),
    );
    if (!rotate) throw new Error("rotate_membership missing from the artifact");
    const tx: VaultTx = await alice.wallet.prepareVaultTx(
      vault,
      await rotate.request({ fee: alice.fee }),
      {
        expiresAt: await expiry(),
      },
    );
    await alice.wallet.executeVaultTx(tx, await approve(tx.approvalMessage, [0, 1]));

    // Keepers activate the new shares at the new epoch.
    aliceSource.rotate(newCommittee, [1, 3]);
    bobSource.rotate(newCommittee, [2, 3]);
    await alice.wallet.setViewing(vault, viewing(aliceSource, 2, newCommittee));
    await bob.wallet.setViewing(vault, viewing(bobSource, 2, newCommittee));
    signingKeys = [signingKeys[0] as GrumpkinScalar, signingKeys[1] as GrumpkinScalar, newKey];

    const config = await bob.wallet.getVaultConfig(vault);
    expect(config.version).toBe(2);
    await expect(alice.wallet.executeVaultTx(stale, staleApprovals)).rejects.toThrow(
      /signer set mismatch/,
    );

    // The new member's approval counts, and both engines still read the vault with the reshared committee.
    const fresh = await transferFromVault(bob.account, 40n);
    await alice.wallet.executeVaultTx(fresh, await approve(fresh.approvalMessage, [0, 2]));
    expect(await balance(alice, vault)).toBe(850n);
    expect(await balance(bob, vault)).toBe(850n);
  });
});
