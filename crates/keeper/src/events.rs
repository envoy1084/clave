//! Events the keeper streams to its owner's engine.

use serde::Serialize;

/// Something the member's engine should know about (streamed over `events.subscribe`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Event {
    /// A ceremony changed status.
    Ceremony {
        /// Session id (hex).
        session: String,
        /// `dkg` or `reshare`.
        kind: String,
        /// `running`, `completed` or `aborted`.
        status: String,
    },
    /// A member's partials failed verification.
    InvalidPartials {
        /// Vault (hex).
        vault: String,
        /// Member index.
        member: u32,
    },
    /// A new committee became active for a vault.
    CommitteeActivated {
        /// Vault (hex).
        vault: String,
        /// Committee epoch.
        epoch: u32,
    },
    /// This keeper is no longer in the vault's committee; its share was destroyed.
    Removed {
        /// Vault (hex).
        vault: String,
    },
    /// The observed on-chain committee matches nothing this keeper knows.
    ConfigMismatch {
        /// Vault (hex).
        vault: String,
        /// Observed config version.
        version: u32,
    },
    /// A vault transaction that matches no known proposal execution.
    UnexpectedSpend {
        /// Vault (hex).
        vault: String,
        /// Transaction hash (hex).
        tx_hash: String,
    },
    /// A proposal arrived or was stored.
    Proposal {
        /// Vault (hex).
        vault: String,
        /// Proposal id (hex).
        id: String,
    },
    /// An approval arrived or was stored.
    Approval {
        /// Vault (hex).
        vault: String,
        /// Proposal id (hex).
        proposal: String,
        /// Signer slot.
        slot: u32,
    },
    /// An execution receipt was stored (put locally or replicated from a member).
    Receipt {
        /// Vault (hex).
        vault: String,
        /// Proposal id (hex).
        proposal: String,
        /// Transaction hash (hex).
        tx_hash: String,
    },
    /// A lobby changed: opened or joined, roster changed, started or closed.
    Lobby {
        /// Lobby id (hex).
        lobby: String,
        /// `open`, `started` or `closed`.
        status: String,
        /// Members on the roster.
        members: u32,
    },
}
