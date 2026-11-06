//! Keepers over libp2p on localhost: DKG, threshold ECDH and replication; a reshare with a joining member; outsiders
//! refused; strangers founding a vault through a lobby; a keeper reachable only through a relay circuit; a member that was offline catching up from its relay
//! mailbox.
#![allow(
    clippy::unwrap_used,
    clippy::missing_panics_doc,
    clippy::too_many_lines
)]

mod common;

use std::{path::Path, sync::Arc, time::Duration};

use common::{ecdh, eventually, manifest_of, point, run_dkg_among};
use libp2p::{Multiaddr, identity::Keypair, multiaddr::Protocol};
use tecpan_keeper::{
    Clock, Keeper, KeeperConfig, KeeperParts, LobbyOpen, LobbyStatus, NewMember, ReshareRequest,
    SystemClock,
    custody::{KeeperKeys, MasterKey, SecretStore},
    mailbox::MailboxService,
    store::Store,
    transport::{P2pConfig, P2pTransport, Peer, RelayMailbox, Transport},
};
use tecpan_relay::{Limits, Relay, RelayConfig, store::MailboxStore};
use tecpan_wire::{
    Header, Message, SignedMessage, encode, encode_body,
    messages::{Manifest, Proposal},
};
use tempfile::TempDir;

struct Node {
    keeper: Keeper,
    transport: Arc<P2pTransport>,
    dir: TempDir,
    master: [u8; 32],
}

fn config() -> KeeperConfig {
    KeeperConfig {
        ecdh_timeout: Duration::from_secs(5),
        ..KeeperConfig::default()
    }
}

fn p2p(relays: &[Multiaddr], direct: bool) -> P2pConfig {
    P2pConfig {
        listen: if direct {
            P2pConfig::default().listen
        } else {
            Vec::new()
        },
        relays: relays.to_vec(),
        request_timeout: Duration::from_secs(5),
        ready_timeout: Duration::from_secs(5),
        ..P2pConfig::default()
    }
}

async fn start(
    dir: &Path,
    master: [u8; 32],
    relays: &[Multiaddr],
    direct: bool,
) -> (Keeper, Arc<P2pTransport>) {
    let secrets = SecretStore::open(dir.join("secrets"), &MasterKey::from_bytes(master)).unwrap();
    let keys = KeeperKeys::load_or_create(&secrets).unwrap();
    let store = Store::open(&dir.join("keeper.db")).unwrap();
    let (transport, inbound) = P2pTransport::start(&keys.identity.seed(), p2p(relays, direct))
        .await
        .unwrap();
    let mailbox: Option<Arc<dyn MailboxService>> = (!relays.is_empty())
        .then(|| Arc::new(RelayMailbox::new(Arc::clone(&transport))) as Arc<dyn MailboxService>);
    let keeper = Keeper::start(KeeperParts {
        keys,
        secrets,
        store,
        transport: Box::new(Arc::clone(&transport)),
        inbound,
        clock: Arc::new(SystemClock),
        config: config(),
        addresses: transport.addresses().to_vec(),
        mailbox,
    })
    .unwrap();
    (keeper, transport)
}

async fn node(seed: u8, relays: &[Multiaddr], direct: bool) -> Node {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();
    let dir = tempfile::tempdir().unwrap();
    let master = [seed; 32];
    let (keeper, transport) = start(dir.path(), master, relays, direct).await;
    Node {
        keeper,
        transport,
        dir,
        master,
    }
}

async fn vault_of(nodes: &[&Node]) -> [u8; 32] {
    manifest_for(nodes).await.vault
}

async fn manifest_for(nodes: &[&Node]) -> Manifest {
    let keepers: Vec<&Keeper> = nodes.iter().map(|n| &n.keeper).collect();
    let result = run_dkg_among(&keepers, 2, [1; 32]).await;
    let (manifest, _) = manifest_of(&keepers, &result, 3);
    for k in &keepers {
        k.register_vault([1; 32], manifest.clone()).await.unwrap();
    }
    manifest
}

fn proposal(summary: &str) -> Vec<u8> {
    encode_body(&Message::Proposal(Proposal {
        config_version: 0,
        outer_hash: [3; 32],
        expires_at: 1_800_000_000,
        tx_nonce: [4; 32],
        fee_method: 0,
        payload: vec![1, 2, 3],
        summary: summary.into(),
        proposer: 1,
    }))
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn keepers_run_dkg_ecdh_and_replication_over_libp2p() {
    let nodes = [
        node(1, &[], true).await,
        node(2, &[], true).await,
        node(3, &[], true).await,
    ];
    assert!(
        nodes[0]
            .keeper
            .addresses()
            .iter()
            .any(|a| a.contains("/quic-v1"))
    );
    let refs: Vec<&Node> = nodes.iter().collect();
    let vault = vault_of(&refs).await;
    let points = [point(1), point(2)];
    let first = ecdh(&nodes[0].keeper, vault, 0, &points).await;
    assert_eq!(ecdh(&nodes[2].keeper, vault, 0, &points).await, first);

    let body = proposal("over libp2p");
    nodes[1].keeper.put_proposal(vault, &body).await.unwrap();
    for n in [&nodes[0], &nodes[2]] {
        eventually(async || {
            (n.keeper.proposals(vault).unwrap() == vec![body.clone()]).then_some(())
        })
        .await;
    }
}

// A joining member is in no committee of the others; the member that finishes first must still deliver its last
// messages to it.
#[tokio::test(flavor = "multi_thread")]
async fn a_joining_member_completes_a_reshare_over_libp2p() {
    let nodes = [
        node(1, &[], true).await,
        node(2, &[], true).await,
        node(3, &[], true).await,
    ];
    let refs: Vec<&Node> = nodes.iter().collect();
    let manifest = manifest_for(&refs).await;
    let newcomer = node(4, &[], true).await;
    let request = ReshareRequest {
        session: [2; 32],
        vault: manifest.vault,
        current: None,
        dealers: vec![1, 2],
        new_threshold: 2,
        new_spending_threshold: 2,
        new_members: [&nodes[0], &nodes[1], &newcomer]
            .iter()
            .zip(&manifest.members)
            .map(|(n, member)| NewMember {
                identity: n.keeper.identity(),
                transport: n.keeper.transport_key(),
                addresses: n.keeper.addresses().to_vec(),
                signer_key: member.signer_key,
            })
            .collect(),
    };
    for n in &nodes[..2] {
        n.keeper.start_reshare(request.clone()).await.unwrap();
    }
    newcomer
        .keeper
        .start_reshare(ReshareRequest {
            current: Some(manifest.clone()),
            ..request
        })
        .await
        .unwrap();
    for keeper in [&nodes[0].keeper, &nodes[1].keeper, &newcomer.keeper] {
        eventually(async || (common::status(keeper, [2; 32]) == "completed").then_some(())).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn peers_outside_every_committee_cannot_connect() {
    let nodes = [node(1, &[], true).await, node(2, &[], true).await];
    let refs: Vec<&Node> = nodes.iter().collect();
    vault_of(&refs).await;
    let outsider = node(9, &[], true).await;
    let target = Peer {
        identity: nodes[0].keeper.identity(),
        transport: nodes[0].keeper.transport_key(),
        addresses: nodes[0].keeper.addresses().to_vec(),
    };
    outsider.transport.set_allowed(vec![target.clone()]);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let refused = Transport::send(&outsider.transport, target, proposal("intrusion")).await;
    assert!(refused.is_err());
}

async fn relay() -> (Relay, Multiaddr) {
    let config = RelayConfig {
        listen: vec![
            "/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap(),
            "/ip4/127.0.0.1/tcp/0".parse().unwrap(),
        ],
        ..RelayConfig::default()
    };
    let relay = Relay::start(
        Keypair::generate_ed25519(),
        MailboxStore::in_memory(Limits::default()).unwrap(),
        config,
    )
    .await
    .unwrap();
    let address = relay.addresses[0]
        .clone()
        .with(Protocol::P2p(relay.peer_id));
    (relay, address)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_keeper_without_a_direct_address_is_reached_through_the_relay() {
    let (relay, address) = relay().await;
    let relays = [address];
    let hidden = node(2, &relays, false).await;
    assert!(!hidden.keeper.addresses().is_empty());
    assert!(
        hidden
            .keeper
            .addresses()
            .iter()
            .all(|a| a.contains("/p2p-circuit"))
    );
    let nodes = [
        node(1, &relays, true).await,
        hidden,
        node(3, &relays, true).await,
    ];
    let refs: Vec<&Node> = nodes.iter().collect();
    let vault = vault_of(&refs).await;
    let points = [point(5)];
    assert_eq!(
        ecdh(&nodes[1].keeper, vault, 0, &points).await,
        ecdh(&nodes[0].keeper, vault, 0, &points).await
    );
    assert!(
        relay
            .metrics
            .reservations
            .load(std::sync::atomic::Ordering::Relaxed)
            >= 3
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_that_was_offline_reads_its_relay_mailbox() {
    let (relay, address) = relay().await;
    let relays = [address];
    let nodes = [
        node(1, &relays, true).await,
        node(2, &relays, true).await,
        node(3, &relays, true).await,
    ];
    let refs: Vec<&Node> = nodes.iter().collect();
    let vault = vault_of(&refs).await;

    // Member 3 goes offline; member 1 proposes. Member 2 receives it directly, member 3's copy goes to the relay.
    nodes[2].transport.stop();
    tokio::time::sleep(Duration::from_millis(200)).await;
    let body = proposal("while you were away");
    nodes[0].keeper.put_proposal(vault, &body).await.unwrap();
    eventually(async || (relay.store.usage().unwrap().envelopes >= 1).then_some(())).await;
    eventually(async || {
        (nodes[1].keeper.proposals(vault).unwrap() == vec![body.clone()]).then_some(())
    })
    .await;

    // Member 3 comes back (new process, same data) and reads its mailbox.
    let (back, _transport) = start(nodes[2].dir.path(), nodes[2].master, &relays, true).await;
    assert!(back.proposals(vault).unwrap().is_empty());
    let delivered = back.poll_mailbox().await.unwrap();
    assert!(delivered >= 1);
    assert_eq!(back.proposals(vault).unwrap(), vec![body]);
    assert_eq!(relay.store.usage().unwrap().envelopes, 0);
}

/// Whether `target` refuses `frame` from `outsider` (connection denied, or not acknowledged).
async fn refused_frame(outsider: &Node, target: &Node, frame: Vec<u8>) -> bool {
    let peer = Peer {
        identity: target.keeper.identity(),
        transport: target.keeper.transport_key(),
        addresses: target.keeper.addresses().to_vec(),
    };
    outsider.transport.set_allowed(vec![peer.clone()]);
    tokio::time::sleep(Duration::from_millis(50)).await;
    Transport::send(&outsider.transport, peer, frame)
        .await
        .is_err()
}

async fn refused(outsider: &Node, target: &Node) -> bool {
    refused_frame(outsider, target, proposal("intrusion")).await
}

/// A lobby frame (it travels on the lobby lane) signed by `node`.
fn lobby_frame(node: &Node, lobby: [u8; 16]) -> Vec<u8> {
    let secrets = SecretStore::open(
        node.dir.path().join("secrets"),
        &MasterKey::from_bytes(node.master),
    )
    .unwrap();
    let keys = KeeperKeys::load_or_create(&secrets).unwrap();
    let header = Header {
        scope: tecpan_wire::lobby::scope(&lobby),
        epoch: 0,
        id: [9; 16],
        created_at: SystemClock.now(),
    };
    let body = Message::LobbyLeave(tecpan_wire::lobby::LobbyLeave { lobby });
    encode(&SignedMessage::sign(&keys.identity, header, body).unwrap()).unwrap()
}

// Keepers in no committee meet in a lobby: the host admits strangers only while it is open and only on the lobby
// lane; the plan reaches every guest, the lobby closes to outsiders, and the members run the DKG with the plan.
#[tokio::test(flavor = "multi_thread")]
async fn strangers_found_a_vault_through_a_lobby_over_libp2p() {
    let host = node(1, &[], true).await;
    let guests = [node(2, &[], true).await, node(3, &[], true).await];
    let outsider = node(9, &[], true).await;
    assert!(refused(&outsider, &host).await, "no lobby open yet");

    let (lobby, invite) = host
        .keeper
        .open_lobby(LobbyOpen {
            name: "Treasury".into(),
            threshold: 2,
            viewing_threshold: 2,
            network: "local-network".into(),
            member_name: "Ada".into(),
            signer_key: point(100),
        })
        .await
        .unwrap();
    for (guest, (name, seed)) in guests.iter().zip([("Grace", 101), ("Alan", 102)]) {
        guest
            .keeper
            .join_lobby(&invite, name.into(), point(seed))
            .await
            .unwrap();
    }
    for keeper in [&host.keeper, &guests[0].keeper, &guests[1].keeper] {
        eventually(async || {
            (keeper.lobby(lobby, false).await.unwrap().members.len() == 3).then_some(())
        })
        .await;
    }
    // While the lobby is open a stranger connects and is heard on the lobby lane, but on no other lane.
    assert!(!refused_frame(&outsider, &host, lobby_frame(&outsider, lobby)).await);
    assert!(
        refused(&outsider, &host).await,
        "a non-lobby frame from a stranger"
    );

    let secrets = b"protocol secrets".to_vec();
    let started = host
        .keeper
        .start_lobby(lobby, 2, 2, [5; 32], &secrets)
        .await
        .unwrap();
    let session = started.plan.unwrap().session;
    for guest in &guests {
        let plan = eventually(async || {
            let info = guest.keeper.lobby(lobby, true).await.unwrap();
            (info.status == LobbyStatus::Started).then(|| info.plan.unwrap())
        })
        .await;
        assert_eq!(plan.session, session);
        assert_eq!(plan.secrets.as_deref(), Some(&secrets));
    }
    assert!(
        refused_frame(&outsider, &host, lobby_frame(&outsider, lobby)).await,
        "the lobby is started"
    );

    // Each engine starts the DKG itself, in the plan's member order.
    let order: Vec<&Keeper> = started
        .members
        .iter()
        .map(|m| {
            [&host, &guests[0], &guests[1]]
                .into_iter()
                .find(|n| n.keeper.identity() == m.participant.identity)
                .map(|n| &n.keeper)
                .unwrap()
        })
        .collect();
    let result = run_dkg_among(&order, 2, session).await;
    assert_eq!(result.verification_shares.len(), 3);
}
