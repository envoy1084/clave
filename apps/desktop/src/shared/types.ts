// Data shapes exchanged between the renderer, the main process and the engine. Plain JSON only: bytes and field
// elements are 0x-prefixed hex strings, points are `{ x, y }` hex pairs.

export interface Point {
  readonly x: string;
  readonly y: string;
}

export interface Settings {
  /** Aztec node JSON-RPC URL. */
  readonly nodeUrl: string;
  /** Relay multiaddrs ending in `/p2p/<peer id>`. */
  readonly relays: readonly string[];
  /** Prove transactions on this device (off for a local network, which accepts fake proofs). */
  readonly proverEnabled: boolean;
  /** Show raw identifiers (peer ids, keys, commitments) inline across the app. */
  readonly technicalDetails: boolean;
}

export interface Profile {
  readonly name: string;
  /** Public half of the approval signing key (the secret stays in the main process). */
  readonly signerKey: Point;
  /** The member's own Aztec account (pays for deployments). */
  readonly account: string;
  readonly createdAt: number;
}

export type ServiceStatus = "stopped" | "starting" | "running" | "restarting" | "failed";

export interface KeeperState {
  readonly status: ServiceStatus;
  readonly identity?: string;
  readonly transport?: string;
  readonly peerId?: string;
  readonly addresses?: readonly string[];
  readonly version?: string;
  readonly error?: string;
}

export interface EngineState {
  readonly status: ServiceStatus;
  readonly error?: string;
}

export interface NodeStatus {
  readonly url: string;
  readonly reachable: boolean;
  readonly nodeVersion?: string;
  readonly l1ChainId?: number;
  readonly rollupVersion?: number;
  readonly blockNumber?: number;
  readonly checkedAt: number;
  readonly error?: string;
}

export interface VaultSummary {
  readonly address: string;
  readonly name: string;
  readonly threshold: number;
  readonly viewingThreshold: number;
  readonly members: number;
  readonly myIndex: number;
  readonly foundedAt: number;
}

export interface AppState {
  readonly profile: Profile | null;
  readonly settings: Settings;
  readonly keeper: KeeperState;
  readonly engine: EngineState;
  readonly vaults: readonly VaultSummary[];
  readonly dataDir: string;
  readonly platform: string;
}

export interface InvitePreview {
  readonly lobby: string;
  readonly name: string;
  readonly hostName: string;
  readonly threshold: number;
  readonly viewingThreshold: number;
  readonly network: string;
  /** Whether the invite's network matches this app's node. */
  readonly sameNetwork: boolean;
  readonly host: {
    readonly identity: string;
    readonly peerId: string;
    readonly addresses: readonly string[];
  };
}

export interface LobbyMember {
  readonly index: number;
  readonly name: string;
  readonly signerKey: Point;
  readonly identity: string;
  readonly peerId: string;
  readonly addresses: readonly string[];
  readonly you: boolean;
}

export type LobbyStatus = "open" | "started" | "closed";

export interface LobbyView {
  readonly lobby: string;
  readonly role: "host" | "guest";
  readonly status: LobbyStatus;
  readonly name: string;
  readonly threshold: number;
  readonly viewingThreshold: number;
  readonly network: string;
  readonly members: readonly LobbyMember[];
  readonly invite?: string;
  /** Present once the host started: the DKG session everyone joins. */
  readonly session?: string;
}

export type FoundingStepId = "agree" | "dkg" | "derive" | "register" | "deploy" | "verify";
export type StepStatus = "pending" | "active" | "done" | "failed";

export interface FoundingStep {
  readonly id: FoundingStepId;
  readonly status: StepStatus;
  readonly startedAt?: number;
  readonly finishedAt?: number;
  /** Short human detail, e.g. "Round 2 of 3" or the deployment tx hash. */
  readonly detail?: string;
}

export interface FoundingState {
  readonly lobby: string;
  readonly steps: readonly FoundingStep[];
  readonly vault?: string;
  readonly error?: {
    readonly step: FoundingStepId;
    readonly message: string;
    readonly blamed?: readonly string[];
  };
  readonly done: boolean;
}

export interface CommitteeMember {
  readonly index: number;
  readonly name: string;
  readonly signerKey: Point;
  readonly verificationShare: Point;
  readonly identity: string;
  readonly transport: string;
  readonly peerId: string;
  readonly addresses: readonly string[];
  readonly you: boolean;
}

export interface VaultDetails {
  readonly summary: VaultSummary;
  readonly contract: {
    readonly address: string;
    readonly preaddress: string;
    readonly partialAddress: string;
    readonly classId: string;
    readonly salt: string;
    readonly deployer: string;
    readonly initializationHash: string;
    readonly publicKeys: Readonly<Record<string, string>>;
  };
  readonly config: {
    readonly version: number;
    readonly threshold: number;
    readonly viewingThreshold: number;
    readonly signerCount: number;
    readonly signerSetCommitment: string;
    readonly committeeCommitment: string;
    readonly matchesSetup: boolean;
    readonly readAt: number;
  } | null;
  readonly committee: {
    readonly epoch: number;
    readonly viewingThreshold: number;
    readonly groupKey: Point;
    readonly members: readonly CommitteeMember[];
    readonly serving: boolean;
    readonly keeperStatus: string;
  };
  readonly founding: {
    readonly session: string;
    readonly transcript?: string;
    readonly deployTx?: string;
  };
}

export interface AuditEntry {
  readonly at: number;
  readonly requester: string;
  readonly requesterName?: string;
  readonly purpose: string;
  readonly points: number;
  readonly outcome: string;
}

export interface PxeStatus {
  readonly syncedBlock?: number;
  readonly proverEnabled: boolean;
  readonly backend: string;
  readonly dataDir: string;
}

export type AppEvent =
  | { readonly type: "state"; readonly state: AppState }
  | { readonly type: "lobby"; readonly lobby: LobbyView }
  | { readonly type: "founding"; readonly founding: FoundingState }
  | { readonly type: "node"; readonly node: NodeStatus }
  | { readonly type: "vault-activity"; readonly vault: string }
  | { readonly type: "execution"; readonly execution: ExecutionProgress };

// Phase 11: assets, proposals, execution.

export interface TokenInfo {
  readonly address: string;
  readonly name: string;
  readonly symbol: string;
  readonly decimals: number;
}

export interface TokenBalance {
  readonly token: TokenInfo;
  /** Private balance of the vault, base units, decimal string. */
  readonly balance?: string;
  readonly error?: string;
}

/** What a proposal is for, carried as JSON in the proposal's `summary` (at most 4 KiB). */
export type ProposalIntent =
  | {
      readonly kind: "transfer";
      readonly token: string;
      readonly to: string;
      /** Base units, decimal string. */
      readonly amount: string;
      readonly memo?: string;
    }
  | {
      readonly kind: "calls";
      readonly calls: readonly {
        readonly to: string;
        readonly fn: string;
        readonly args: readonly string[];
      }[];
      readonly memo?: string;
    }
  | { readonly kind: "cancel"; readonly cancels: string; readonly memo?: string };

export type ProposalStatus =
  | "collecting"
  | "ready"
  | "executed"
  | "cancelled"
  | "expired"
  | "stale";

export interface ProposalApproval {
  readonly slot: number;
  readonly index: number;
  readonly name: string;
  readonly valid: boolean;
}

export interface DecodedCall {
  readonly to: string;
  readonly toLabel?: string;
  readonly fn: string;
  readonly args: readonly string[];
  readonly isStatic: boolean;
}

export interface ProposalView {
  readonly id: string;
  readonly vault: string;
  readonly intent: ProposalIntent;
  readonly proposer: { readonly index: number; readonly name: string };
  readonly status: ProposalStatus;
  readonly threshold: number;
  readonly approvals: readonly ProposalApproval[];
  /** Slot (0-based) of this member, and whether it has approved. */
  readonly mySlot: number;
  readonly approvedByMe: boolean;
  readonly configVersion: number;
  readonly currentConfigVersion: number;
  readonly expiresAt: number;
  readonly txNonce: string;
  readonly outerHash: string;
  readonly approvalMessage: string;
  readonly feeMethod: string;
  readonly calls: readonly DecodedCall[];
  /** For transfers: the token as this member knows it. */
  readonly token?: TokenInfo;
  readonly execution?: { readonly txHash: string; readonly at: number };
  /** Set on a proposal a cancel proposal targets, and on the cancel proposal itself. */
  readonly cancelledBy?: string;
}

export interface ProposalPreview {
  readonly ok: boolean;
  readonly calls: readonly DecodedCall[];
  readonly gas?: { readonly da: number; readonly l2: number };
  readonly error?: string;
}

export interface ContractFunction {
  readonly name: string;
  readonly kind: "private" | "public" | "utility";
  readonly isStatic: boolean;
  readonly params: readonly { readonly name: string; readonly type: string }[];
}

export interface MemberReachability {
  readonly index: number;
  readonly name: string;
  readonly identity: string;
  readonly peerId: string;
  readonly you: boolean;
  readonly connected: boolean;
  readonly relayed: boolean;
  readonly lastSeen: number | null;
  readonly rttMs: number | null;
}

export type ActivityEntry =
  | { readonly kind: "proposal"; readonly at: number; readonly proposal: ProposalView }
  | {
      readonly kind: "received";
      readonly at: number;
      readonly token: TokenInfo;
      readonly from: string;
      readonly amount: string;
      readonly txHash: string;
      readonly block: number;
    };

export interface Contact {
  readonly address: string;
  readonly label: string;
}

/** Progress of an execution on this device. */
export interface ExecutionProgress {
  readonly vault: string;
  readonly proposal: string;
  readonly phase: "rebuilding" | "proving" | "sending" | "mined" | "failed";
  readonly error?: string;
  readonly txHash?: string;
}
