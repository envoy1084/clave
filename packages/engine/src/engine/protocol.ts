// What the engine needs to run, and the founded-vault record it keeps per vault (`vaults/<address>.json`).
import type { Secret } from "../rpc/contract.ts";
import type { DaemonEvent, Point } from "../types.ts";

/** Everything the engine needs to run; secrets arrive here once and are never logged. */
export interface EngineInit {
  readonly dataDir: string;
  readonly nodeUrl: string;
  readonly proverEnabled: boolean;
  readonly profile: { readonly name: string; readonly signerKey: Point } | null;
  /** The member's own Aztec account (an initializerless Schnorr account). */
  readonly account: {
    readonly secret: string;
    readonly salt: string;
    readonly signingKey: string;
  } | null;
  readonly vaults: readonly VaultRecord[];
}

/** A founded vault as the engine needs it after a restart. */
export interface VaultRecord {
  readonly address: string;
  readonly name: string;
  readonly foundedAt: number;
  readonly myIndex: number;
  readonly lobby: string;
  readonly session: string;
  readonly threshold: number;
  readonly viewingThreshold: number;
  readonly salt: string;
  /** Encoded protocol secrets and the epoch tagging secret (see `secrets.ts`). */
  readonly secrets: string;
  readonly groupKey: Point;
  readonly verificationShares: readonly Point[];
  readonly members: readonly {
    readonly name: string;
    readonly signerKey: Point;
    readonly identity: string;
    readonly transport: string;
    readonly addresses: readonly string[];
  }[];
  readonly transcript?: string;
  readonly deployTx?: string;
}

/** What the engine asks of the daemon around it. */
export interface EngineHost {
  emit(event: DaemonEvent): void;
  saveVault(record: VaultRecord): Promise<void>;
  adoptAccount(address: string): void;
  /** Signs a field element with the member's approval key (see `signer/signer.ts`). */
  sign(message: string, secret: Secret | undefined): Promise<string[]>;
  /** The address book, for labelling recipients. */
  contacts(): readonly { readonly address: string; readonly label: string }[];
}
