// Messages between the main process and the engine `utilityProcess` (over its parent port).
import type { AppEvent, Point } from "./types.ts";

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
  readonly keeper: { readonly socket: string; readonly token: string } | null;
  readonly vaults: readonly VaultRecord[];
}

/** A founded vault as the engine needs it after a restart. Sealed at rest by the main process (`safeStorage`). */
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
  /** Encoded protocol secrets and the epoch tagging secret (see `engine/secrets.ts`). */
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

export type MainToEngine =
  | { readonly kind: "init"; readonly init: EngineInit }
  | {
      readonly kind: "call";
      readonly id: number;
      readonly method: string;
      readonly params: unknown;
    }
  | {
      readonly kind: "host-reply";
      readonly id: number;
      readonly result?: unknown;
      readonly error?: string;
    };

export type EngineToMain =
  | { readonly kind: "ready"; readonly account?: string }
  | {
      readonly kind: "reply";
      readonly id: number;
      readonly result?: unknown;
      readonly error?: string;
    }
  | { readonly kind: "event"; readonly event: AppEvent }
  | {
      readonly kind: "host-call";
      readonly id: number;
      readonly method: HostMethod;
      readonly params: unknown;
    };

/** Calls the engine makes back into the main process. */
export type HostMethod = "vault.save";
