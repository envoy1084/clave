//! Vault-founding lobbies: the join proof a co-founder presents with an invite, and the envelope that carries the
//! engine's protocol secrets from the founder (host) to each member.
//!
//! - `proof = HMAC-SHA256(invite secret, "tecpan.lobby.join.v0" ‖ lobby ‖ joiner identity ‖ joiner transport)`,
//!   checked in constant time.
//! - Secrets are sealed with the HPKE Auth envelope ([`crate::envelope`]), kind [`KIND_LOBBY_SECRETS`], bound to the
//!   plan's session, epoch 0, sender 1 (the host) and the recipient's 1-based roster index.

use hmac::{Hmac, KeyInit, Mac};
use rand_core::CryptoRng;
use sha2::Sha256;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

use crate::{
    Error,
    envelope::{self, Context, Envelope, TransportPublic, TransportSecret},
    shamir::MemberIndex,
};

/// Domain tag of join proofs.
pub const JOIN_DOMAIN: &[u8] = b"tecpan.lobby.join.v0";
/// Envelope kind of lobby protocol secrets (after DKG share 1, reshare sub-share 2 and mailbox 3).
pub const KIND_LOBBY_SECRETS: u16 = 4;
/// The host's member index in every lobby plan.
pub const HOST_INDEX: MemberIndex = 1;

/// The proof that a joiner holds the invite secret, bound to the lobby and the joiner's keys.
#[must_use]
pub fn join_proof(
    secret: &[u8; 32],
    lobby: &[u8; 16],
    identity: &[u8; 32],
    transport: &[u8; 32],
) -> [u8; 32] {
    // HMAC accepts keys of any length; a 32-byte key cannot fail.
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(secret)
        .unwrap_or_else(|_| unreachable!("HMAC takes any key length"));
    mac.update(JOIN_DOMAIN);
    mac.update(lobby);
    mac.update(identity);
    mac.update(transport);
    mac.finalize().into_bytes().into()
}

/// Whether `proof` is the join proof for these values (constant time in the proof).
#[must_use]
pub fn verify_join_proof(
    secret: &[u8; 32],
    lobby: &[u8; 16],
    identity: &[u8; 32],
    transport: &[u8; 32],
    proof: &[u8; 32],
) -> bool {
    join_proof(secret, lobby, identity, transport)
        .ct_eq(proof)
        .into()
}

fn context(session: &[u8; 32], recipient: MemberIndex) -> Context {
    Context {
        session: *session,
        epoch: 0,
        kind: KIND_LOBBY_SECRETS,
        sender: HOST_INDEX,
        recipient,
    }
}

/// Seals the protocol `secrets` from the host to member `recipient` (1-based) of the plan `session`.
///
/// # Errors
/// Envelope errors.
pub fn seal_secrets(
    session: &[u8; 32],
    recipient: MemberIndex,
    host: &TransportSecret,
    recipient_key: &TransportPublic,
    secrets: &[u8],
    rng: &mut impl CryptoRng,
) -> Result<Envelope, Error> {
    envelope::seal(
        &context(session, recipient),
        host,
        recipient_key,
        secrets,
        rng,
    )
}

/// Opens the secrets the host sealed for member `recipient` of the plan `session`.
///
/// # Errors
/// [`Error::EnvelopeOpen`] if the envelope was not sealed by `host` for this session and recipient, or was modified.
pub fn open_secrets(
    session: &[u8; 32],
    recipient: MemberIndex,
    recipient_secret: &TransportSecret,
    host: &TransportPublic,
    envelope: &Envelope,
) -> Result<Zeroizing<Vec<u8>>, Error> {
    envelope::open(
        &context(session, recipient),
        recipient_secret,
        host,
        envelope,
    )
}

#[cfg(test)]
mod tests {
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    use super::*;

    #[test]
    fn join_proofs_bind_secret_lobby_and_keys() {
        let proof = join_proof(&[1; 32], &[2; 16], &[3; 32], &[4; 32]);
        assert!(verify_join_proof(
            &[1; 32], &[2; 16], &[3; 32], &[4; 32], &proof
        ));
        assert!(!verify_join_proof(
            &[9; 32], &[2; 16], &[3; 32], &[4; 32], &proof
        ));
        assert!(!verify_join_proof(
            &[1; 32], &[9; 16], &[3; 32], &[4; 32], &proof
        ));
        assert!(!verify_join_proof(
            &[1; 32], &[2; 16], &[9; 32], &[4; 32], &proof
        ));
        assert!(!verify_join_proof(
            &[1; 32], &[2; 16], &[3; 32], &[9; 32], &proof
        ));
        let mut flipped = proof;
        flipped[31] ^= 1;
        assert!(!verify_join_proof(
            &[1; 32], &[2; 16], &[3; 32], &[4; 32], &flipped
        ));
    }

    #[test]
    fn join_proof_matches_hmac_sha256() {
        // Computed independently over the concatenated message.
        let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(&[1; 32]).unwrap();
        let mut message = JOIN_DOMAIN.to_vec();
        message.extend_from_slice(&[2; 16]);
        message.extend_from_slice(&[3; 32]);
        message.extend_from_slice(&[4; 32]);
        mac.update(&message);
        let expected: [u8; 32] = mac.finalize().into_bytes().into();
        assert_eq!(join_proof(&[1; 32], &[2; 16], &[3; 32], &[4; 32]), expected);
    }

    #[test]
    fn secrets_open_only_for_their_session_recipient_and_host() {
        let mut rng = ChaCha20Rng::seed_from_u64(41);
        let (host, host_pub) = TransportSecret::generate(&mut rng);
        let (guest, guest_pub) = TransportSecret::generate(&mut rng);
        let (other, other_pub) = TransportSecret::generate(&mut rng);
        let session = [7; 32];
        let sealed = seal_secrets(
            &session,
            2,
            &host,
            &guest_pub,
            b"protocol secrets",
            &mut rng,
        )
        .unwrap();
        assert_eq!(
            open_secrets(&session, 2, &guest, &host_pub, &sealed)
                .unwrap()
                .as_slice(),
            b"protocol secrets"
        );
        assert!(open_secrets(&[8; 32], 2, &guest, &host_pub, &sealed).is_err());
        assert!(open_secrets(&session, 3, &guest, &host_pub, &sealed).is_err());
        assert!(open_secrets(&session, 2, &other, &host_pub, &sealed).is_err());
        assert!(open_secrets(&session, 2, &guest, &other_pub, &sealed).is_err());
        // The same bytes under another envelope kind (a DKG share context) do not open.
        let dkg = Context {
            session,
            epoch: 0,
            kind: 1,
            sender: 1,
            recipient: 2,
        };
        assert!(envelope::open(&dkg, &guest, &host_pub, &sealed).is_err());
        let mut tampered = sealed;
        tampered.ciphertext[0] ^= 1;
        assert!(open_secrets(&session, 2, &guest, &host_pub, &tampered).is_err());
    }
}
