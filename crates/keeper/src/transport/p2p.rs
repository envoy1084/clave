use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use futures::StreamExt;
use libp2p::{
    Multiaddr, PeerId, Swarm, dcutr, identify, identity,
    multiaddr::Protocol,
    noise, ping, relay,
    request_response::{self, OutboundRequestId, ProtocolSupport},
    swarm::{ConnectionId, NetworkBehaviour, SwarmEvent},
    tcp, yamux,
};
use tecpan_wire::{
    IdentityPublic, Message, decode,
    p2p::{CEREMONY, ECDH, FrameCodec, IDENTIFY_VERSION, LOBBY, MAILBOX, SYNC},
};
use tokio::{
    sync::{mpsc, oneshot},
    time::Instant,
};

use super::{Channel, Inbound, Peer, PeerStatus, SendFuture, Transport, gate::Gate};
use crate::{Clock, Error, Result, SystemClock};

/// Most addresses a keeper advertises (the manifest's limit).
const MAX_ADVERTISED: usize = 8;
/// How long to keep collecting listen addresses after every listener has reported one.
const LISTEN_SETTLE: Duration = Duration::from_millis(150);
/// Lobby requests a non-member may send per minute.
const LOBBY_REQUESTS_PER_MINUTE: u32 = 10;
/// Most non-members whose lobby request rate is tracked at once.
const MAX_LOBBY_RATE_PEERS: usize = 4096;

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

/// The keeper identity behind a peer id (an Ed25519 key inlined in the id), if any.
fn identity_of(peer: &PeerId) -> Option<IdentityPublic> {
    let hash = peer.as_ref();
    if hash.code() != 0 {
        return None;
    }
    let key = identity::PublicKey::try_decode_protobuf(hash.digest())
        .ok()?
        .try_into_ed25519()
        .ok()?;
    Some(IdentityPublic(key.to_bytes()))
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    gate: Gate,
    relay_client: relay::client::Behaviour,
    dcutr: dcutr::Behaviour,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
    ecdh: request_response::Behaviour<FrameCodec>,
    ceremony: request_response::Behaviour<FrameCodec>,
    sync: request_response::Behaviour<FrameCodec>,
    lobby: request_response::Behaviour<FrameCodec>,
    mailbox: request_response::Behaviour<FrameCodec>,
}

/// What the node observed of one peer's connections.
#[derive(Default)]
struct Link {
    /// Open connections and whether each is a relay circuit.
    connections: HashMap<ConnectionId, bool>,
    /// Unix seconds of the last established connection or received frame.
    last_seen: Option<u64>,
    /// Last ping round trip in milliseconds.
    rtt_ms: Option<u64>,
}

/// Links by peer, shared between the swarm task (writer) and [`Transport::peer_status`] (reader).
type Links = Arc<Mutex<HashMap<PeerId, Link>>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lane {
    Ecdh,
    Ceremony,
    Sync,
    Lobby,
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
        Ok(
            Message::LobbyJoin(_)
            | Message::LobbyState(_)
            | Message::LobbyPlan(_)
            | Message::LobbyLeave(_),
        ) => Lane::Lobby,
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
    Allow {
        peers: Vec<Peer>,
        lobby_open: bool,
    },
    Stop,
}

/// The keeper's libp2p node. The peer id is the keeper identity, so a connection authenticates the member; only
/// current committee, ceremony and lobby peers and configured relays may connect, plus anyone while this keeper hosts
/// an open lobby (whose frames are then accepted on the lobby lane only).
pub struct P2pTransport {
    commands: mpsc::UnboundedSender<Command>,
    links: Links,
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
                gate: Gate::default(),
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
                lobby: keeper_protocol(LOBBY),
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
            swarm.behaviour_mut().gate.add_relay(id);
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
        let addresses = advertised(&config, &relays, &listening, &reserved);
        tracing::info!(peer = %local, ?addresses, "p2p node ready");

        let (commands, rx) = mpsc::unbounded_channel();
        let (inbound, inbound_rx) = mpsc::unbounded_channel();
        let relay_ids: Vec<PeerId> = relays.iter().map(|(id, _)| *id).collect();
        let links = Links::default();
        tokio::spawn(run(
            swarm,
            rx,
            inbound,
            relay_ids.iter().copied().collect(),
            Arc::clone(&links),
        ));
        Ok((
            Arc::new(Self {
                commands,
                links,
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
        self.set_admission(peers, false);
    }

    fn set_admission(&self, peers: Vec<Peer>, lobby_open: bool) {
        let _ = self.commands.send(Command::Allow { peers, lobby_open });
    }

    fn peer_status(&self, peers: &[IdentityPublic]) -> Vec<PeerStatus> {
        let links = self.links.lock().unwrap_or_else(PoisonError::into_inner);
        peers
            .iter()
            .map(|identity| {
                let Some(link) = peer_id(identity).ok().and_then(|id| links.get(&id)) else {
                    return PeerStatus::default();
                };
                let connected = !link.connections.is_empty();
                PeerStatus {
                    connected,
                    relayed: connected && link.connections.values().all(|relayed| *relayed),
                    last_seen: link.last_seen,
                    rtt_ms: link.rtt_ms,
                }
            })
            .collect()
    }
}

/// Addresses to advertise: direct addresses first (peers try them before relaying), relay circuits always included as
/// the fallback, at most [`MAX_ADVERTISED`].
fn advertised(
    config: &P2pConfig,
    relays: &[(PeerId, Multiaddr)],
    listening: &[Multiaddr],
    reserved: &HashSet<PeerId>,
) -> Vec<String> {
    let circuits: Vec<String> = relays
        .iter()
        .filter(|(id, _)| reserved.contains(id))
        .map(|(_, relay)| relay.clone().with(Protocol::P2pCircuit).to_string())
        .take(MAX_ADVERTISED)
        .collect();
    // A wildcard listener reports loopback too. Advertised to another host, a loopback address dials the dialer
    // itself, and libp2p settles a dial on its first transport success, so that self-connection (wrong peer id)
    // would fail the whole dial, relay circuit included. Loopback is advertised only by a loopback-only keeper.
    let routable = listening.iter().any(|a| !is_loopback(a));
    let mut addresses: Vec<String> = config
        .external
        .iter()
        .chain(listening.iter().filter(|a| !(routable && is_loopback(a))))
        .map(ToString::to_string)
        .collect();
    addresses.dedup();
    addresses.truncate(MAX_ADVERTISED - circuits.len());
    addresses.extend(circuits);
    addresses
}

/// Records a peer's advertised addresses, skipping any of our own: a member behind another NAT may advertise the same
/// private address and port, and dialing it would reach this keeper and fail the whole dial.
fn add_addresses(swarm: &mut Swarm<Behaviour>, id: PeerId, addresses: &[String]) {
    for address in addresses.iter().filter_map(|a| a.parse::<Multiaddr>().ok()) {
        if !swarm.listeners().any(|own| *own == address) {
            swarm.add_peer_address(id, address);
        }
    }
}

fn is_loopback(address: &Multiaddr) -> bool {
    address.iter().any(|p| match p {
        Protocol::Ip4(ip) => ip.is_loopback(),
        Protocol::Ip6(ip) => ip.is_loopback(),
        _ => false,
    })
}

/// Waits until every listen address is bound and every relay accepted a reservation, or the timeout passes.
async fn wait_ready(
    swarm: &mut Swarm<Behaviour>,
    config: &P2pConfig,
    relays: &[(PeerId, Multiaddr)],
) -> (Vec<Multiaddr>, HashSet<PeerId>) {
    let deadline = Instant::now() + config.ready_timeout;
    let mut listening = Vec::new();
    let mut listeners = HashSet::new();
    let mut reserved = HashSet::new();
    // A wildcard listener (0.0.0.0) reports one address per interface; once every listener has reported, keep
    // collecting briefly so all of an interface's addresses (and both transports) make it into what we advertise.
    let mut settle: Option<Instant> = None;
    loop {
        let all_listening = listeners.len() >= config.listen.len();
        if all_listening && reserved.len() >= relays.len() {
            let until = *settle.get_or_insert_with(|| Instant::now() + LISTEN_SETTLE);
            if Instant::now() >= until {
                break;
            }
        }
        let wake = settle.map_or(deadline, |until| until.min(deadline));
        if Instant::now() >= deadline {
            break;
        }
        let Ok(Some(event)) = tokio::time::timeout_at(wake, swarm.next()).await else {
            if Instant::now() >= deadline {
                break;
            }
            continue;
        };
        match event {
            SwarmEvent::NewListenAddr {
                listener_id,
                address,
            } if !address.iter().any(|p| p == Protocol::P2pCircuit) => {
                listeners.insert(listener_id);
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
    /// Lobby requests of non-members in the current minute: window start and count.
    lobby_rate: HashMap<PeerId, (Instant, u32)>,
    /// Connection state per peer, read by [`Transport::peer_status`].
    links: Links,
}

impl Pending {
    fn links(&self) -> MutexGuard<'_, HashMap<PeerId, Link>> {
        self.links.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Records a frame received from `peer` (only peers with a tracked connection).
    fn seen(&self, peer: &PeerId) {
        if let Some(link) = self.links().get_mut(peer) {
            link.last_seen = Some(SystemClock.now());
        }
    }

    /// Forgets peers that are neither members nor connected (strangers met in a lobby, removed members).
    fn prune_links(&self) {
        self.links()
            .retain(|id, link| !link.connections.is_empty() || self.members.contains_key(id));
    }

    /// Counts a lobby request from a non-member; `false` once it exceeded its budget for the minute.
    fn lobby_request_allowed(&mut self, peer: PeerId) -> bool {
        let now = Instant::now();
        let minute = Duration::from_mins(1);
        if self.lobby_rate.len() >= MAX_LOBBY_RATE_PEERS {
            self.lobby_rate
                .retain(|_, (start, _)| now.duration_since(*start) < minute);
            if self.lobby_rate.len() >= MAX_LOBBY_RATE_PEERS {
                return false;
            }
        }
        let (start, count) = self.lobby_rate.entry(peer).or_insert((now, 0));
        if now.duration_since(*start) >= minute {
            *start = now;
            *count = 0;
        }
        *count += 1;
        *count <= LOBBY_REQUESTS_PER_MINUTE
    }
}

async fn run(
    mut swarm: Swarm<Behaviour>,
    mut commands: mpsc::UnboundedReceiver<Command>,
    inbound: mpsc::UnboundedSender<Inbound>,
    relays: HashSet<PeerId>,
    links: Links,
) {
    let mut pending = Pending {
        links,
        ..Pending::default()
    };
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
            add_addresses(swarm, id, &peer.addresses);
            let behaviour = swarm.behaviour_mut();
            let request = match lane(&frame) {
                Lane::Ecdh => behaviour.ecdh.send_request(&id, frame),
                Lane::Ceremony => behaviour.ceremony.send_request(&id, frame),
                Lane::Sync => behaviour.sync.send_request(&id, frame),
                Lane::Lobby => behaviour.lobby.send_request(&id, frame),
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
        Command::Allow { peers, lobby_open } => {
            let mut next = HashMap::new();
            for peer in peers {
                if let Ok(id) = peer_id(&peer.identity) {
                    add_addresses(swarm, id, &peer.addresses);
                    next.insert(id, peer.identity);
                }
            }
            swarm
                .behaviour_mut()
                .gate
                .set(next.keys().copied().collect(), lobby_open);
            if !lobby_open {
                pending.lobby_rate.clear();
            }
            // Close what the gate no longer admits: removed members, and strangers once no lobby is open.
            let closing: Vec<PeerId> = swarm
                .connected_peers()
                .filter(|id| !relays.contains(id) && !swarm.behaviour().gate.admits(id))
                .copied()
                .collect();
            for id in closing {
                let _ = swarm.disconnect_peer_id(id);
            }
            pending.members = next;
            pending.prune_links();
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
        SwarmEvent::Behaviour(BehaviourEvent::Lobby(event)) => {
            on_lane(swarm, pending, inbound, Lane::Lobby, event);
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
        SwarmEvent::Behaviour(BehaviourEvent::Ping(ping::Event {
            peer,
            result: Ok(rtt),
            ..
        })) => {
            if let Some(link) = pending.links().get_mut(&peer) {
                link.rtt_ms = Some(u64::try_from(rtt.as_millis()).unwrap_or(u64::MAX));
            }
        }
        SwarmEvent::ConnectionEstablished {
            peer_id,
            connection_id,
            endpoint,
            ..
        } => {
            tracing::debug!(%peer_id, relayed = endpoint.is_relayed(), "connected");
            let mut links = pending.links();
            let link = links.entry(peer_id).or_default();
            link.connections
                .insert(connection_id, endpoint.is_relayed());
            link.last_seen = Some(SystemClock.now());
        }
        SwarmEvent::ConnectionClosed {
            peer_id,
            connection_id,
            ..
        } => {
            if let Some(link) = pending.links().get_mut(&peer_id) {
                link.connections.remove(&connection_id);
            }
            pending.prune_links();
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
            pending.seen(&peer);
            // Members are delivered on every lane. A stranger (admitted while a lobby is open) only on the lobby lane,
            // with lobby messages, within its rate. The keeper still checks that the frame's signer is this peer.
            let identity = match pending.members.get(&peer).copied() {
                Some(identity) => identity,
                None if lane == Lane::Lobby
                    && swarm.behaviour().gate.lobby_open()
                    && self::lane(&request) == Lane::Lobby
                    && pending.lobby_request_allowed(peer) =>
                {
                    let Some(identity) = identity_of(&peer) else {
                        return;
                    };
                    identity
                }
                None => {
                    tracing::debug!(%peer, ?lane, "dropped a request from a non-member");
                    return;
                }
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
                Lane::Lobby => &mut behaviour.lobby,
            };
            let _ = protocol.send_response(channel, Vec::new());
        }
        request_response::Event::Message {
            peer,
            message: request_response::Message::Response { request_id, .. },
            ..
        } => {
            pending.seen(&peer);
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

#[cfg(test)]
mod tests {
    use tecpan_wire::KeeperIdentity;

    use super::*;

    #[test]
    fn a_peer_id_yields_its_keeper_identity() {
        let identity = KeeperIdentity::from_seed(&[7; 32]).public();
        let id = peer_id(&identity).unwrap_or_else(|_| PeerId::random());
        assert_eq!(identity_of(&id), Some(identity));
        assert_eq!(identity_of(&PeerId::random()), None);
    }

    #[test]
    fn non_members_get_ten_lobby_requests_a_minute() {
        let mut pending = Pending::default();
        let stranger = PeerId::random();
        for _ in 0..LOBBY_REQUESTS_PER_MINUTE {
            assert!(pending.lobby_request_allowed(stranger));
        }
        assert!(!pending.lobby_request_allowed(stranger));
        assert!(pending.lobby_request_allowed(PeerId::random()));
    }
}
