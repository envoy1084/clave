import { Fr } from "@aztec-labs/foundation/curves/bn254";
import { GrumpkinScalar, type Point } from "@aztec-labs/foundation/curves/grumpkin";
import {
  PublicKeys,
  deriveKeys,
  derivePublicKeyFromSecretKey,
  hashPublicKey,
} from "@aztec-labs/stdlib/keys";

import type { ThresholdAccountKeys } from "../keys/threshold-key-store.ts";

/**
 * Account secrets every member holds for a vault, besides its threshold-shared incoming viewing key: the nullifier
 * hiding, outgoing viewing and tagging secrets, and the public message-signing and fallback keys (whose secrets are
 * discarded at generation). Shared with joining members over the keeper channel; never published.
 */
export interface VaultProtocolSecrets {
  readonly nullifierHidingSecretKey: GrumpkinScalar;
  readonly outgoingViewingSecretKey: GrumpkinScalar;
  readonly taggingSecretKey: GrumpkinScalar;
  readonly messageSigningPublicKey: Point;
  readonly fallbackPublicKey: Point;
}

/** Fresh protocol secrets for a new vault. */
export async function generateVaultProtocolSecrets(): Promise<VaultProtocolSecrets> {
  const keys = await deriveKeys(Fr.random());
  return {
    nullifierHidingSecretKey: keys.masterNullifierHidingSecretKey,
    outgoingViewingSecretKey: keys.masterOutgoingViewingSecretKey,
    taggingSecretKey: keys.masterTaggingSecretKey,
    messageSigningPublicKey: keys.masterMessageSigningPublicKey,
    fallbackPublicKey: keys.masterFallbackPublicKey,
  };
}

/** A fresh per-epoch tagging secret for the vault's unconstrained self-sends (rotated on every reshare). */
export function generateEpochTaggingSecret(): Promise<Point> {
  return derivePublicKeyFromSecretKey(GrumpkinScalar.random());
}

/** The vault's public keys: its protocol keys plus the committee's group key `Ivpk`. */
export async function vaultPublicKeys(
  secrets: VaultProtocolSecrets,
  groupKey: Point,
): Promise<PublicKeys> {
  return new PublicKeys(
    await hashPublicKey(await derivePublicKeyFromSecretKey(secrets.nullifierHidingSecretKey)),
    groupKey,
    await hashPublicKey(await derivePublicKeyFromSecretKey(secrets.outgoingViewingSecretKey)),
    await hashPublicKey(await derivePublicKeyFromSecretKey(secrets.taggingSecretKey)),
    await hashPublicKey(secrets.messageSigningPublicKey),
    await hashPublicKey(secrets.fallbackPublicKey),
  );
}

/** Key-store registration for the vault (no incoming viewing secret). */
export function thresholdAccountKeys(
  secrets: VaultProtocolSecrets,
  groupKey: Point,
): ThresholdAccountKeys {
  return {
    kind: "threshold",
    masterNullifierHidingSecretKey: secrets.nullifierHidingSecretKey,
    masterIncomingViewingPublicKey: groupKey,
    masterOutgoingViewingSecretKey: secrets.outgoingViewingSecretKey,
    masterTaggingSecretKey: secrets.taggingSecretKey,
    masterMessageSigningPublicKey: secrets.messageSigningPublicKey,
    masterFallbackPublicKey: secrets.fallbackPublicKey,
  };
}
