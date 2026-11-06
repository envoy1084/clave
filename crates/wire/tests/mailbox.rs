//! Mailbox protocol encoding.
#![allow(clippy::unwrap_used)]

use tecpan_wire::{
    Error,
    mailbox::{
        MAX_ENVELOPE_BYTES, MAX_FETCH_MAILBOXES, MailboxRequest, MailboxResponse, Rejection,
        StoredEnvelope, decode_request, decode_response, encode, envelope_id,
    },
    messages::RelayEnvelope,
};

fn envelope(byte: u8) -> RelayEnvelope {
    RelayEnvelope {
        recipient: [byte; 32],
        expires_at: 1_800_000_000,
        sealed: vec![byte; 80],
    }
}

#[test]
fn requests_and_responses_round_trip() {
    for request in [
        MailboxRequest::Deposit(envelope(1)),
        MailboxRequest::Fetch {
            mailboxes: vec![[2; 32], [3; 32]],
        },
        MailboxRequest::Delete { ids: vec![[4; 32]] },
    ] {
        assert_eq!(decode_request(&encode(&request).unwrap()).unwrap(), request);
    }
    let id = envelope_id(&envelope(5)).unwrap();
    for response in [
        MailboxResponse::Stored { id },
        MailboxResponse::Rejected(Rejection::QuotaExceeded),
        MailboxResponse::Envelopes(vec![StoredEnvelope {
            id,
            envelope: envelope(5),
        }]),
        MailboxResponse::Deleted(3),
    ] {
        assert_eq!(
            decode_response(&encode(&response).unwrap()).unwrap(),
            response
        );
    }
}

#[test]
fn envelope_ids_bind_every_field() {
    let base = envelope_id(&envelope(1)).unwrap();
    let mut other = envelope(1);
    other.expires_at += 1;
    assert_ne!(envelope_id(&other).unwrap(), base);
    other = envelope(1);
    other.sealed.push(0);
    assert_ne!(envelope_id(&other).unwrap(), base);
}

#[test]
fn bounds_and_canonical_encoding_are_enforced() {
    let mut big = envelope(1);
    big.sealed = vec![0; MAX_ENVELOPE_BYTES + 1];
    let bytes = encode(&MailboxRequest::Deposit(big)).unwrap();
    assert!(matches!(
        decode_request(&bytes),
        Err(Error::OutOfBounds { field: "sealed" })
    ));

    let fetch = MailboxRequest::Fetch {
        mailboxes: vec![[0; 32]; MAX_FETCH_MAILBOXES + 1],
    };
    assert!(matches!(
        decode_request(&encode(&fetch).unwrap()),
        Err(Error::OutOfBounds { .. })
    ));

    let mut bytes = encode(&MailboxRequest::Delete { ids: vec![] }).unwrap();
    bytes.push(0);
    assert!(matches!(decode_request(&bytes), Err(Error::Malformed)));
}
