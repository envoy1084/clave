//! Encoding round trips, golden bytes and decode rejection.
#![allow(clippy::unwrap_used)]

mod common;

use std::collections::BTreeMap;

use common::{header, identity, samples, signed};
use tecpan_wire::{
    Error, MAX_MESSAGE_BYTES, Message, SignedMessage, decode, encode,
    messages::{EcdhPurpose, EcdhRequest, MAX_ECDH_POINTS, RelayEnvelope},
};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/wire.json");

#[test]
fn every_message_round_trips() {
    for (name, body) in samples() {
        let message = signed(body);
        let bytes = encode(&message).unwrap();
        assert_eq!(decode(&bytes).unwrap(), message, "{name}");
    }
}

/// Byte-exact encodings of every message kind. Any change here is a wire-format change: bump `WIRE_VERSION` or
/// only append. Regenerate with `UPDATE_WIRE_FIXTURES=1 cargo test -p tecpan-wire --test codec`.
#[test]
fn encodings_match_golden_fixtures() {
    let actual: BTreeMap<String, String> = samples()
        .into_iter()
        .map(|(name, body)| (name.to_owned(), hex::encode(encode(&signed(body)).unwrap())))
        .collect();
    if std::env::var_os("UPDATE_WIRE_FIXTURES").is_some() {
        std::fs::write(
            FIXTURES,
            serde_json::to_string_pretty(&actual).unwrap() + "\n",
        )
        .unwrap();
    }
    let expected: BTreeMap<String, String> =
        serde_json::from_str(&std::fs::read_to_string(FIXTURES).unwrap()).unwrap();
    assert_eq!(actual, expected);
}

/// Offset of the body's variant tag for the sample header: version (1) + scope (32) + epoch varint (1) + id (16) +
/// timestamp varint (5).
const BODY_TAG: usize = 1 + 32 + 1 + 16 + 5;

#[test]
fn variant_tags_follow_declaration_order() {
    for (tag, (name, body)) in samples().into_iter().enumerate() {
        let bytes = encode(&signed(body)).unwrap();
        assert_eq!(usize::from(bytes[BODY_TAG]), tag, "{name}");
    }
}

#[test]
fn rejects_trailing_bytes() {
    let mut bytes = encode(&signed(samples().remove(0).1)).unwrap();
    bytes.push(0);
    assert!(matches!(decode(&bytes), Err(Error::Malformed)));
}

#[test]
fn rejects_truncation() {
    let bytes = encode(&signed(samples().remove(0).1)).unwrap();
    for len in 0..bytes.len() {
        assert!(decode(&bytes[..len]).is_err(), "prefix {len}");
    }
}

#[test]
fn rejects_unknown_variant() {
    let mut bytes = encode(&signed(samples().remove(3).1)).unwrap();
    bytes[BODY_TAG] = 0x7f;
    assert!(matches!(decode(&bytes), Err(Error::Malformed)));
}

#[test]
fn rejects_other_versions() {
    let mut message = signed(samples().remove(3).1);
    message.version = 1;
    assert!(matches!(
        decode(&encode(&message).unwrap()),
        Err(Error::UnsupportedVersion(1))
    ));
}

#[test]
fn rejects_oversized_input() {
    assert!(matches!(
        decode(&vec![0; MAX_MESSAGE_BYTES + 1]),
        Err(Error::TooLarge(_))
    ));
    let relay = Message::RelayEnvelope(RelayEnvelope {
        recipient: [0; 32],
        expires_at: 0,
        sealed: vec![0; MAX_MESSAGE_BYTES],
    });
    assert!(matches!(
        SignedMessage::sign(&identity(1), header(1), relay),
        Err(Error::OutOfBounds { field: "sealed" })
    ));
}

#[test]
fn bounds_are_enforced_on_decode() {
    // Build an over-limit request by hand (sign refuses it) and check decode rejects it too.
    let points = vec![common::wire_point(2); MAX_ECDH_POINTS + 1];
    let body = Message::EcdhRequest(EcdhRequest {
        purpose: EcdhPurpose::Handshake,
        points,
    });
    assert!(SignedMessage::sign(&identity(1), header(1), body.clone()).is_err());
    let mut message = signed(Message::EcdhRequest(EcdhRequest {
        purpose: EcdhPurpose::Handshake,
        points: vec![common::wire_point(2)],
    }));
    message.body = body;
    assert!(matches!(
        decode(&encode(&message).unwrap()),
        Err(Error::OutOfBounds { field: "points" })
    ));
}
