import { Fr } from "@aztec-labs/foundation/curves/bn254";
import type { Approval as WireApproval, Proposal as WireProposal } from "@tecpan/napi";

import type { VaultTx } from "../wallet/vault-wallet.ts";
import type { VaultApproval } from "./approvals.ts";
import { decodePayload, encodePayload } from "./payload.ts";

/** Why a proposal cannot be turned back into a transaction. */
export class ProposalMismatchError extends Error {
  readonly reason: "stale-config" | "outer-hash";

  constructor(reason: "stale-config" | "outer-hash", message: string) {
    super(message);
    this.name = "ProposalMismatchError";
    this.reason = reason;
  }
}

/** The wire proposal for a prepared vault transaction. */
export function proposalOf(
  tx: VaultTx,
  meta: { summary: string; proposer: number; feeMethod: number },
): WireProposal {
  return {
    configVersion: tx.configVersion,
    outerHash: tx.outerHash.toString(),
    expiresAt: Number(tx.expiresAt),
    txNonce: tx.txNonce.toString(),
    feeMethod: meta.feeMethod,
    payload: encodePayload(tx.payload),
    summary: meta.summary,
    proposer: meta.proposer,
  };
}

/**
 * Rebuilds the vault transaction a proposal describes, re-deriving its outer hash from the payload under the vault's
 * current config. A member signs only what this returns: a proposal whose payload hashes differently, or that was
 * made under an older config version, is refused.
 */
export async function vaultTxOf(
  prepare: (
    payload: ReturnType<typeof decodePayload>,
    options: { expiresAt: bigint; txNonce: Fr },
  ) => Promise<VaultTx>,
  proposal: WireProposal,
): Promise<VaultTx> {
  const tx = await prepare(decodePayload(proposal.payload), {
    expiresAt: BigInt(proposal.expiresAt),
    txNonce: Fr.fromString(proposal.txNonce),
  });
  if (tx.configVersion !== proposal.configVersion)
    throw new ProposalMismatchError(
      "stale-config",
      `Proposed under config version ${proposal.configVersion}; the vault is at ${tx.configVersion}`,
    );
  if (!tx.outerHash.equals(Fr.fromString(proposal.outerHash)))
    throw new ProposalMismatchError(
      "outer-hash",
      "The payload does not hash to the proposed outer hash",
    );
  return tx;
}

/** Wire approvals as the witness encoder takes them. */
export const vaultApprovalsOf = (approvals: readonly WireApproval[]): VaultApproval[] =>
  approvals.map((a) => ({
    slot: a.slot,
    signature: a.signature.map((limb) => Fr.fromString(limb)),
  }));
