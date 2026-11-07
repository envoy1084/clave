//! Echo broadcast, sync and relay mailboxes on the in-memory network.
#![allow(
    clippy::unwrap_used,
    clippy::missing_panics_doc,
    clippy::too_many_lines
)]

mod common;

use std::time::Duration;

use clave_crypto::{SecretScalar, schnorr, vault};
use clave_keeper::{
    KeeperConfig,
    ceremony::dkg::{Dkg, DkgParams},
    transport::{Peer, Transport},
};
use clave_wire::{
    Header, Message, SignedMessage, encode, encode_body,
    messages::{Approval, Proposal, Rejection},
    types::{field, parse_field},
};
use common::{Harness, TestKeeper, eventually, manifest, run_dkg};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

fn config() -> KeeperConfig {
    KeeperConfig {
        round_timeout_secs: 30,
        ecdh_timeout: Duration::from_millis(500),
        ..KeeperConfig::default()
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
async fn an_equivocating_dealer_is_caught_by_the_echo() {
    let harness = Harness::new(config());
    let keepers: Vec<TestKeeper> = (1..=3).map(|i| harness.keeper(i)).collect();
    let params = DkgParams {
        session: [5; 32],
        threshold: 2,
        members: keepers.iter().map(TestKeeper::participant).collect(),
    };
    for k in &keepers[..2] {
        k.keeper.start_dkg(params.clone()).await.unwrap();
    }
    // Member 3 is replaced by a dealer that shows member 1 and member 2 different round-1 packages.
    let keys = keepers[2].keys();
    let (evil, _inbox) = harness
        .net
        .endpoint(keys.identity.public(), keys.transport_public.0);
    let mut rng = ChaCha20Rng::seed_from_u64(1);
    for (n, target) in keepers[..2].iter().enumerate() {
        let (_, outbox) =
            Dkg::start(params.clone(), &keys.identity.public(), 0, 30, &mut rng).unwrap();
        let header = Header {
            scope: [5; 32],
            epoch: 0,
            id: [u8::try_from(n).unwrap() + 1; 16],
            created_at: clave_keeper::Clock::now(&*harness.clock),
        };
        let frame =
            encode(&SignedMessage::sign(&keys.identity, header, outbox.0[0].1.clone()).unwrap())
                .unwrap();
        evil.send(peer(target), frame).await.unwrap();
    }
    // Every honest member aborts. Attribution is local: the member that compares the two signed packages blames the
    // dealer; a member that processes that member's abort before the echoed evidence aborts without blame (sends to a
    // peer are not ordered). At least one member always holds the proof.
    let blamed = hex::encode(keys.identity.public().0);
    let mut attributed = 0;
    for k in &keepers[..2] {
        let failure =
            eventually(async || k.keeper.ceremony([5; 32]).ok().and_then(|c| c.failure)).await;
        if failure.blamed == vec![blamed.clone()] {
            assert!(
                failure.reason.contains("two different round 1"),
                "{}",
                failure.reason
            );
            attributed += 1;
        } else {
            assert!(failure.blamed.is_empty(), "{failure:?}");
            assert!(
                failure.reason.contains("aborted by member"),
                "{}",
                failure.reason
            );
        }
    }
    assert!(attributed >= 1, "no member attributed the equivocation");
}

struct Records {
    proposal: Vec<u8>,
    approval: Vec<u8>,
    rejection: Vec<u8>,
}

/// A vault of three keepers; member 3 is cut off while member 1 proposes, member 2 approves and member 1 rejects.
async fn records_while_member_3_is_away(harness: &Harness) -> (Vec<TestKeeper>, [u8; 32], Records) {
    let keepers: Vec<TestKeeper> = (1..=3).map(|i| harness.keeper(i)).collect();
    let refs: Vec<&TestKeeper> = keepers.iter().collect();
    let result = run_dkg(&refs, 2, [1; 32]).await;
    let (manifest, signers) = manifest(&refs, &result, 7);
    for k in &keepers {
        k.keeper
            .register_vault([1; 32], manifest.clone())
            .await
            .unwrap();
    }
    let vault = manifest.vault;
    let away = keepers[2].keeper.identity();
    harness
        .net
        .set_filter(move |from, to, _| from != &away && to != &away);

    let proposal = Proposal {
        config_version: 0,
        outer_hash: [3; 32],
        expires_at: 1_800_000_000,
        tx_nonce: [4; 32],
        fee_method: 0,
        payload: b"transfer 25 to the payroll account".to_vec(),
        summary: "payroll".into(),
        proposer: 1,
    };
    let proposal_body = encode_body(&Message::Proposal(proposal.clone())).unwrap();
    let id = keepers[0]
        .keeper
        .put_proposal(vault, &proposal_body)
        .await
        .unwrap();
    eventually(async || (!keepers[1].keeper.proposals(vault).unwrap().is_empty()).then_some(()))
        .await;
    let message = proposal
        .approval_message(parse_field(&vault).unwrap())
        .unwrap();
    let signature =
        schnorr::sign(&signers[1], message, &mut ChaCha20Rng::seed_from_u64(2)).unwrap();
    let approval = encode_body(&Message::Approval(Approval::new(id, 1, &signature))).unwrap();
    keepers[1]
        .keeper
        .put_approval(vault, &approval)
        .await
        .unwrap();
    let rejection = rejection_body(vault, id, 0, &signers[0]);
    // Signed by member 1 but claiming slot 2: refused.
    assert!(
        keepers[0]
            .keeper
            .put_rejection(vault, &rejection_body(vault, id, 2, &signers[0]))
            .await
            .is_err()
    );
    keepers[0]
        .keeper
        .put_rejection(vault, &rejection)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(keepers[2].keeper.proposals(vault).unwrap().is_empty());
    (
        keepers,
        vault,
        Records {
            proposal: proposal_body,
            approval,
            rejection,
        },
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn an_offline_member_catches_up_through_sync() {
    let harness = Harness::new(config());
    let (keepers, vault, records) = records_while_member_3_is_away(&harness).await;
    let id = Proposal::id(&match clave_wire::decode_body(&records.proposal).unwrap() {
        Message::Proposal(p) => p,
        _ => unreachable!(),
    })
    .unwrap();
    harness.net.clear_filter();
    keepers[2].keeper.sync(vault).await.unwrap();
    eventually(async || {
        (keepers[2].keeper.proposals(vault).unwrap() == vec![records.proposal.clone()])
            .then_some(())
    })
    .await;
    eventually(async || {
        (keepers[2].keeper.approvals(id).unwrap() == vec![records.approval.clone()]).then_some(())
    })
    .await;
    eventually(async || {
        (keepers[2].keeper.rejections(id).unwrap() == vec![records.rejection.clone()]).then_some(())
    })
    .await;
}

/// A signed rejection body for `slot`.
fn rejection_body(vault: [u8; 32], id: [u8; 32], slot: u32, key: &SecretScalar) -> Vec<u8> {
    let message = vault::rejection_message(parse_field(&vault).unwrap(), &id);
    let signature = schnorr::sign(key, message, &mut ChaCha20Rng::seed_from_u64(9)).unwrap();
    let limbs = signature.to_limbs();
    encode_body(&Message::Rejection(Rejection {
        proposal: id,
        slot,
        reason: "wrong recipient".into(),
        signature: [
            field(&limbs[0]),
            field(&limbs[1]),
            field(&limbs[2]),
            field(&limbs[3]),
        ],
    }))
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn an_offline_member_catches_up_from_its_mailbox_hours_later() {
    let harness = Harness::with_mailbox(config());
    let (keepers, vault, records) = records_while_member_3_is_away(&harness).await;
    let mailbox = harness.mailbox.clone().unwrap();
    eventually(async || (mailbox.len() >= 2).then_some(())).await;

    // The relay holds ciphertext under opaque ids only.
    for stored in mailbox.snapshot() {
        let sealed = &stored.envelope.sealed;
        assert!(!sealed.windows(7).any(|w| w == b"payroll"));
        for k in &keepers {
            assert_ne!(stored.envelope.recipient, k.keeper.identity().0);
            assert_ne!(stored.envelope.recipient, k.keeper.transport_key());
        }
        assert_ne!(stored.envelope.recipient, vault);
    }

    // Members 1 and 2 go offline; member 3 comes back hours later (beyond the freshness window).
    harness.net.leave(&keepers[0].keeper.identity());
    harness.net.leave(&keepers[1].keeper.identity());
    harness.net.clear_filter();
    harness.clock.advance(6 * 3600);
    let delivered = keepers[2].keeper.poll_mailbox().await.unwrap();
    assert!(delivered >= 2);
    assert!(mailbox.is_empty());
    assert_eq!(
        keepers[2].keeper.proposals(vault).unwrap(),
        vec![records.proposal]
    );
    let id = match clave_wire::decode_body(&keepers[2].keeper.proposals(vault).unwrap()[0]).unwrap()
    {
        Message::Proposal(p) => p.id().unwrap(),
        _ => unreachable!(),
    };
    eventually(async || {
        (keepers[2].keeper.approvals(id).unwrap() == vec![records.approval.clone()]).then_some(())
    })
    .await;
}
