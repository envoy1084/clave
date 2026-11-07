import { type ContractArtifact, loadContractArtifact } from "@aztec-labs/stdlib/abi";
import type { NoirCompiledContract } from "@aztec-labs/stdlib/noir";
import compiled from "@clave/contracts/ClaveVault.json" with { type: "json" };

let artifact: ContractArtifact | undefined;

/** The compiled `ClaveVault` contract (built by `@clave/contracts`). */
export function vaultArtifact(): ContractArtifact {
  artifact ??= loadContractArtifact(compiled as unknown as NoirCompiledContract);
  return artifact;
}
