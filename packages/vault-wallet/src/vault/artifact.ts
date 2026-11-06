import { type ContractArtifact, loadContractArtifact } from "@aztec-labs/stdlib/abi";
import type { NoirCompiledContract } from "@aztec-labs/stdlib/noir";
import compiled from "@tecpan/contracts/TecpanVault.json" with { type: "json" };

let artifact: ContractArtifact | undefined;

/** The compiled `TecpanVault` contract (built by `@tecpan/contracts`). */
export function vaultArtifact(): ContractArtifact {
  artifact ??= loadContractArtifact(compiled as unknown as NoirCompiledContract);
  return artifact;
}
