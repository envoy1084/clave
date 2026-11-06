//! Connection gate of the keeper's libp2p node. Members of the keeper's committees and ceremonies, lobby peers and
//! configured relays may always connect. While the keeper hosts an open lobby, anyone may connect so that co-founders
//! can join; the node then delivers their frames only on the lobby lane, rate limited. Otherwise every other
//! connection is denied before any protocol runs.

use std::{
    collections::HashSet,
    convert::Infallible,
    task::{Context, Poll},
};

use libp2p::{
    Multiaddr, PeerId,
    core::{Endpoint, transport::PortUse},
    swarm::{
        ConnectionDenied, ConnectionId, FromSwarm, NetworkBehaviour, THandler, THandlerInEvent,
        THandlerOutEvent, ToSwarm, dummy,
    },
};

/// Why a connection was denied.
#[derive(Debug)]
struct NotAdmitted(PeerId);

impl std::fmt::Display for NotAdmitted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "peer {} is not admitted", self.0)
    }
}

impl std::error::Error for NotAdmitted {}

/// Admission state: allowed peers, relays, and whether an open lobby admits strangers.
#[derive(Default)]
pub(super) struct Gate {
    allowed: HashSet<PeerId>,
    relays: HashSet<PeerId>,
    lobby_open: bool,
}

impl Gate {
    pub(super) fn add_relay(&mut self, relay: PeerId) {
        self.relays.insert(relay);
    }

    /// Replaces the allowed peers and the lobby flag.
    pub(super) fn set(&mut self, allowed: HashSet<PeerId>, lobby_open: bool) {
        self.allowed = allowed;
        self.lobby_open = lobby_open;
    }

    /// Whether `peer` is a member, lobby peer or relay (as opposed to a stranger admitted by an open lobby).
    pub(super) fn is_known(&self, peer: &PeerId) -> bool {
        self.allowed.contains(peer) || self.relays.contains(peer)
    }

    /// Whether strangers are admitted.
    pub(super) fn lobby_open(&self) -> bool {
        self.lobby_open
    }

    /// Whether a connection with `peer` may exist.
    pub(super) fn admits(&self, peer: &PeerId) -> bool {
        self.lobby_open || self.is_known(peer)
    }

    fn enforce(&self, peer: &PeerId) -> Result<(), ConnectionDenied> {
        if self.admits(peer) {
            Ok(())
        } else {
            Err(ConnectionDenied::new(NotAdmitted(*peer)))
        }
    }
}

impl NetworkBehaviour for Gate {
    type ConnectionHandler = dummy::ConnectionHandler;
    type ToSwarm = Infallible;

    fn handle_established_inbound_connection(
        &mut self,
        _: ConnectionId,
        peer: PeerId,
        _: &Multiaddr,
        _: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        self.enforce(&peer)?;
        Ok(dummy::ConnectionHandler)
    }

    fn handle_pending_outbound_connection(
        &mut self,
        _: ConnectionId,
        peer: Option<PeerId>,
        _: &[Multiaddr],
        _: Endpoint,
    ) -> Result<Vec<Multiaddr>, ConnectionDenied> {
        if let Some(peer) = peer {
            self.enforce(&peer)?;
        }
        Ok(Vec::new())
    }

    fn handle_established_outbound_connection(
        &mut self,
        _: ConnectionId,
        peer: PeerId,
        _: &Multiaddr,
        _: Endpoint,
        _: PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        self.enforce(&peer)?;
        Ok(dummy::ConnectionHandler)
    }

    fn on_swarm_event(&mut self, _: FromSwarm) {}

    fn on_connection_handler_event(
        &mut self,
        _: PeerId,
        _: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        match event {}
    }

    fn poll(&mut self, _: &mut Context<'_>) -> Poll<ToSwarm<Infallible, THandlerInEvent<Self>>> {
        Poll::Pending
    }
}
