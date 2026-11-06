//! Vault-founding lobbies on the in-memory network: hosting, joining with an invite, the live roster, the plan and its
//! sealed secrets, refused joins, leaving and closing, and a guest restart.
#![allow(clippy::unwrap_used, clippy::missing_panics_doc)]

mod common;

use common::{Harness, TestKeeper, eventually, point};
use tecpan_keeper::{
    Keeper, KeeperConfig, LobbyInfo, LobbyOpen, LobbyRole, LobbyStatus, events::Event,
};
use tecpan_wire::lobby::Invite;
use zeroize::Zeroizing;

const SECRETS: &[u8] = b"engine protocol secrets: signer seeds, deployment salt, contract class";

fn open_request(name: &str) -> LobbyOpen {
    LobbyOpen {
        name: "Treasury".into(),
        threshold: 2,
        viewing_threshold: 2,
        network: "local-network".into(),
        member_name: name.into(),
        signer_key: point(100),
    }
}

async fn info(keeper: &Keeper, lobby: [u8; 16]) -> LobbyInfo {
    keeper.lobby(lobby, true).await.unwrap()
}

async fn roster(keeper: &Keeper, lobby: [u8; 16]) -> Vec<String> {
    info(keeper, lobby)
        .await
        .members
        .iter()
        .map(|m| m.name.clone())
        .collect()
}

async fn wait_roster(keeper: &Keeper, lobby: [u8; 16], expected: &[&str]) {
    eventually(async || (roster(keeper, lobby).await == expected).then_some(())).await;
}

async fn wait_status(keeper: &Keeper, lobby: [u8; 16], status: LobbyStatus) {
    eventually(async || (info(keeper, lobby).await.status == status).then_some(())).await;
}

/// A host and two guests in an open lobby.
async fn lobby_of_three(harness: &Harness) -> ([TestKeeper; 3], [u8; 16], String) {
    let keepers = [harness.keeper(1), harness.keeper(2), harness.keeper(3)];
    let (lobby, invite) = keepers[0]
        .keeper
        .open_lobby(open_request("Ada"))
        .await
        .unwrap();
    for (k, (name, seed)) in keepers[1..].iter().zip([("Grace", 101), ("Alan", 102)]) {
        let joined = k
            .keeper
            .join_lobby(&invite, name.into(), point(seed))
            .await
            .unwrap();
        assert_eq!(joined, lobby);
    }
    for k in &keepers {
        wait_roster(&k.keeper, lobby, &["Ada", "Grace", "Alan"]).await;
    }
    (keepers, lobby, invite)
}

#[tokio::test(flavor = "multi_thread")]
async fn guests_see_the_roster_and_receive_the_same_plan_and_secrets() {
    let harness = Harness::new(KeeperConfig::default());
    let (keepers, lobby, invite) = lobby_of_three(&harness).await;
    let mut events = keepers[1].keeper.subscribe();

    let host = info(&keepers[0].keeper, lobby).await;
    assert_eq!(host.role, LobbyRole::Host);
    assert_eq!(host.status, LobbyStatus::Open);
    assert_eq!(host.invite.as_deref(), Some(invite.as_str()));
    let guest = info(&keepers[1].keeper, lobby).await;
    assert_eq!(guest.role, LobbyRole::Guest);
    assert_eq!(guest.host_name, "Ada");
    assert_eq!(guest.network, "local-network");
    assert!(guest.invite.is_none() && guest.plan.is_none());
    assert_eq!(guest.members, host.members);

    let salt = [0x07; 32];
    let started = keepers[0]
        .keeper
        .start_lobby(lobby, 2, 2, salt, SECRETS)
        .await
        .unwrap();
    assert_eq!(started.status, LobbyStatus::Started);
    assert!(started.invite.is_none());
    let plan = started.plan.unwrap();
    assert_eq!(plan.secrets.as_deref().map(Vec::as_slice), Some(SECRETS));

    for k in &keepers[1..] {
        wait_status(&k.keeper, lobby, LobbyStatus::Started).await;
        let mine = info(&k.keeper, lobby).await;
        let received = mine.plan.unwrap();
        assert_eq!(received.session, plan.session);
        assert_eq!(received.salt, salt);
        assert_eq!((received.threshold, received.viewing_threshold), (2, 2));
        assert_eq!(
            received.secrets.as_deref().map(Vec::as_slice),
            Some(SECRETS)
        );
        assert_eq!(mine.members, started.members);
    }
    // The guest's event stream reported the start.
    let members = eventually(async || match events.try_recv() {
        Ok(Event::Lobby {
            status, members, ..
        }) if status == "started" => Some(members),
        _ => None,
    })
    .await;
    assert_eq!(members, 3);
    // Listing never carries secrets.
    let listed = keepers[1].keeper.lobbies().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].plan.as_ref().unwrap().secrets.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn wrong_proofs_lobbies_and_invites_are_refused() {
    let harness = Harness::new(KeeperConfig::default());
    let host = harness.keeper(1);
    let guest = harness.keeper(2);
    let (lobby, invite) = host.keeper.open_lobby(open_request("Ada")).await.unwrap();
    let decoded = Invite::decode(&invite).unwrap();

    // Wrong secret: the host acknowledges the frame but refuses the proof.
    let mut forged = decoded.clone();
    forged.secret = Zeroizing::new([0x55; 32]);
    guest
        .keeper
        .join_lobby(&forged.encode().unwrap(), "Mallory".into(), point(101))
        .await
        .unwrap();
    // Another lobby id under the same host.
    let mut other = decoded.clone();
    other.lobby = [0x66; 16];
    let third = harness.keeper(3);
    third
        .keeper
        .join_lobby(&other.encode().unwrap(), "Eve".into(), point(102))
        .await
        .unwrap();
    // A damaged invite string does not decode.
    let tampered = format!("{}x", &invite[..invite.len() - 3]);
    assert!(
        harness
            .keeper(4)
            .keeper
            .join_lobby(&tampered, "Trudy".into(), point(103))
            .await
            .is_err()
    );
    // The host's own signing key cannot be reused by a guest.
    let fourth = harness.keeper(5);
    fourth
        .keeper
        .join_lobby(&invite, "Copycat".into(), point(100))
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(roster(&host.keeper, lobby).await, ["Ada"]);

    // A rightful join still works, and a stranger cannot join once the lobby started.
    let honest = harness.keeper(6);
    honest
        .keeper
        .join_lobby(&invite, "Grace".into(), point(104))
        .await
        .unwrap();
    wait_roster(&host.keeper, lobby, &["Ada", "Grace"]).await;
    host.keeper
        .start_lobby(lobby, 1, 2, [0; 32], SECRETS)
        .await
        .unwrap();
    let late = harness.keeper(7);
    late.keeper
        .join_lobby(&invite, "Late".into(), point(105))
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(roster(&host.keeper, lobby).await, ["Ada", "Grace"]);
    assert_eq!(
        info(&late.keeper, lobby).await.status,
        LobbyStatus::Open,
        "the late joiner never receives a plan"
    );
    // Thresholds that do not fit the roster are refused.
    let (second, _) = host.keeper.open_lobby(open_request("Ada")).await.unwrap();
    assert!(
        host.keeper
            .start_lobby(second, 1, 2, [0; 32], SECRETS)
            .await
            .is_err(),
        "a lobby of one cannot have a viewing threshold of 2"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn leaving_updates_the_roster_and_closing_reaches_the_guests() {
    let harness = Harness::new(KeeperConfig::default());
    let (keepers, lobby, invite) = lobby_of_three(&harness).await;

    keepers[2]
        .keeper
        .close_lobby(lobby, LobbyRole::Guest)
        .await
        .unwrap();
    assert_eq!(
        info(&keepers[2].keeper, lobby).await.status,
        LobbyStatus::Closed
    );
    wait_roster(&keepers[0].keeper, lobby, &["Ada", "Grace"]).await;
    wait_roster(&keepers[1].keeper, lobby, &["Ada", "Grace"]).await;
    // A guest cannot close, the host cannot leave.
    assert!(
        keepers[1]
            .keeper
            .close_lobby(lobby, LobbyRole::Host)
            .await
            .is_err()
    );
    assert!(
        keepers[0]
            .keeper
            .close_lobby(lobby, LobbyRole::Guest)
            .await
            .is_err()
    );
    // A guest cannot start.
    assert!(
        keepers[1]
            .keeper
            .start_lobby(lobby, 1, 2, [0; 32], SECRETS)
            .await
            .is_err()
    );

    keepers[0]
        .keeper
        .close_lobby(lobby, LobbyRole::Host)
        .await
        .unwrap();
    wait_status(&keepers[1].keeper, lobby, LobbyStatus::Closed).await;
    // Nobody joins a closed lobby.
    let late = harness.keeper(4);
    late.keeper
        .join_lobby(&invite, "Late".into(), point(110))
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(roster(&keepers[0].keeper, lobby).await, ["Ada", "Grace"]);
    assert!(
        keepers[0]
            .keeper
            .start_lobby(lobby, 1, 2, [0; 32], SECRETS)
            .await
            .is_err()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_restarted_guest_keeps_its_plan() {
    let harness = Harness::new(KeeperConfig::default());
    let (keepers, lobby, _) = lobby_of_three(&harness).await;
    let plan = keepers[0]
        .keeper
        .start_lobby(lobby, 2, 3, [0x09; 32], SECRETS)
        .await
        .unwrap()
        .plan
        .unwrap();
    wait_status(&keepers[1].keeper, lobby, LobbyStatus::Started).await;

    let restarted = harness.start(keepers[1].dir.path(), keepers[1].master);
    let after = info(&restarted, lobby).await;
    assert_eq!(after.status, LobbyStatus::Started);
    let reopened = after.plan.unwrap();
    assert_eq!(reopened.session, plan.session);
    assert_eq!(reopened.viewing_threshold, 3);
    assert_eq!(
        reopened.secrets.as_deref().map(Vec::as_slice),
        Some(SECRETS)
    );
    assert_eq!(roster(&restarted, lobby).await, ["Ada", "Grace", "Alan"]);
}
