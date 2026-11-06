import { Schnorr } from "@aztec-labs/foundation/crypto/schnorr";
import { Fr } from "@aztec-labs/foundation/curves/bn254";
import type { GrumpkinScalar, Point } from "@aztec-labs/foundation/curves/grumpkin";
import type { AztecAddress } from "@aztec-labs/stdlib/aztec-address";
import {
  approvalMessage as napiApprovalMessage,
  encodeWitness,
  verifySignature,
} from "@tecpan/napi";

import { toNapiPoint } from "../keys/points.ts";

/** One signer's approval: its slot and Schnorr signature limbs `[s.lo, s.hi, e.lo, e.hi]`. */
export interface VaultApproval {
  readonly slot: number;
  readonly signature: readonly Fr[];
}

/** What signers sign: `Poseidon2("tecpan.approval.v0", [vault, outerHash, configVersion, expiresAt])`. */
export function approvalMessage(
  vault: AztecAddress,
  outerHash: Fr,
  configVersion: number,
  expiresAt: bigint,
): Fr {
  return Fr.fromString(
    napiApprovalMessage(vault.toString(), outerHash.toString(), configVersion, Number(expiresAt)),
  );
}

/** Signs an approval message with a Grumpkin signing key, as the desktop signer does. */
export async function signApproval(
  signingKey: GrumpkinScalar,
  message: Fr,
  slot: number,
): Promise<VaultApproval> {
  const signature = await new Schnorr().constructSignature(message, signingKey);
  return { slot, signature: signature.toLimbFields() };
}

/**
 * The 49-field auth witness for `approvals` of `message`. Every approval is verified first, so a bad signature fails
 * here with its slot instead of inside the circuit.
 */
export function encodeVaultWitness(
  signerKeys: readonly Point[],
  message: Fr,
  approvals: readonly VaultApproval[],
  expiresAt: bigint,
): Fr[] {
  const bySlot: (string[] | null)[] = signerKeys.map(() => null);
  for (const { slot, signature } of approvals) {
    const key = signerKeys[slot];
    if (!key) throw new Error(`Approval for empty slot ${slot}`);
    if (bySlot[slot]) throw new Error(`Duplicate approval for slot ${slot}`);
    const limbs = signature.map((limb) => limb.toString());
    if (!verifySignature(toNapiPoint(key), message.toString(), limbs))
      throw new Error(`Invalid approval for slot ${slot}`);
    bySlot[slot] = limbs;
  }
  return encodeWitness(signerKeys.map(toNapiPoint), bySlot, Number(expiresAt)).map((field) =>
    Fr.fromString(field),
  );
}
