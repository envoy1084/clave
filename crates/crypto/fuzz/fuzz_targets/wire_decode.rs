//! Decoding untrusted wire bytes (signed messages and bare bodies) never panics, accepted values re-encode to the
//! same bytes, and verification and crypto conversions of decoded bodies never panic.
#![no_main]

use libfuzzer_sys::fuzz_target;
use tecpan_wire::{Message, decode, decode_body, encode, encode_body, mailbox};

fuzz_target!(|data: &[u8]| {
    if let Ok(request) = mailbox::decode_request(data) {
        assert_eq!(mailbox::encode(&request).ok().as_deref(), Some(data));
    }
    if let Ok(response) = mailbox::decode_response(data) {
        assert_eq!(mailbox::encode(&response).ok().as_deref(), Some(data));
    }
    if let Ok(body) = decode_body(data) {
        assert_eq!(encode_body(&body).ok().as_deref(), Some(data));
        exercise(&body);
    }
    let Ok(message) = decode(data) else { return };
    assert_eq!(encode(&message).ok().as_deref(), Some(data));
    let _ = message.verify();
    exercise(&message.body);
});

fn exercise(body: &Message) {
    match body {
        Message::Manifest(manifest) => {
            let _ = manifest.signer_set_commitment();
            let _ = manifest.committee_commitment();
            let _ = manifest.address_context();
        }
        Message::DkgRound1(round1) => {
            let _ = tecpan_crypto::dkg::Round1Package::try_from(round1);
        }
        Message::ReshareDeal(deal) => {
            let _ = tecpan_crypto::reshare::DealerPackage::try_from(deal);
        }
        Message::EcdhResponse(response) => {
            let _ = response.partials();
        }
        Message::Proposal(proposal) => {
            let _ = proposal.id();
        }
        _ => {}
    }
}
