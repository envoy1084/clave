//! Lobby invites (string form, canonical bytes, bounds) and lobby message bounds.
#![allow(clippy::unwrap_used)]

mod common;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use common::{header, identity, invite, lobby_member};
use tecpan_wire::{
    Error, Message, SignedMessage, decode, encode,
    lobby::{
        INVITE_PREFIX, Invite, LobbyPlan, LobbyState, MAX_INVITE_CHARS, MAX_SEALED_SECRETS_BYTES,
        SealedSecrets, scope,
    },
};

#[test]
fn invites_round_trip_through_their_string_form() {
    let text = invite().encode().unwrap();
    assert!(text.starts_with(INVITE_PREFIX));
    assert!(!text.contains('='));
    let decoded = Invite::decode(&format!("  {text}\n")).unwrap();
    assert_eq!(decoded, invite());
    assert_eq!(decoded.encode().unwrap(), text);
}

#[test]
fn invite_debug_output_hides_the_secret() {
    let printed = format!("{:?}", invite());
    assert!(!printed.contains("50, 50, 50"));
    assert!(printed.contains("Treasury"));
}

#[test]
fn malformed_invites_are_rejected() {
    let text = invite().encode().unwrap();
    let body = text.strip_prefix(INVITE_PREFIX).unwrap();
    let bytes = URL_SAFE_NO_PAD.decode(body).unwrap();

    assert!(Invite::decode(body).is_err(), "missing prefix");
    assert!(Invite::decode(&format!("tecpan-invite-v1:{body}")).is_err());
    assert!(Invite::decode(&format!("{text}=")).is_err(), "padding");
    assert!(Invite::decode(&format!("{text}!")).is_err(), "alphabet");
    for len in 0..bytes.len() {
        let truncated = format!("{INVITE_PREFIX}{}", URL_SAFE_NO_PAD.encode(&bytes[..len]));
        assert!(Invite::decode(&truncated).is_err(), "prefix {len}");
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(matches!(
        Invite::from_bytes(&trailing),
        Err(Error::Malformed)
    ));
    // The network's length varint re-encoded as an overlong varint.
    let encoded = invite().to_bytes().unwrap();
    let position = encoded.len() - "aztec-testnet".len() - 1;
    assert_eq!(usize::from(encoded[position]), "aztec-testnet".len());
    let mut overlong = encoded[..position].to_vec();
    overlong.extend_from_slice(&[encoded[position] | 0x80, 0x00]);
    overlong.extend_from_slice(&encoded[position + 1..]);
    assert!(matches!(
        Invite::from_bytes(&overlong),
        Err(Error::Malformed)
    ));
    // Non-zero trailing bits in the last base64 character (a second string form of the same bytes).
    if !bytes.len().is_multiple_of(3) {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut chars = text.clone().into_bytes();
        let last = chars.len() - 1;
        let value = ALPHABET.iter().position(|c| *c == chars[last]).unwrap();
        chars[last] = ALPHABET[value | 1];
        assert!(Invite::decode(&String::from_utf8(chars).unwrap()).is_err());
    }
    assert!(matches!(
        Invite::decode(&"a".repeat(MAX_INVITE_CHARS + 1)),
        Err(Error::TooLarge(_))
    ));
}

#[test]
fn invite_bounds_are_enforced() {
    let cases: Vec<(Invite, &str)> = vec![
        (
            Invite {
                name: "n".repeat(65),
                ..invite()
            },
            "name",
        ),
        (
            Invite {
                host_name: "n".repeat(65),
                ..invite()
            },
            "host_name",
        ),
        (
            Invite {
                network: "n".repeat(65),
                ..invite()
            },
            "network",
        ),
        (
            Invite {
                threshold: 0,
                ..invite()
            },
            "threshold",
        ),
        (
            Invite {
                threshold: 9,
                ..invite()
            },
            "threshold",
        ),
        (
            Invite {
                viewing_threshold: 1,
                ..invite()
            },
            "viewing_threshold",
        ),
    ];
    for (bad, field) in cases {
        assert_eq!(bad.encode(), Err(Error::OutOfBounds { field }), "{field}");
    }
    let mut host = invite().host.clone();
    host.addresses = vec!["a".repeat(257)];
    assert!(Invite { host, ..invite() }.encode().is_err());
}

#[test]
fn lobby_scope_pads_the_id() {
    let padded = scope(&[7; 16]);
    assert_eq!(&padded[..16], &[7; 16]);
    assert_eq!(&padded[16..], &[0; 16]);
}

fn sign(body: Message) -> Result<SignedMessage, Error> {
    SignedMessage::sign(&identity(1), header(1), body)
}

#[test]
fn lobby_message_bounds_are_enforced() {
    let state = LobbyState {
        lobby: [1; 16],
        revision: 1,
        name: "v".into(),
        threshold: 2,
        viewing_threshold: 2,
        members: vec![lobby_member(1)],
        started: false,
    };
    assert!(sign(Message::LobbyState(state.clone())).is_ok());
    let too_many = LobbyState {
        members: (1..=9).map(lobby_member).collect(),
        ..state.clone()
    };
    assert_eq!(
        sign(Message::LobbyState(too_many)).err(),
        Some(Error::OutOfBounds { field: "members" })
    );
    let empty = LobbyState {
        members: Vec::new(),
        ..state.clone()
    };
    assert!(sign(Message::LobbyState(empty)).is_err());
    let mut long_name = lobby_member(2);
    long_name.name = "x".repeat(65);
    assert!(
        sign(Message::LobbyState(LobbyState {
            members: vec![long_name],
            ..state
        }))
        .is_err()
    );

    let plan = LobbyPlan {
        lobby: [1; 16],
        session: [2; 32],
        threshold: 2,
        viewing_threshold: 2,
        salt: [0; 32],
        members: vec![lobby_member(1), lobby_member(2)],
        secrets: vec![SealedSecrets {
            index: 2,
            enc: [3; 32],
            ciphertext: vec![4; MAX_SEALED_SECRETS_BYTES],
        }],
    };
    assert!(sign(Message::LobbyPlan(plan.clone())).is_ok());
    let mut oversized = plan.clone();
    oversized.secrets[0].ciphertext.push(0);
    assert!(sign(Message::LobbyPlan(oversized.clone())).is_err());
    let mut message = sign(Message::LobbyPlan(plan.clone())).unwrap();
    message.body = Message::LobbyPlan(oversized);
    assert!(matches!(
        decode(&encode(&message).unwrap()),
        Err(Error::OutOfBounds {
            field: "secrets.ciphertext"
        })
    ));
    assert_eq!(plan.sealed_for(2).map(|s| s.enc), Some([3; 32]));
    assert!(plan.sealed_for(1).is_none());
}
