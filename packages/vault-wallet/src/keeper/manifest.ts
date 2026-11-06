import { toNapiPoint } from "../keys/points.ts";
import type { VaultDeployment } from "../vault/deployment.ts";
import type { KeeperManifest, KeeperParticipant } from "./types.ts";

/**
 * The committee manifest members register with their keepers after the DKG: the vault's address context, both
 * thresholds, and per member its signing key, verification share and keeper. `keepers` are in member order.
 */
export function keeperManifest(
  deployment: VaultDeployment,
  keepers: readonly KeeperParticipant[],
  epoch = 0,
): KeeperManifest {
  const { setup } = deployment;
  if (keepers.length !== setup.signerKeys.length)
    throw new Error("One keeper per member is required");
  return {
    vault: deployment.address.toString(),
    epoch,
    threshold: setup.threshold,
    viewingThreshold: setup.viewingThreshold,
    preaddress: deployment.preaddress.toString(),
    groupKey: toNapiPoint(setup.groupKey),
    members: keepers.map((keeper, i) => {
      const signerKey = setup.signerKeys[i];
      const verificationShare = setup.verificationShares[i];
      if (!signerKey || !verificationShare)
        throw new Error(`Member ${i + 1} is missing from the setup`);
      return {
        index: i + 1,
        signerKey: toNapiPoint(signerKey),
        verificationShare: toNapiPoint(verificationShare),
        identity: keeper.identity,
        transport: keeper.transport,
        addresses: keeper.addresses,
      };
    }),
  };
}
