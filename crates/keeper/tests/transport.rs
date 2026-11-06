//! The Noise TCP transport: encrypted frames between allowed peers, large frames, and refusals.
#![allow(clippy::unwrap_used)]

use std::{net::SocketAddr, time::Duration};

use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use tecpan_crypto::envelope::TransportSecret;
use tecpan_keeper::transport::{Peer, TcpTransport, Transport};
use tecpan_wire::IdentityPublic;

fn key(seed: u64) -> TransportSecret {
    TransportSecret::generate(&mut ChaCha20Rng::seed_from_u64(seed)).0
}

fn peer(key: &TransportSecret, transport: &TcpTransport) -> Peer {
    Peer {
        identity: IdentityPublic([0; 32]),
        transport: key.public().unwrap().0,
        addresses: vec![format!("tcp://{}", transport.local_addr())],
    }
}

const LOCAL: SocketAddr = SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 0);

#[tokio::test]
async fn allowed_peers_exchange_frames_both_ways() {
    let (ka, kb) = (key(1), key(2));
    let (a, mut a_in) = TcpTransport::bind(LOCAL, &ka).await.unwrap();
    let (b, mut b_in) = TcpTransport::bind(LOCAL, &kb).await.unwrap();
    a.set_allowed(vec![peer(&kb, &b)]);
    b.set_allowed(vec![peer(&ka, &a)]);

    let big = vec![7u8; 300_000];
    a.send(peer(&kb, &b), big.clone()).await.unwrap();
    let frame = tokio::time::timeout(Duration::from_secs(5), b_in.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(frame.bytes, big);
    assert_eq!(frame.transport, ka.public().unwrap().0);

    // The reply reuses the connection.
    b.send(peer(&ka, &a), b"pong".to_vec()).await.unwrap();
    let reply = tokio::time::timeout(Duration::from_secs(5), a_in.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reply.bytes, b"pong");
}

#[tokio::test]
async fn unknown_peers_are_refused() {
    let (ka, kb) = (key(3), key(4));
    let (a, _) = TcpTransport::bind(LOCAL, &ka).await.unwrap();
    let (b, mut b_in) = TcpTransport::bind(LOCAL, &kb).await.unwrap();
    // `b` allows nobody: the connection is dropped after the handshake.
    a.set_allowed(vec![peer(&kb, &b)]);
    let _ = a.send(peer(&kb, &b), b"hello".to_vec()).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(300), b_in.recv())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn a_peer_with_an_unexpected_key_is_not_trusted() {
    let (ka, kb, impostor) = (key(5), key(6), key(7));
    let (a, _) = TcpTransport::bind(LOCAL, &ka).await.unwrap();
    let (b, _) = TcpTransport::bind(LOCAL, &kb).await.unwrap();
    b.set_allowed(vec![peer(&ka, &a)]);
    // `a` expects `impostor` at `b`'s address.
    let mut expected = peer(&impostor, &b);
    expected.addresses = vec![format!("tcp://{}", b.local_addr())];
    assert!(a.send(expected, b"x".to_vec()).await.is_err());
}
