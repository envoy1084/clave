//! Pairwise mailboxes for store-and-forward through an untrusted relay.
//!
//! The mailbox where keeper `A` leaves messages for keeper `B` on day `d` has the id
//! `SHA-256("clave.mailbox.v0" ‖ X25519(A, B) ‖ A_pub ‖ B_pub ‖ d)`. Only `A` and `B` can compute it, so the relay
//! cannot link a mailbox to a keeper, a vault, the reverse direction or another day. Payloads are HPKE envelopes
//! (Auth mode) bound to the mailbox id and day, so the relay holds ciphertext only and the recipient learns which keeper
//! sealed each one.

use rand_core::CryptoRng;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::{
    Error,
    envelope::{self, Context, Envelope, KEY_BYTES, TransportPublic, TransportSecret},
};

/// Domain tag of mailbox ids.
pub const MAILBOX_DOMAIN: &[u8] = b"clave.mailbox.v0";
/// Envelope kind of mailbox payloads.
pub const KIND_MAILBOX: u16 = 3;

/// Id of the mailbox `sender` writes to for `recipient` on `day` (days since the unix epoch). `mine` is either party's
/// secret and `peer` the other party's public key.
///
/// # Errors
/// [`Error::EnvelopeKey`] if the key exchange is degenerate (a low-order peer key).
pub fn mailbox_id(
    mine: &TransportSecret,
    peer: &TransportPublic,
    sender: &TransportPublic,
    recipient: &TransportPublic,
    day: u32,
) -> Result<[u8; 32], Error> {
    let secret = StaticSecret::from(*mine.to_bytes());
    let shared = secret.diffie_hellman(&PublicKey::from(peer.0));
    if !shared.was_contributory() {
        return Err(Error::EnvelopeKey);
    }
    let mut hasher = Sha256::new();
    hasher.update(MAILBOX_DOMAIN);
    hasher.update(shared.as_bytes());
    hasher.update(sender.0);
    hasher.update(recipient.0);
    hasher.update(day.to_be_bytes());
    Ok(hasher.finalize().into())
}

fn context(mailbox: &[u8; 32], day: u32) -> Context {
    Context {
        session: *mailbox,
        epoch: day,
        kind: KIND_MAILBOX,
        sender: 0,
        recipient: 0,
    }
}

/// Seals `plaintext` for `recipient` into mailbox `mailbox` of `day`; returns `encapsulated key ‖ ciphertext`.
///
/// # Errors
/// Envelope errors.
pub fn seal(
    mailbox: &[u8; 32],
    day: u32,
    sender: &TransportSecret,
    recipient: &TransportPublic,
    plaintext: &[u8],
    rng: &mut impl CryptoRng,
) -> Result<Vec<u8>, Error> {
    let sealed = envelope::seal(&context(mailbox, day), sender, recipient, plaintext, rng)?;
    let mut out = Vec::with_capacity(KEY_BYTES + sealed.ciphertext.len());
    out.extend_from_slice(&sealed.encapsulated_key);
    out.extend_from_slice(&sealed.ciphertext);
    Ok(out)
}

/// Opens a payload from mailbox `mailbox` of `day`, sealed by `sender`.
///
/// # Errors
/// [`Error::InvalidLength`] for a truncated payload, [`Error::EnvelopeOpen`] if it was not sealed by `sender` for this
/// mailbox or was modified.
pub fn open(
    mailbox: &[u8; 32],
    day: u32,
    recipient: &TransportSecret,
    sender: &TransportPublic,
    sealed: &[u8],
) -> Result<Zeroizing<Vec<u8>>, Error> {
    if sealed.len() <= KEY_BYTES {
        return Err(Error::InvalidLength {
            expected: KEY_BYTES + 1,
            actual: sealed.len(),
        });
    }
    let (key, ciphertext) = sealed.split_at(KEY_BYTES);
    let mut encapsulated_key = [0u8; KEY_BYTES];
    encapsulated_key.copy_from_slice(key);
    envelope::open(
        &context(mailbox, day),
        recipient,
        sender,
        &Envelope {
            encapsulated_key,
            ciphertext: ciphertext.to_vec(),
        },
    )
}

#[cfg(test)]
mod tests {
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    use super::*;

    fn pair(seed: u64) -> (TransportSecret, TransportPublic) {
        TransportSecret::generate(&mut ChaCha20Rng::seed_from_u64(seed))
    }

    #[test]
    fn both_parties_derive_the_same_id_and_ids_separate_direction_and_day() {
        let (a, a_pub) = pair(1);
        let (b, b_pub) = pair(2);
        let from_a = mailbox_id(&a, &b_pub, &a_pub, &b_pub, 100).unwrap();
        let from_b = mailbox_id(&b, &a_pub, &a_pub, &b_pub, 100).unwrap();
        assert_eq!(from_a, from_b);
        assert_ne!(mailbox_id(&a, &b_pub, &b_pub, &a_pub, 100).unwrap(), from_a);
        assert_ne!(mailbox_id(&a, &b_pub, &a_pub, &b_pub, 101).unwrap(), from_a);
        let (_, c_pub) = pair(3);
        assert_ne!(mailbox_id(&a, &c_pub, &a_pub, &c_pub, 100).unwrap(), from_a);
    }

    #[test]
    fn payloads_open_only_for_their_mailbox_and_sender() {
        let mut rng = ChaCha20Rng::seed_from_u64(9);
        let (a, a_pub) = pair(1);
        let (b, b_pub) = pair(2);
        let (c, _) = pair(3);
        let id = mailbox_id(&a, &b_pub, &a_pub, &b_pub, 7).unwrap();
        let sealed = seal(&id, 7, &a, &b_pub, b"signed frame", &mut rng).unwrap();
        assert_eq!(
            open(&id, 7, &b, &a_pub, &sealed).unwrap().as_slice(),
            b"signed frame"
        );
        assert!(open(&id, 8, &b, &a_pub, &sealed).is_err());
        assert!(open(&[0; 32], 7, &b, &a_pub, &sealed).is_err());
        assert!(open(&id, 7, &c, &a_pub, &sealed).is_err());
        assert!(open(&id, 7, &b, &b_pub, &sealed).is_err());
        assert!(open(&id, 7, &b, &a_pub, &sealed[..10]).is_err());
    }

    #[test]
    fn low_order_peer_keys_are_refused() {
        let (a, a_pub) = pair(1);
        let zero = TransportPublic([0; 32]);
        assert!(mailbox_id(&a, &zero, &a_pub, &zero, 1).is_err());
    }
}
