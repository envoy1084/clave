//! Daemon result shapes the CLI reads (`architecture/cli.md`, Shapes). Every field is defaulted so a daemon that adds
//! or omits optional fields still decodes; `--json` prints the daemon's own JSON, not these structs.

use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct DaemonInfo {
    pub(crate) version: String,
    pub(crate) pid: u32,
    pub(crate) profile: String,
    pub(crate) profile_dir: String,
    pub(crate) started_at: u64,
    pub(crate) node: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Settings {
    pub(crate) node_url: String,
    pub(crate) relays: Vec<String>,
    pub(crate) prover_enabled: bool,
    pub(crate) unlock_timeout_secs: u64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Point {
    pub(crate) x: String,
    pub(crate) y: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Profile {
    pub(crate) name: String,
    pub(crate) signer_key: Point,
    pub(crate) account: String,
    pub(crate) created_at: u64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct SignerStatus {
    pub(crate) locked: bool,
    pub(crate) unlocked_until: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct KeeperState {
    pub(crate) status: String,
    pub(crate) peer_id: Option<String>,
    pub(crate) identity: Option<String>,
    pub(crate) addresses: Vec<String>,
    pub(crate) version: Option<String>,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct EngineState {
    pub(crate) status: String,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct NodeStatus {
    pub(crate) url: String,
    pub(crate) reachable: bool,
    pub(crate) node_version: Option<String>,
    pub(crate) l1_chain_id: Option<u64>,
    pub(crate) rollup_version: Option<u64>,
    pub(crate) block_number: Option<u64>,
    pub(crate) checked_at: u64,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct PxeStatus {
    pub(crate) synced_block: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct VaultSummary {
    pub(crate) address: String,
    pub(crate) name: String,
    pub(crate) alias: Option<String>,
    pub(crate) threshold: u32,
    pub(crate) viewing_threshold: u32,
    pub(crate) members: u32,
    pub(crate) my_index: u32,
    pub(crate) founded_at: u64,
    pub(crate) is_default: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Status {
    pub(crate) profile: Option<Profile>,
    pub(crate) settings: Settings,
    pub(crate) keeper: KeeperState,
    pub(crate) engine: EngineState,
    pub(crate) node: NodeStatus,
    pub(crate) pxe: PxeStatus,
    pub(crate) signer: SignerStatus,
    pub(crate) vaults: Vec<VaultSummary>,
    pub(crate) default_vault: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct TokenInfo {
    pub(crate) address: String,
    pub(crate) name: String,
    pub(crate) symbol: String,
    pub(crate) decimals: u32,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct TokenBalance {
    pub(crate) token: TokenInfo,
    pub(crate) balance: Option<String>,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Contact {
    pub(crate) address: String,
    pub(crate) label: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Vote {
    pub(crate) slot: u32,
    pub(crate) index: u32,
    pub(crate) name: String,
    pub(crate) valid: bool,
    pub(crate) reason: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Proposer {
    pub(crate) index: u32,
    pub(crate) name: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct DecodedCall {
    pub(crate) to: String,
    pub(crate) to_label: Option<String>,
    #[serde(rename = "fn")]
    pub(crate) function: String,
    pub(crate) args: Vec<String>,
    pub(crate) is_static: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Execution {
    pub(crate) tx_hash: String,
    pub(crate) at: u64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct ProposalView {
    pub(crate) id: String,
    pub(crate) vault: String,
    /// `{ kind: "transfer" | "calls" | "cancel", … }`, kept as JSON.
    pub(crate) intent: Value,
    pub(crate) proposer: Proposer,
    pub(crate) status: String,
    pub(crate) threshold: u32,
    pub(crate) members: u32,
    pub(crate) approvals: Vec<Vote>,
    pub(crate) rejections: Vec<Vote>,
    pub(crate) my_slot: u32,
    pub(crate) approved_by_me: bool,
    pub(crate) rejected_by_me: bool,
    pub(crate) config_version: u64,
    pub(crate) current_config_version: u64,
    pub(crate) expires_at: u64,
    pub(crate) tx_nonce: String,
    pub(crate) outer_hash: String,
    pub(crate) approval_message: String,
    pub(crate) fee_method: String,
    pub(crate) calls: Vec<DecodedCall>,
    pub(crate) token: Option<TokenInfo>,
    pub(crate) execution: Option<Execution>,
    pub(crate) cancelled_by: Option<String>,
}

impl ProposalView {
    pub(crate) fn valid_approvals(&self) -> usize {
        self.approvals.iter().filter(|v| v.valid).count()
    }

    pub(crate) fn valid_rejections(&self) -> usize {
        self.rejections.iter().filter(|v| v.valid).count()
    }

    pub(crate) fn memo(&self) -> Option<&str> {
        self.intent
            .get("memo")
            .and_then(Value::as_str)
            .filter(|m| !m.is_empty())
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Checks {
    pub(crate) rebuilt: bool,
    pub(crate) outer_hash: String,
    pub(crate) config_current: bool,
    pub(crate) not_expired: bool,
    pub(crate) failure: Option<String>,
}

impl Checks {
    pub(crate) fn passed(&self) -> bool {
        self.rebuilt && self.config_current && self.not_expired && self.failure.is_none()
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Recipient {
    pub(crate) address: String,
    pub(crate) label: Option<String>,
    pub(crate) source: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Gas {
    pub(crate) da: u64,
    pub(crate) l2: u64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Simulation {
    pub(crate) ok: bool,
    pub(crate) gas: Option<Gas>,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Signs {
    pub(crate) message: String,
    pub(crate) formula: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Review {
    pub(crate) proposal: ProposalView,
    pub(crate) vault_name: String,
    pub(crate) my_name: String,
    pub(crate) checks: Checks,
    pub(crate) action: String,
    pub(crate) recipient: Option<Recipient>,
    pub(crate) simulation: Simulation,
    /// What an approval signs.
    pub(crate) signs: Signs,
    /// What a rejection signs.
    pub(crate) rejects: Signs,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct ProposalPreview {
    pub(crate) ok: bool,
    pub(crate) calls: Vec<DecodedCall>,
    pub(crate) gas: Option<Gas>,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct LobbyMember {
    pub(crate) index: u32,
    pub(crate) name: String,
    pub(crate) peer_id: String,
    pub(crate) you: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct LobbyView {
    pub(crate) lobby: String,
    pub(crate) role: String,
    pub(crate) status: String,
    pub(crate) name: String,
    pub(crate) threshold: u32,
    pub(crate) viewing_threshold: u32,
    pub(crate) network: String,
    pub(crate) members: Vec<LobbyMember>,
    pub(crate) invite: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct InvitePreview {
    pub(crate) lobby: String,
    pub(crate) name: String,
    pub(crate) host_name: String,
    pub(crate) threshold: u32,
    pub(crate) viewing_threshold: u32,
    pub(crate) network: String,
    pub(crate) same_network: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct FoundingStep {
    pub(crate) id: String,
    pub(crate) status: String,
    pub(crate) started_at: Option<u64>,
    pub(crate) finished_at: Option<u64>,
    pub(crate) detail: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct FoundingError {
    pub(crate) step: String,
    pub(crate) message: String,
    pub(crate) blamed: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct FoundingState {
    pub(crate) lobby: String,
    pub(crate) steps: Vec<FoundingStep>,
    pub(crate) vault: Option<String>,
    pub(crate) error: Option<FoundingError>,
    pub(crate) done: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct MemberReachability {
    pub(crate) index: u32,
    pub(crate) name: String,
    pub(crate) peer_id: String,
    pub(crate) you: bool,
    pub(crate) connected: bool,
    pub(crate) relayed: bool,
    pub(crate) last_seen: Option<u64>,
    pub(crate) rtt_ms: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct AuditEntry {
    pub(crate) at: u64,
    pub(crate) requester: String,
    pub(crate) requester_name: Option<String>,
    pub(crate) purpose: String,
    pub(crate) points: u64,
    pub(crate) outcome: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct ContractFunction {
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) is_static: bool,
    pub(crate) params: Vec<FunctionParam>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct FunctionParam {
    pub(crate) name: String,
    #[serde(rename = "type")]
    pub(crate) ty: String,
}
