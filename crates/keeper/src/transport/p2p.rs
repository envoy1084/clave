use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use futures::StreamExt;
use libp2p::{
    Multiaddr, PeerId, Swarm, allow_block_list, dcutr, identify, identity,
    multiaddr::Protocol,
    noise, ping, relay,
    request_response::{self, OutboundRequestId, ProtocolSupport},
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux,
};
use tecpan_wire::{
    IdentityPublic, Message, decode,
    p2p::{CEREMONY, ECDH, FrameCodec, IDENTIFY_VERSION, MAILBOX, SYNC},
};
use tokio::{
    sync::{mpsc, oneshot},
    time::Instant,
};

use super::{Channel, Inbound, Peer, SendFuture, Transport};
use crate::{Error, Result};

/// Most addresses a keeper advertises (the manifest's limit).
const MAX_ADVERTISED: usize = 8;

/// libp2p settings of a keeper.
#[derive(Clone, Debug)]
pub struct P2pConfig {
    /// Listen addresses (QUIC and TCP).
    pub listen: Vec<Multiaddr>,
    /// Relays, each ending in `/p2p/<relay peer id>`: the keeper reserves a circuit at each and uses them as mailboxes.
    pub relays: Vec<Multiaddr>,
    /// Extra addresses to advertise (e.g. a public address behind port forwarding).
    pub external: Vec<Multiaddr>,
    /// Timeout of one request.
    pub request_timeout: Duration,
    /// How long `start` waits for listeners and relay reservations.
    pub ready_timeout: Duration,
}

impl Default for P2pConfig {
    fn default() -> Self {
        Self {
            listen: vec![
                "/ip4/127.0.0.1/udp/0/quic-v1"
                    .parse()
                    .unwrap_or_else(|_| Multiaddr::empty()),
                "/ip4/127.0.0.1/tcp/0"
                    .parse()
                    .unwrap_or_else(|_| Multiaddr::empty()),
            ],
            relays: Vec::new(),
            external: Vec::new(),
            request_timeout: Duration::from_secs(10),
            ready_timeout: Duration::from_secs(10),
        }
    }
}

/// The libp2p peer id of a keeper identity (its Ed25519 key).
///
/// # Errors
/// [`Error::Invalid`] if the identity is not a valid Ed25519 key.
pub fn peer_id(identity: &IdentityPublic) -> Result<PeerId> {
    let key = identity::ed25519::PublicKey::try_from_bytes(&identity.0)
        .map_err(|e| Error::Invalid(e.to_string()))?;
    Ok(identity::PublicKey::from(key).to_peer_id())
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    allow: allow_block_list::Behaviour<allow_block_list::AllowedPeers>,
    relay_client: relay::client::Behaviour,
    dcutr: dcutr::Behaviour,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
    ecdh: request_response::Behaviour<FrameCodec>,
    ceremony: request_response::Behaviour<FrameCodec>,
    sync: request_response::Behaviour<FrameCodec>,
    mailbox: request_response::Behaviour<FrameCodec>,
}

#[derive(Clone, Copy, Debug)]
enum Lane {
    Ecdh,
    Ceremony,
    Sync,
}

fn lane(frame: &[u8]) -> Lane {
    match decode(frame).map(|m| m.body) {
        Ok(Message::EcdhRequest(_) | Message::EcdhResponse(_)) => Lane::Ecdh,
        Ok(
            Message::Proposal(_)
            | Message::Approval(_)
            | Message::ExecutionReceipt(_)
            | Message::SyncRequest(_),
        ) => Lane::Sync,
        _ => Lane::Ceremony,
    }
}

enum Command {
    Send {
        peer: Peer,
        frame: Vec<u8>,
        reply: oneshot::Sender<Result<()>>,
    },
    Mailbox {
        relay: PeerId,
        request: Vec<u8>,
        reply: oneshot::Sender<Result<Vec<u8>>>,
    },
    Allow(Vec<Peer>),
    Stop,
}

/// The keeper's libp2p node. The peer id is the keeper identity, so a connection authenticates the member; only
/// current committee and ceremony members and configured relays may connect.
pub struct P2pTransport {
    commands: mpsc::UnboundedSender<Command>,
    peer_id: PeerId,
    relays: Vec<PeerId>,
    addresses: Vec<String>,
}

impl P2pTransport {
    /// Starts the node with the keeper's Ed25519 seed; returns once listeners and relay reservations are up (or the
    /// ready timeout passed).
    ///
    /// # Errors
    /// [`Error::Transport`] if the node cannot start or listen.
    pub async fn start(
        seed: &[u8; 32],
        config: P2pConfig,
    ) -> Result<(Arc<Self>, mpsc::UnboundedReceiver<Inbound>)> {
        let mut seed = *seed;
        let keypair = identity::Keypair::ed25519_from_bytes(&mut seed).map_err(transport_error)?;
        let local = keypair.public().to_peer_id();
        let timeout = config.request_timeout;
        let keeper_protocol = |protocol| {
            request_response::Behaviour::with_codec(
                FrameCodec::keeper(),
                [(protocol, ProtocolSupport::Full)],
                request_response::Config::default().with_request_timeout(timeout),
            )
        };
        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_tcp(
                tcp::Config::default().nodelay(true),
                noise::Config::new,
                yamux::Config::default,
            )
            .map_err(transport_error)?
            .with_quic()
            .with_dns()
            .map_err(transport_error)?
            .with_relay_client(noise::Config::new, yamux::Config::default)
            .map_err(transport_error)?
            .with_behaviour(|key, relay_client| Behaviour {
                allow: allow_block_list::Behaviour::default(),
                relay_client,
                dcutr: dcutr::Behaviour::new(key.public().to_peer_id()),
                identify: identify::Behaviour::new(identify::Config::new(
                    IDENTIFY_VERSION.into(),
                    key.public(),
                )),
                ping: ping::Behaviour::default(),
                ecdh: keeper_protocol(ECDH),
                ceremony: keeper_protocol(CEREMONY),
                sync: keeper_protocol(SYNC),
                mailbox: request_response::Behaviour::with_codec(
                    FrameCodec::mailbox(),
                    [(MAILBOX, ProtocolSupport::Outbound)],
                    request_response::Config::default().with_request_timeout(timeout * 3),
                ),
            })
            .map_err(transport_error)?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_mins(2)))
            .build();

        let mut relays = Vec::new();
        for relay in &config.relays {
            let Some(Protocol::P2p(id)) = relay.iter().last() else {
                return Err(Error::Invalid(format!(
                    "relay address {relay} must end with /p2p/<peer id>"
                )));
            };
            swarm.behaviour_mut().allow.allow_peer(id);
            swarm.add_peer_address(id, relay.clone());
            relays.push((id, relay.clone()));
        }
        for address in &config.listen {
            swarm.listen_on(address.clone()).map_err(transport_error)?;
        }
        for (_, relay) in &relays {
            swarm
                .listen_on(relay.clone().with(Protocol::P2pCircuit))
                .map_err(transport_error)?;
        }

        let (listening, reserved) = wait_ready(&mut swarm, &config, &relays).await;
        // Direct addresses first (peers try them before relaying), relay circuits always included as the fallback.
        let circuits: Vec<String> = relays
            .iter()
            .filter(|(id, _)| reserved.contains(id))
            .map(|(_, relay)| relay.clone().with(Protocol::P2pCircuit).to_string())
            .take(MAX_ADVERTISED)
            .collect();
        let mut addresses: Vec<String> = config
            .external
            .iter()
            .chain(&listening)
            .map(ToString::to_string)
            .collect();
        addresses.dedup();
        addresses.truncate(MAX_ADVERTISED - circuits.len());
        addresses.extend(circuits);
        tracing::info!(peer = %local, ?addresses, "p2p node ready");

        let (commands, rx) = mpsc::unbounded_channel();
        let (inbound, inbound_rx) = mpsc::unbounded_channel();
        let relay_ids: Vec<PeerId> = relays.iter().map(|(id, _)| *id).collect();
        tokio::spawn(run(swarm, rx, inbound, relay_ids.iter().copied().collect()));
        Ok((
            Arc::new(Self {
                commands,
                peer_id: local,
                relays: relay_ids,
                addresses,
            }),
            inbound_rx,
        ))
    }

    /// This node's peer id.
    #[must_use]
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    /// Addresses to publish in manifests: direct addresses first, then relay circuits.
    #[must_use]
    pub fn addresses(&self) -> &[String] {
        &self.addresses
    }

    /// Configured relays.
    #[must_use]
    pub fn relays(&self) -> &[PeerId] {
        &self.relays
    }

    /// Stops the node: closes every connection and ends the inbound stream (the keeper's loops end with it).
    pub fn stop(&self) {
        let _ = self.commands.send(Command::Stop);
    }

    /// Sends a mailbox request to `relay` and returns its raw answer.
    pub(super) async fn mailbox(&self, relay: PeerId, request: Vec<u8>) -> Result<Vec<u8>> {
        let (reply, answer) = oneshot::channel();
        self.commands
            .send(Command::Mailbox {
                relay,
                request,
                reply,
            })
            .map_err(|_| Error::Transport("p2p node stopped".into()))?;
        answer
            .await
            .map_err(|_| Error::Transport("p2p node stopped".into()))?
    }
}

impl Transport for Arc<P2pTransport> {
    fn send(&self, peer: Peer, frame: Vec<u8>) -> SendFuture<'_> {
        Box::pin(async move {
            let (reply, answer) = oneshot::channel();
            self.commands
                .send(Command::Send { peer, frame, reply })
                .map_err(|_| Error::Transport("p2p node stopped".into()))?;
            answer
                .await
                .map_err(|_| Error::Transport("p2p node stopped".into()))?
        })
    }

    fn set_allowed(&self, peers: Vec<Peer>) {
        let _ = self.commands.send(Command::Allow(peers));
    }
}

/// Waits until every listen address is bound and every relay accepted a reservation, or the timeout passes.
async fn wait_ready(
    swarm: &mut Swarm<Behaviour>,
    config: &P2pConfig,
    relays: &[(PeerId, Multiaddr)],
) -> (Vec<Multiaddr>, HashSet<PeerId>) {
    let deadline = Instant::now() + config.ready_timeout;
    let mut listening = Vec::new();
    let mut reserved = HashSet::new();
    while (listening.len() < config.listen.len() || reserved.len() < relays.len())
        && Instant::now() < deadline
    {
        let Ok(Some(event)) = tokio::time::timeout_at(deadline, swarm.next()).await else {
            break;
        };
        match event {
            SwarmEvent::NewListenAddr { address, .. }
                if !address.iter().any(|p| p == Protocol::P2pCircuit) =>
            {
                listening.push(address);
            }
            SwarmEvent::Behaviour(BehaviourEvent::RelayClient(
                relay::client::Event::ReservationReqAccepted { relay_peer_id, .. },
            )) => {
                reserved.insert(relay_peer_id);
            }
            _ => {}
        }
    }
    (listening, reserved)
}

#[derive(Default)]
struct Pending {
    sends: HashMap<OutboundRequestId, oneshot::Sender<Result<()>>>,
    mailbox: HashMap<OutboundRequestId, oneshot::Sender<Result<Vec<u8>>>>,
    /// Allowed members by peer id.
    members: HashMap<PeerId, IdentityPublic>,
}

async fn run(
    mut swarm: Swarm<Behaviour>,
    mut commands: mpsc::UnboundedReceiver<Command>,
    inbound: mpsc::UnboundedSender<Inbound>,
    relays: HashSet<PeerId>,
) {
    let mut pending = Pending::default();
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(Command::Stop) | None => break,
                Some(command) => on_command(&mut swarm, &mut pending, &relays, command),
            },
            Some(event) = swarm.next() => on_event(&mut swarm, &mut pending, &inbound, event),
        }
    }
}

fn on_command(
    swarm: &mut Swarm<Behaviour>,
    pending: &mut Pending,
    relays: &HashSet<PeerId>,
    command: Command,
) {
    match command {
        Command::Send { peer, frame, reply } => {
            let id = match peer_id(&peer.identity) {
                Ok(id) => id,
                Err(error) => {
                    let _ = reply.send(Err(error));
                    return;
                }
            };
            if !pending.members.contains_key(&id) {
                let _ = reply.send(Err(Error::Transport("peer is not in any committee".into())));
                return;
            }
            for address in peer
                .addresses
                .iter()
                .filter_map(|a| a.parse::<Multiaddr>().ok())
            {
                swarm.add_peer_address(id, address);
            }
            let behaviour = swarm.behaviour_mut();
            let request = match lane(&frame) {
                Lane::Ecdh => behaviour.ecdh.send_request(&id, frame),
                Lane::Ceremony => behaviour.ceremony.send_request(&id, frame),
                Lane::Sync => behaviour.sync.send_request(&id, frame),
            };
            pending.sends.insert(request, reply);
        }
        Command::Mailbox {
            relay,
            request,
            reply,
        } => {
            let request = swarm.behaviour_mut().mailbox.send_request(&relay, request);
            pending.mailbox.insert(request, reply);
        }
        Command::Stop => {}
        Command::Allow(peers) => {
            let mut next = HashMap::new();
            for peer in peers {
                if let Ok(id) = peer_id(&peer.identity) {
                    for address in peer
                        .addresses
                        .iter()
                        .filter_map(|a| a.parse::<Multiaddr>().ok())
                    {
                        swarm.add_peer_address(id, address);
                    }
                    next.insert(id, peer.identity);
                }
            }
            let behaviour = swarm.behaviour_mut();
            for removed in pending
                .members
                .keys()
                .filter(|id| !next.contains_key(id) && !relays.contains(id))
            {
                behaviour.allow.disallow_peer(*removed);
            }
            for added in next.keys() {
                behaviour.allow.allow_peer(*added);
            }
            pending.members = next;
        }
    }
}

fn on_event(
    swarm: &mut Swarm<Behaviour>,
    pending: &mut Pending,
    inbound: &mpsc::UnboundedSender<Inbound>,
    event: SwarmEvent<BehaviourEvent>,
) {
    match event {
        SwarmEvent::Behaviour(BehaviourEvent::Ecdh(event)) => {
            on_lane(swarm, pending, inbound, Lane::Ecdh, event);
        }
        SwarmEvent::Behaviour(BehaviourEvent::Ceremony(event)) => {
            on_lane(swarm, pending, inbound, Lane::Ceremony, event);
        }
        SwarmEvent::Behaviour(BehaviourEvent::Sync(event)) => {
            on_lane(swarm, pending, inbound, Lane::Sync, event);
        }
        SwarmEvent::Behaviour(BehaviourEvent::Mailbox(event)) => match event {
            request_response::Event::Message {
                message:
                    request_response::Message::Response {
                        request_id,
                        response,
                    },
                ..
            } => {
                if let Some(reply) = pending.mailbox.remove(&request_id) {
                    let _ = reply.send(Ok(response));
                }
            }
            request_response::Event::OutboundFailure {
                request_id, error, ..
            } => {
                if let Some(reply) = pending.mailbox.remove(&request_id) {
                    let _ = reply.send(Err(transport_error(error)));
                }
            }
            _ => {}
        },
        SwarmEvent::Behaviour(BehaviourEvent::Dcutr(event)) => {
            tracing::debug!(?event, "hole punching");
        }
        SwarmEvent::ConnectionEstablished {
            peer_id, endpoint, ..
        } => {
            tracing::debug!(%peer_id, relayed = endpoint.is_relayed(), "connected");
        }
        _ => {}
    }
}

/// One keeper protocol's events: deliver requests from members (and acknowledge them), settle our own requests.
fn on_lane(
    swarm: &mut Swarm<Behaviour>,
    pending: &mut Pending,
    inbound: &mpsc::UnboundedSender<Inbound>,
    lane: Lane,
    event: request_response::Event<Vec<u8>, Vec<u8>>,
) {
    match event {
        request_response::Event::Message {
            peer,
            message:
                request_response::Message::Request {
                    request, channel, ..
                },
            ..
        } => {
            // Only allowed members connect; the keeper still checks that the frame's signer is this peer.
            let Some(identity) = pending.members.get(&peer).copied() else {
                return;
            };
            let _ = inbound.send(Inbound {
                channel: Channel::Peer(identity),
                bytes: request,
            });
            let behaviour = swarm.behaviour_mut();
            let protocol = match lane {
                Lane::Ecdh => &mut behaviour.ecdh,
                Lane::Ceremony => &mut behaviour.ceremony,
                Lane::Sync => &mut behaviour.sync,
            };
            let _ = protocol.send_response(channel, Vec::new());
        }
        request_response::Event::Message {
            message: request_response::Message::Response { request_id, .. },
            ..
        } => {
            if let Some(reply) = pending.sends.remove(&request_id) {
                let _ = reply.send(Ok(()));
            }
        }
        request_response::Event::OutboundFailure {
            request_id, error, ..
        } => {
            if let Some(reply) = pending.sends.remove(&request_id) {
                let _ = reply.send(Err(transport_error(error)));
            }
        }
        request_response::Event::InboundFailure { peer, error, .. } => {
            tracing::debug!(%peer, %error, "inbound request failed");
        }
        request_response::Event::ResponseSent { .. } => {}
    }
}

fn transport_error(error: impl std::fmt::Display) -> Error {
    Error::Transport(error.to_string())
}
