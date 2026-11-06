/**
 * Tecpan vault wallet: hosts an Aztec PXE able to operate a threshold-viewing vault. The patched PXE routes every use
 * of an account's incoming viewing secret through {@link ThresholdKeyStore}, which never holds a vault's secret and
 * obtains `address_secret · P` from members through threshold ECDH. See `architecture/pxe-integration.md`.
 */
export { initBarretenberg } from "./barretenberg.ts";
export type { EcdhProvider, EcdhPurpose } from "./keys/ecdh.ts";
export { fromNapiPoint, toNapiPoint } from "./keys/points.ts";
export {
  type PartialsRequest,
  type PartialsSource,
  ThresholdEcdhProvider,
  type ViewingCommittee,
} from "./keys/threshold-ecdh.ts";
export {
  type ThresholdAccountKeys,
  ThresholdKeyStore,
  ThresholdViewingError,
} from "./keys/threshold-key-store.ts";
export { VaultAccountContract, VaultWitnessProvider } from "./vault/account.ts";
export {
  type VaultApproval,
  approvalMessage,
  encodeVaultWitness,
  signApproval,
} from "./vault/approvals.ts";
export { vaultArtifact } from "./vault/artifact.ts";
export { type VaultConfig, readSignerKeys, readVaultConfig } from "./vault/config.ts";
export {
  MAX_SIGNERS,
  type VaultDeployment,
  type VaultSetup,
  deriveVault,
} from "./vault/deployment.ts";
export {
  type VaultProtocolSecrets,
  generateEpochTaggingSecret,
  generateVaultProtocolSecrets,
  thresholdAccountKeys,
  vaultPublicKeys,
} from "./vault/keys.ts";
export {
  type VaultAuthWitRequest,
  type VaultTx,
  type VaultViewing,
  VaultWallet,
  type VaultWalletOptions,
} from "./wallet/vault-wallet.ts";
export { AZTEC_VERSION } from "./version.ts";
