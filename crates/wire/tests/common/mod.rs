//! Deterministic sample messages shared by the wire tests.
#![allow(
    dead_code,
    unreachable_pub,
    clippy::unwrap_used,
    clippy::missing_panics_doc,
    clippy::too_many_lines
)]

use tecpan_crypto::{Point, Scalar, SecretScalar};
use tecpan_wire::{
    Header, KeeperIdentity, Message, SignedMessage,
    messages::{
        Abort, AbortReason, Approval, Ceremony, Complaint, Confirm, DkgRound1, EcdhPurpose,
        EcdhRequest, EcdhResponse, ExecutionReceipt, Justification, Manifest, Member, Proposal,
        RelayEnvelope, ReshareDeal, SealedShare, WirePartial,
    },
    types::{Bytes64, WirePoint},
};

pub const NOW: u64 = 1_700_000_000;

pub fn point(n: u64) -> Point {
    Point::generator_mul_secret(&SecretScalar::new(Scalar::from(n))).unwrap()
}

pub fn wire_point(n: u64) -> WirePoint {
    WirePoint::from(&point(n))
}

pub fn identity(n: u8) -> KeeperIdentity {
    KeeperIdentity::from_seed(&[n; 32])
}

pub fn header(id: u8) -> Header {
    Header {
        scope: [0x11; 32],
        epoch: 3,
        id: [id; 16],
        created_at: NOW,
    }
}

pub fn proposal() -> Proposal {
    Proposal {
        config_version: 3,
        outer_hash: [0x22; 32],
        expires_at: NOW + 3600,
        tx_nonce: [0x01; 32],
        fee_method: 1,
        payload: vec![1, 2, 3, 4],
        summary: "transfer 10 to 0x05".to_owned(),
        proposer: 1,
    }
}

/// One sample of every message kind, in [`Message`] variant order, named for the fixture file.
pub fn samples() -> Vec<(&'static str, Message)> {
    let manifest = Manifest {
        vault: [0x11; 32],
        epoch: 3,
        threshold: 2,
        viewing_threshold: 2,
        preaddress: [0x03; 32],
        group_key: wire_point(1000),
        members: (1..=3)
            .map(|i| Member {
                index: i,
                signer_key: wire_point(u64::from(i)),
                verification_share: wire_point(1000 + u64::from(i)),
                identity: identity(u8::try_from(i).unwrap()).public(),
                transport: [u8::try_from(i).unwrap(); 32],
                addresses: vec![format!("/ip4/127.0.0.1/udp/400{i}/quic-v1")],
            })
            .collect(),
    };
    let sealed = SealedShare {
        dealer: 1,
        recipient: 2,
        encapsulated_key: [0x44; 32],
        ciphertext: vec![0x55; 48],
    };
    let confirm = Confirm {
        transcript: Bytes64([0x66; 32], [0x77; 32]),
    };
    vec![
        ("manifest", Message::Manifest(manifest)),
        (
            "dkg_round1",
            Message::DkgRound1(DkgRound1 {
                dealer: 1,
                commitments: vec![wire_point(5), wire_point(6)],
                proof_commitment: wire_point(7),
                proof_response: [0x08; 32],
            }),
        ),
        ("dkg_round2", Message::DkgRound2(sealed.clone())),
        ("dkg_confirm", Message::DkgConfirm(confirm)),
        (
            "reshare_deal",
            Message::ReshareDeal(ReshareDeal {
                dealer: 2,
                commitments: vec![wire_point(9)],
            }),
        ),
        ("reshare_sub_share", Message::ReshareSubShare(sealed)),
        ("reshare_confirm", Message::ReshareConfirm(confirm)),
        (
            "complaint",
            Message::Complaint(Complaint {
                ceremony: Ceremony::Reshare,
                accused: 2,
            }),
        ),
        (
            "justification",
            Message::Justification(Justification {
                ceremony: Ceremony::Dkg,
                complainant: 3,
                share: [0x09; 32],
            }),
        ),
        (
            "abort",
            Message::Abort(Abort {
                ceremony: Ceremony::Dkg,
                reason: AbortReason::Timeout,
            }),
        ),
        (
            "ecdh_request",
            Message::EcdhRequest(EcdhRequest {
                purpose: EcdhPurpose::NoteDecryption,
                points: vec![wire_point(10), wire_point(11)],
            }),
        ),
        (
            "ecdh_response",
            Message::EcdhResponse(EcdhResponse {
                request: [0x0a; 16],
                member: 2,
                partials: vec![WirePartial {
                    point: wire_point(12),
                    challenge: [0x0b; 32],
                    response: [0x0c; 32],
                }],
            }),
        ),
        ("proposal", Message::Proposal(proposal())),
        (
            "approval",
            Message::Approval(Approval {
                proposal: [0x0d; 32],
                slot: 0,
                signature: [[0x0e; 32]; 4],
            }),
        ),
        (
            "execution_receipt",
            Message::ExecutionReceipt(ExecutionReceipt {
                proposal: [0x0d; 32],
                tx_hash: [0x0f; 32],
            }),
        ),
        (
            "relay_envelope",
            Message::RelayEnvelope(RelayEnvelope {
                recipient: [0x10; 32],
                expires_at: NOW + 86_400,
                sealed: vec![1; 64],
            }),
        ),
    ]
}

pub fn signed(body: Message) -> SignedMessage {
    SignedMessage::sign(&identity(1), header(1), body).unwrap()
}
