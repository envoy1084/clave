//! Decoding untrusted wire bytes never panics, accepted messages re-encode to the same bytes, and verification and
//! crypto conversions of decoded bodies never panic.
#![no_main]

use libfuzzer_sys::fuzz_target;
use tecpan_wire::{Message, decode, encode};

fuzz_target!(|data: &[u8]| {
    let Ok(message) = decode(data) else { return };
    assert_eq!(encode(&message).ok().as_deref(), Some(data));
    let _ = message.verify();
    match &message.body {
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
});
