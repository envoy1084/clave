//! Keepers on an in-memory network: DKG, threshold ECDH, reshare with activation and removal, malicious members,
//! rate limits, timeouts, restarts and proposal replication.
#![allow(
    clippy::unwrap_used,
    clippy::missing_panics_doc,
    clippy::too_many_lines
)]

mod common;

use std::time::Duration;

use common::{Harness, TestKeeper, ecdh, eventually, manifest, point, run_dkg, status};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use tecpan_crypto::{SecretScalar, ecdh::AddressSecretShare, schnorr, shamir::random_scalar};
use tecpan_keeper::{
    Error, KeeperConfig, NewMember, Observation, ReshareRequest, ReshareResult,
    ceremony::dkg::DkgParams,
    committee::Committee,
    events::Event,
    transport::{Peer, Transport},
};
use tecpan_wire::{
    Header, Message, SignedMessage, decode, encode, encode_body,
    messages::{Approval, EcdhPurpose, EcdhRequest, EcdhResponse, Proposal},
    types::{WirePoint, parse_field},
};

fn config() -> KeeperConfig {
    KeeperConfig {
        round_timeout_secs: 30,
        ecdh_timeout: Duration::from_millis(500),
        ..KeeperConfig::default()
    }
}

struct Vault {
    harness: Harness,
    keepers: Vec<TestKeeper>,
    address: [u8; 32],
    signers: Vec<SecretScalar>,
    manifest: tecpan_wire::messages::Manifest,
}

/// Three keepers, a 2-of-3 committee from a real DKG, registered as a vault.
async fn vault(config: KeeperConfig) -> Vault {
    let harness = Harness::new(config);
    let keepers: Vec<TestKeeper> = (1..=3).map(|i| harness.keeper(i)).collect();
    let refs: Vec<&TestKeeper> = keepers.iter().collect();
    let result = run_dkg(&refs, 2, [1; 32]).await;
    let (manifest, signers) = manifest(&refs, &result, 7);
    for k in &keepers {
        let info = k
            .keeper
            .register_vault([1; 32], manifest.clone())
            .await
            .unwrap();
        assert!(info.serving);
    }
    Vault {
        harness,
        address: manifest.vault,
        keepers,
        signers,
        manifest,
    }
}

fn peer(k: &TestKeeper) -> Peer {
    Peer {
        identity: k.keeper.identity(),
        transport: k.keeper.transport_key(),
        addresses: Vec::new(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn every_member_gathers_the_same_shared_secret() {
    let v = vault(config()).await;
    let points = [point(1), point(2)];
    let first = ecdh(&v.keepers[0].keeper, v.address, 0, &points).await;
    for k in &v.keepers[1..] {
        assert_eq!(ecdh(&k.keeper, v.address, 0, &points).await, first);
    }
    // Local requests and the peers' answers are audited.
    let audit = v.keepers[1].keeper.audit_log(v.address, 10).unwrap();
    assert!(
        audit
            .iter()
            .any(|e| e.requester == "local" && e.outcome == "served")
    );
    assert!(
        audit
            .iter()
            .any(|e| e.requester != "local" && e.outcome == "served")
    );
    // Wrong epochs and unknown vaults are refused.
    let wrong = v.keepers[0]
        .keeper
        .ecdh_batch(v.address, 1, EcdhPurpose::NoteDecryption, points.to_vec())
        .await;
    assert!(matches!(
        wrong,
        Err(Error::WrongEpoch {
            expected: 0,
            actual: 1
        })
    ));
    let unknown = v.keepers[0]
        .keeper
        .ecdh_batch([9; 32], 0, EcdhPurpose::NoteDecryption, points.to_vec())
        .await;
    assert!(matches!(unknown, Err(Error::UnknownVault)));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lying_member_is_detected_and_routed_around() {
    let v = vault(config()).await;
    let mut events = v.keepers[0].keeper.subscribe();
    // Member 2 is replaced by an impostor that answers with partials from a wrong share.
    let keys = v.keepers[1].keys();
    let (evil, mut inbox) = v
        .harness
        .net
        .endpoint(keys.identity.public(), keys.transport_public.0);
    let committee = Committee::new(v.manifest.clone()).unwrap();
    let target = peer(&v.keepers[0]);
    let clock = v.harness.clock.clone();
    tokio::spawn(async move {
        let mut rng = ChaCha20Rng::seed_from_u64(3);
        while let Some(frame) = inbox.recv().await {
            let message = decode(&frame.bytes).unwrap();
            let Message::EcdhRequest(request) = message.body else {
                continue;
            };
            let wrong =
                AddressSecretShare::new(2, &random_scalar(&mut rng), &committee.context).unwrap();
            let points = tecpan_wire::types::parse_points(&request.points).unwrap();
            let partials = wrong
                .partials(&points, &committee.ecdh_context(), &mut rng)
                .unwrap();
            let header = Header {
                scope: message.header.scope,
                epoch: 0,
                id: [7; 16],
                created_at: tecpan_keeper::Clock::now(&*clock),
            };
            let body = Message::EcdhResponse(EcdhResponse::new(message.header.id, 2, &partials));
            let frame =
                encode(&SignedMessage::sign(&keys.identity, header, body).unwrap()).unwrap();
            evil.send(target.clone(), frame).await.unwrap();
        }
    });
    let points = [point(3)];
    // With member 3 unreachable, only the impostor answers: the batch fails and the impostor is reported.
    let silent = v.keepers[2].keeper.identity();
    v.harness
        .net
        .set_filter(move |from, to, _| from != &silent && to != &silent);
    let result = v.keepers[0]
        .keeper
        .ecdh_batch(v.address, 0, EcdhPurpose::NoteDecryption, points.to_vec())
        .await;
    assert!(matches!(
        result,
        Err(Error::Unavailable {
            needed: 2,
            received: 1
        })
    ));
    let reported = eventually(async || match events.try_recv() {
        Ok(Event::InvalidPartials { member, .. }) => Some(member),
        _ => None,
    })
    .await;
    // With member 3 back, the batch routes around the impostor.
    v.harness.net.clear_filter();
    let gathered = v.keepers[0]
        .keeper
        .ecdh_batch(v.address, 0, EcdhPurpose::NoteDecryption, points.to_vec())
        .await
        .unwrap();
    assert_eq!(
        gathered.iter().map(|m| m.member).collect::<Vec<_>>(),
        vec![1, 3]
    );
    assert_eq!(reported, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn peers_enforce_the_ecdh_budget() {
    let v = vault(KeeperConfig {
        ecdh_points_per_minute: 4,
        ..config()
    })
    .await;
    let points: Vec<WirePoint> = (10..18).map(point).collect();
    let result = v.keepers[0]
        .keeper
        .ecdh_batch(v.address, 0, EcdhPurpose::Handshake, points)
        .await;
    assert!(matches!(
        result,
        Err(Error::Unavailable {
            needed: 2,
            received: 1
        })
    ));
    let audit = v.keepers[1].keeper.audit_log(v.address, 10).unwrap();
    assert!(
        audit
            .iter()
            .any(|e| e.outcome.contains("rate limit") && e.purpose == "handshake")
    );
    // Small batches still go through.
    ecdh(&v.keepers[0].keeper, v.address, 0, &[point(20)]).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn reshare_activates_on_chain_and_removes_the_departing_member() {
    let v = vault(config()).await;
    let points = [point(4)];
    let before = ecdh(&v.keepers[0].keeper, v.address, 0, &points).await;

    let newcomer = v.harness.keeper(4);
    let mut rng = ChaCha20Rng::seed_from_u64(9);
    let new_members: Vec<NewMember> = [&v.keepers[0], &v.keepers[1], &newcomer]
        .iter()
        .map(|k| NewMember {
            identity: k.keeper.identity(),
            transport: k.keeper.transport_key(),
            addresses: Vec::new(),
            signer_key: WirePoint::from(
                &tecpan_crypto::Point::generator_mul_secret(&random_scalar(&mut rng)).unwrap(),
            ),
        })
        .collect();
    let request = ReshareRequest {
        session: [2; 32],
        vault: v.address,
        current: None,
        dealers: vec![1, 2],
        new_threshold: 2,
        new_spending_threshold: 2,
        new_members,
    };
    for k in &v.keepers[..2] {
        k.keeper.start_reshare(request.clone()).await.unwrap();
    }
    newcomer
        .keeper
        .start_reshare(ReshareRequest {
            current: Some(v.manifest.clone()),
            ..request.clone()
        })
        .await
        .unwrap();
    for k in [&v.keepers[0], &v.keepers[1], &newcomer] {
        eventually(async || (status(&k.keeper, [2; 32]) == "completed").then_some(())).await;
    }
    let result: ReshareResult = serde_json::from_value(
        v.keepers[0]
            .keeper
            .ceremony([2; 32])
            .unwrap()
            .result
            .unwrap(),
    )
    .unwrap();
    let mut next = result.manifest.unwrap();
    next.epoch = 1;
    let commitment = Committee::new(next).unwrap().commitment().unwrap();

    // Until the chain names the new committee, the old one keeps serving.
    assert_eq!(
        ecdh(&v.keepers[2].keeper, v.address, 0, &points).await,
        before
    );

    for k in [&v.keepers[0], &v.keepers[1], &newcomer] {
        assert_eq!(
            k.keeper
                .observe_config(v.address, 1, commitment)
                .await
                .unwrap(),
            Observation::Activated
        );
    }
    assert_eq!(
        v.keepers[2]
            .keeper
            .observe_config(v.address, 1, commitment)
            .await
            .unwrap(),
        Observation::Removed
    );

    // Same address secret, new committee; the removed member can no longer take part.
    assert_eq!(ecdh(&newcomer.keeper, v.address, 1, &points).await, before);
    assert_eq!(
        ecdh(&v.keepers[0].keeper, v.address, 1, &points).await,
        before
    );
    let removed = v.keepers[2]
        .keeper
        .ecdh_batch(v.address, 1, EcdhPurpose::NoteDecryption, points.to_vec())
        .await;
    assert!(matches!(removed, Err(Error::Removed)));
    assert_eq!(
        v.keepers[2].keeper.vault(v.address).await.unwrap().status,
        "removed"
    );
    assert!(
        !v.keepers[2]
            .dir
            .path()
            .join("secrets")
            .read_dir()
            .unwrap()
            .any(|e| {
                e.unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("share-")
            })
    );

    // A request forged by the removed member is not served.
    let keys = v.keepers[2].keys();
    let (evil, mut inbox) = v
        .harness
        .net
        .endpoint(keys.identity.public(), keys.transport_public.0);
    let header = Header {
        scope: v.address,
        epoch: 1,
        id: [8; 16],
        created_at: tecpan_keeper::Clock::now(&*v.harness.clock),
    };
    let body = Message::EcdhRequest(EcdhRequest {
        purpose: EcdhPurpose::NoteDecryption,
        points: points.to_vec(),
    });
    let frame = encode(&SignedMessage::sign(&keys.identity, header, body).unwrap()).unwrap();
    evil.send(peer(&v.keepers[0]), frame).await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(300), inbox.recv())
            .await
            .is_err()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_silent_member_times_the_ceremony_out() {
    let harness = Harness::new(config());
    let keepers: Vec<TestKeeper> = (1..=3).map(|i| harness.keeper(i)).collect();
    let params = DkgParams {
        session: [3; 32],
        threshold: 2,
        members: keepers.iter().map(TestKeeper::participant).collect(),
    };
    for k in &keepers[..2] {
        k.keeper.start_dkg(params.clone()).await.unwrap();
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(status(&keepers[0].keeper, [3; 32]), "running");
    harness.clock.advance(31);
    let failure = eventually(async || {
        keepers[0]
            .keeper
            .ceremony([3; 32])
            .ok()
            .and_then(|c| c.failure)
    })
    .await;
    assert_eq!(
        failure.blamed,
        vec![hex::encode(keepers[2].keeper.identity().0)]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn keepers_resume_vaults_and_abort_interrupted_ceremonies_after_a_restart() {
    let v = vault(config()).await;
    let points = [point(5)];
    let before = ecdh(&v.keepers[1].keeper, v.address, 0, &points).await;

    // A ceremony in progress when keeper 1 stops.
    let params = DkgParams {
        session: [4; 32],
        threshold: 2,
        members: v.keepers.iter().map(TestKeeper::participant).collect(),
    };
    v.keepers[0].keeper.start_dkg(params).await.unwrap();

    let identity = v.keepers[0].keeper.identity();
    v.harness.net.leave(&identity);
    let restarted = v
        .harness
        .start(v.keepers[0].dir.path(), v.keepers[0].master);
    assert_eq!(restarted.identity(), identity);
    let interrupted = restarted.ceremony([4; 32]).unwrap();
    assert_eq!(interrupted.status, "aborted");
    assert!(interrupted.failure.unwrap().reason.contains("restarted"));
    let info = restarted.vault(v.address).await.unwrap();
    assert!(info.serving);
    assert_eq!(ecdh(&restarted, v.address, 0, &points).await, before);
}

#[tokio::test(flavor = "multi_thread")]
async fn proposals_and_verified_approvals_replicate() {
    let v = vault(config()).await;
    let proposal = Proposal {
        config_version: 0,
        outer_hash: [3; 32],
        expires_at: 1_800_000_000,
        tx_nonce: [4; 32],
        fee_method: 0,
        payload: vec![1, 2, 3],
        summary: "pay 10".into(),
        proposer: 1,
    };
    let body = encode_body(&Message::Proposal(proposal.clone())).unwrap();
    let id = v.keepers[0]
        .keeper
        .put_proposal(v.address, &body)
        .await
        .unwrap();
    for k in &v.keepers[1..] {
        eventually(async || {
            (k.keeper.proposals(v.address).unwrap() == vec![body.clone()]).then_some(())
        })
        .await;
    }

    let mut rng = ChaCha20Rng::seed_from_u64(11);
    let message = proposal
        .approval_message(parse_field(&v.address).unwrap())
        .unwrap();
    let signature = schnorr::sign(&v.signers[1], message, &mut rng).unwrap();
    let approval = encode_body(&Message::Approval(Approval::new(id, 1, &signature))).unwrap();
    v.keepers[1]
        .keeper
        .put_approval(v.address, &approval)
        .await
        .unwrap();
    eventually(async || {
        (v.keepers[2].keeper.approvals(id).unwrap() == vec![approval.clone()]).then_some(())
    })
    .await;

    // The same signature claimed for another slot is rejected.
    let forged = encode_body(&Message::Approval(Approval::new(id, 0, &signature))).unwrap();
    assert!(matches!(
        v.keepers[0].keeper.put_approval(v.address, &forged).await,
        Err(Error::Invalid(_))
    ));

    // Executions are tracked; anything else spending from the vault raises an alert.
    v.keepers[0]
        .keeper
        .put_receipt(v.address, id, [5; 32])
        .await
        .unwrap();
    eventually(async || {
        v.keepers[2]
            .keeper
            .observe_tx(v.address, [5; 32])
            .unwrap()
            .then_some(())
    })
    .await;
    let mut events = v.keepers[2].keeper.subscribe();
    assert!(!v.keepers[2].keeper.observe_tx(v.address, [6; 32]).unwrap());
    let alert = eventually(async || match events.try_recv() {
        Ok(Event::UnexpectedSpend { tx_hash, .. }) => Some(tx_hash),
        _ => None,
    })
    .await;
    assert_eq!(alert, hex::encode([6; 32]));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_committee_on_chain_raises_an_alert() {
    let v = vault(config()).await;
    let observation = v.keepers[0]
        .keeper
        .observe_config(v.address, 0, [9; 32])
        .await
        .unwrap();
    assert_eq!(observation, Observation::Unknown);
    let current = Committee::new(v.manifest.clone())
        .unwrap()
        .commitment()
        .unwrap();
    assert_eq!(
        v.keepers[0]
            .keeper
            .observe_config(v.address, 0, current)
            .await
            .unwrap(),
        Observation::Current
    );
}
