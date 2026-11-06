import type { Point as NapiPoint } from "@tecpan/napi";

/** A keeper's public identity, as `keeper.info` reports it. */
export interface KeeperInfo {
  readonly identity: string;
  readonly transport: string;
  readonly addresses: readonly string[];
  readonly version: string;
}

/** A ceremony participant. */
export interface KeeperParticipant {
  readonly identity: string;
  readonly transport: string;
  readonly addresses: readonly string[];
}

/** A committee manifest (keeper JSON shape). */
export interface KeeperManifest {
  readonly vault: string;
  readonly epoch: number;
  readonly threshold: number;
  readonly viewingThreshold: number;
  readonly preaddress: string;
  readonly groupKey: NapiPoint;
  readonly members: readonly {
    readonly index: number;
    readonly signerKey: NapiPoint;
    readonly verificationShare: NapiPoint;
    readonly identity: string;
    readonly transport: string;
    readonly addresses: readonly string[];
  }[];
}

/** A finished DKG. */
export interface KeeperDkgResult {
  readonly index: number;
  readonly threshold: number;
  readonly groupKey: NapiPoint;
  readonly verificationShares: readonly NapiPoint[];
  readonly transcript: string;
}

/** A finished reshare. */
export interface KeeperReshareResult {
  readonly newIndex: number | null;
  readonly manifest: KeeperManifest | null;
  readonly transcript: string | null;
}

/** Ceremony status. */
export interface KeeperCeremony<R> {
  readonly kind: "dkg" | "reshare";
  readonly status: "running" | "completed" | "aborted" | "activated";
  readonly failure: { readonly reason: string; readonly blamed: readonly string[] } | null;
  readonly result: R | null;
}

/** A vault as a keeper sees it. */
export interface KeeperVault {
  readonly manifest: KeeperManifest;
  readonly status: "active" | "retired" | "removed";
  readonly index: number | null;
  readonly serving: boolean;
}
