//! Signatures, signer binding and replay protection.
#![allow(clippy::unwrap_used)]

mod common;

use clave_wire::{Error, KeeperIdentity, ReplayGuard, decode, encode};
use common::{NOW, header, identity, samples, signed};
use rand_core::SeedableRng;

#[test]
fn signatures_verify_and_bind_the_signer() {
    for (name, body) in samples() {
        let message = signed(body);
        message.verify().unwrap();
        message.verify_from(&identity(1).public()).unwrap();
        assert!(
            matches!(
                message.verify_from(&identity(2).public()),
                Err(Error::UnexpectedSigner)
            ),
            "{name}"
        );
    }
}

#[test]
fn any_flipped_bit_breaks_the_message() {
    let bytes = encode(&signed(samples().remove(12).1)).unwrap();
    for position in 0..bytes.len() {
        let mut tampered = bytes.clone();
        tampered[position] ^= 0x01;
        if let Ok(message) = decode(&tampered) {
            assert!(
                message.verify().is_err(),
                "byte {position} flipped and still verified"
            );
        }
    }
}

#[test]
fn swapping_the_signer_fails() {
    let mut message = signed(samples().remove(0).1);
    message.signer = identity(2).public();
    assert!(matches!(message.verify(), Err(Error::InvalidSignature)));
}

#[test]
fn header_is_signed() {
    let mut message = signed(samples().remove(0).1);
    message.header.epoch += 1;
    assert!(matches!(message.verify(), Err(Error::InvalidSignature)));
}

#[test]
fn identities_are_stable_from_seed_and_random_otherwise() {
    assert_eq!(
        identity(9).public(),
        KeeperIdentity::from_seed(&identity(9).seed()).public()
    );
    let mut rng = rand_chacha::ChaCha20Rng::seed_from_u64(1);
    assert_ne!(
        KeeperIdentity::generate(&mut rng).public(),
        KeeperIdentity::generate(&mut rng).public()
    );
    assert!(!format!("{:?}", identity(9)).contains(&hex::encode(identity(9).seed().as_slice())));
}

#[test]
fn replay_guard_rejects_duplicates_epochs_and_stale_messages() {
    let mut guard = ReplayGuard::new(300, 2);
    let alice = identity(1).public();
    let bob = identity(2).public();
    guard.check(&alice, &header(1), 3, NOW).unwrap();
    assert!(matches!(
        guard.check(&alice, &header(1), 3, NOW),
        Err(Error::Replayed)
    ));
    // Same id from another signer is independent.
    guard.check(&bob, &header(1), 3, NOW).unwrap();
    assert!(matches!(
        guard.check(&alice, &header(2), 4, NOW),
        Err(Error::WrongEpoch {
            expected: 4,
            actual: 3
        })
    ));
    assert!(matches!(
        guard.check(&alice, &header(3), 3, NOW + 301),
        Err(Error::Stale)
    ));
    assert!(matches!(
        guard.check(&alice, &header(3), 3, NOW - 301),
        Err(Error::Stale)
    ));
    guard.check(&alice, &header(3), 3, NOW + 300).unwrap();
}

#[test]
fn replay_guard_evicts_oldest_beyond_capacity() {
    let mut guard = ReplayGuard::new(300, 2);
    let alice = identity(1).public();
    for id in 1..=3 {
        guard.check(&alice, &header(id), 3, NOW).unwrap();
    }
    // id 1 was evicted (capacity 2); ids 2 and 3 are still remembered.
    guard.check(&alice, &header(1), 3, NOW).unwrap();
    assert!(matches!(
        guard.check(&alice, &header(3), 3, NOW),
        Err(Error::Replayed)
    ));
}
