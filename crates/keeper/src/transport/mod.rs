//! How keepers reach each other. The keeper core only needs "send these bytes to that member" and a stream of inbound
//! frames; frames are always `SignedMessage`s, so authenticity never depends on the transport. Implementations: an
//! in-memory network for tests and a Noise-encrypted TCP transport; libp2p replaces the latter in Phase 9.

mod memory;
mod noise;
mod tcp;

use std::{future::Future, pin::Pin};

pub use memory::{MemoryNetwork, MemoryTransport};
pub use tcp::TcpTransport;
use tecpan_wire::{IdentityPublic, messages::Member};

use crate::Result;

/// A reachable keeper.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Peer {
    /// Signing identity.
    pub identity: IdentityPublic,
    /// X25519 transport key (authenticates the channel).
    pub transport: [u8; 32],
    /// Network addresses, e.g. `tcp://127.0.0.1:4100`.
    pub addresses: Vec<String>,
}

impl From<&Member> for Peer {
    fn from(member: &Member) -> Self {
        Self {
            identity: member.identity,
            transport: member.transport,
            addresses: member.addresses.clone(),
        }
    }
}

/// What authenticated the channel a frame arrived on; the frame's signer must match it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// A channel keyed by the sender's X25519 transport key (in-memory network, relay mailbox envelopes).
    Key([u8; 32]),
    /// A channel authenticated by the sender's identity (libp2p, where the peer id is the keeper identity).
    Peer(IdentityPublic),
}

/// A frame received from a peer.
#[derive(Debug)]
pub struct Inbound {
    /// How the sender was authenticated.
    pub channel: Channel,
    /// Encoded `SignedMessage`.
    pub bytes: Vec<u8>,
}

/// Boxed send future.
pub type SendFuture<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

/// Outbound side of a transport. Inbound frames arrive on the channel returned when the transport is created.
pub trait Transport: Send + Sync + 'static {
    /// Delivers `frame` to `peer` (best effort; protocols time out and retry).
    fn send(&self, peer: Peer, frame: Vec<u8>) -> SendFuture<'_>;

    /// Replaces the set of transport keys allowed to connect (current committees and ceremonies).
    fn set_allowed(&self, peers: Vec<Peer>);
}
