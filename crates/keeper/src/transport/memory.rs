use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, PoisonError},
};

use tecpan_wire::IdentityPublic;
use tokio::sync::mpsc;

use super::{Channel, Inbound, Peer, PeerStatus, SendFuture, Transport};
use crate::{Clock, Error, SystemClock};

/// An endpoint's channel and transport key.
type Endpoint = (mpsc::UnboundedSender<Inbound>, [u8; 32]);
type Filter = Box<dyn Fn(&IdentityPublic, &IdentityPublic, &[u8]) -> bool + Send + Sync>;

/// In-process network for tests: every endpoint is a channel; an optional filter drops frames (partitions, faults).
#[derive(Default)]
pub struct MemoryNetwork {
    endpoints: Mutex<HashMap<IdentityPublic, Endpoint>>,
    filter: Mutex<Option<Filter>>,
    /// Unix seconds of the last frame delivered, by (receiver, sender).
    seen: Mutex<HashMap<(IdentityPublic, IdentityPublic), u64>>,
}

impl MemoryNetwork {
    /// A new, empty network.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Joins the network as `identity` with transport key `transport`.
    pub fn endpoint(
        self: &Arc<Self>,
        identity: IdentityPublic,
        transport: [u8; 32],
    ) -> (MemoryTransport, mpsc::UnboundedReceiver<Inbound>) {
        let (tx, rx) = mpsc::unbounded_channel();
        self.endpoints
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(identity, (tx, transport));
        (
            MemoryTransport {
                network: Arc::clone(self),
                me: identity,
                allowed: Mutex::new(HashSet::new()),
            },
            rx,
        )
    }

    /// Removes an endpoint (a crashed or stopped keeper).
    pub fn leave(&self, identity: &IdentityPublic) {
        self.endpoints
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(identity);
    }

    /// Installs a filter `(from, to, frame) -> deliver?`.
    pub fn set_filter(
        &self,
        filter: impl Fn(&IdentityPublic, &IdentityPublic, &[u8]) -> bool + Send + Sync + 'static,
    ) {
        *self.filter.lock().unwrap_or_else(PoisonError::into_inner) = Some(Box::new(filter));
    }

    /// Removes the filter.
    pub fn clear_filter(&self) {
        *self.filter.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }

    /// Whether frames from `from` currently reach `to`: `to` is joined and the filter lets an (empty) frame through.
    fn reaches(&self, from: &IdentityPublic, to: &IdentityPublic) -> bool {
        self.endpoints
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains_key(to)
            && self
                .filter
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_ref()
                .is_none_or(|f| f(from, to, &[]))
    }
}

/// One keeper's endpoint on a [`MemoryNetwork`].
pub struct MemoryTransport {
    network: Arc<MemoryNetwork>,
    me: IdentityPublic,
    allowed: Mutex<HashSet<[u8; 32]>>,
}

impl Transport for MemoryTransport {
    fn send(&self, peer: Peer, frame: Vec<u8>) -> SendFuture<'_> {
        let result = (|| {
            let deliver = self
                .network
                .filter
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_ref()
                .is_none_or(|f| f(&self.me, &peer.identity, &frame));
            if !deliver {
                return Err(Error::Transport("peer unreachable".into()));
            }
            let endpoints = self
                .network
                .endpoints
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let my_transport = endpoints
                .get(&self.me)
                .map(|(_, t)| *t)
                .ok_or_else(|| Error::Transport("not joined".into()))?;
            let (tx, transport) = endpoints
                .get(&peer.identity)
                .ok_or_else(|| Error::Transport("peer unreachable".into()))?;
            if *transport != peer.transport {
                return Err(Error::Transport("peer transport key mismatch".into()));
            }
            tx.send(Inbound {
                channel: Channel::Key(my_transport),
                bytes: frame,
            })
            .map_err(|_| Error::Transport("peer stopped".into()))?;
            self.network
                .seen
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert((peer.identity, self.me), SystemClock.now());
            Ok(())
        })();
        Box::pin(async move { result })
    }

    fn set_allowed(&self, peers: Vec<Peer>) {
        // Delivery is by identity; the keeper still checks that the sender's identity maps to the channel's key.
        *self.allowed.lock().unwrap_or_else(PoisonError::into_inner) =
            peers.iter().map(|p| p.transport).collect();
    }

    /// Connected while the peer is joined and the filter lets frames through both ways; never relayed, no round trip.
    fn peer_status(&self, peers: &[IdentityPublic]) -> Vec<PeerStatus> {
        let seen = self
            .network
            .seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        peers
            .iter()
            .map(|peer| PeerStatus {
                connected: self.network.reaches(&self.me, peer)
                    && self.network.reaches(peer, &self.me),
                relayed: false,
                last_seen: seen.get(&(self.me, *peer)).copied(),
                rtt_ms: None,
            })
            .collect()
    }
}
