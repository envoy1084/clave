//! Distributed key generation: agreement, correctness end to end through threshold ECDH, and culprit detection.
#![allow(clippy::unwrap_used, clippy::similar_names)]

mod common;

use ark_ff::One;
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use tecpan_crypto::{
    Base, Error, Point, Scalar, SecretScalar,
    aztec::AddressContext,
    dkg::{self, KeyShare, Parameters, Round1Package, Round2Package, complaint_is_valid},
    ecdh::{AddressSecretShare, EcdhContext, combine},
    shamir::{MemberIndex, lagrange_coefficient, random_scalar},
};

fn parameters(t: usize, n: u32) -> Parameters {
    Parameters {
        session: [7u8; 32],
        threshold: t,
        participants: (1..=n).collect(),
    }
}

/// Runs round 1 and round 2 for every member; returns secrets, broadcast packages and private packages.
fn rounds(
    params: &Parameters,
    rng: &mut ChaCha20Rng,
) -> (
    Vec<dkg::Round1Secret>,
    Vec<Round1Package>,
    Vec<Round2Package>,
) {
    let (secrets, round1): (Vec<_>, Vec<_>) = params
        .participants
        .iter()
        .map(|&i| dkg::round1(params, i, rng).unwrap())
        .unzip();
    let round2 = secrets
        .iter()
        .flat_map(|s| dkg::round2(params, s, &round1).unwrap())
        .collect();
    (secrets, round1, round2)
}

fn interpolate(shares: &[&KeyShare]) -> Scalar {
    let set: Vec<MemberIndex> = shares.iter().map(|s| s.index()).collect();
    shares
        .iter()
        .map(|s| lagrange_coefficient(s.index(), &set).unwrap() * s.share().expose())
        .sum()
}

#[test]
fn members_agree_and_shares_reconstruct_the_group_key() {
    let mut rng = ChaCha20Rng::seed_from_u64(11);
    for (t, n) in [(2, 3), (3, 5), (8, 8)] {
        let shares = common::run_dkg(t, n, &mut rng);
        let ivpk = *shares[0].group_public_key();
        for share in &shares {
            assert_eq!(*share.group_public_key(), ivpk);
            assert_eq!(share.transcript(), shares[0].transcript());
            assert_eq!(
                share.verification_shares().unwrap(),
                shares[0].verification_shares().unwrap()
            );
            assert_eq!(
                Point::generator_mul_secret(share.share()).unwrap(),
                share.verification_share(share.index()).unwrap()
            );
        }
        let first_t: Vec<&KeyShare> = shares.iter().take(t).collect();
        let secret = SecretScalar::new(interpolate(&first_t));
        assert_eq!(Point::generator_mul_secret(&secret).unwrap(), ivpk);
    }
}

#[test]
fn dkg_shares_drive_threshold_ecdh_end_to_end() {
    let mut rng = ChaCha20Rng::seed_from_u64(12);
    let shares = common::run_dkg(2, 3, &mut rng);
    let preaddress = Base::from(0xabcdu64);
    let context = AddressContext::new(preaddress, shares[0].group_public_key()).unwrap();
    let ecdh = EcdhContext {
        vault: context.address(),
        epoch: 0,
    };
    let input = Point::generator_mul_secret(&random_scalar(&mut rng)).unwrap();

    // Reference: the secret the committee holds (test-only reconstruction).
    let ivsk = SecretScalar::new(interpolate(&[&shares[0], &shares[2]]));
    let reference = tecpan_crypto::aztec::single_key_address_secret(preaddress, &ivsk).unwrap();
    let expected = input.mul_secret(&reference).unwrap();

    let members: Vec<AddressSecretShare> = shares
        .iter()
        .map(|s| AddressSecretShare::new(s.index(), s.share(), &context).unwrap())
        .collect();
    for pair in [[0, 1], [0, 2], [1, 2]] {
        let partials: Vec<_> = pair
            .iter()
            .map(|&k| members[k].partial(&input, &ecdh, &mut rng).unwrap())
            .collect();
        assert_eq!(combine(&partials, 2).unwrap(), expected);
    }
}

#[test]
fn bad_proof_of_knowledge_names_the_dealer() {
    let mut rng = ChaCha20Rng::seed_from_u64(13);
    let params = parameters(2, 3);
    let (secrets, mut round1, _) = rounds(&params, &mut rng);
    round1[1].proof.response += Scalar::one();
    assert_eq!(
        dkg::round2(&params, &secrets[0], &round1).err(),
        Some(Error::InvalidProof(2))
    );
}

#[test]
fn proofs_are_bound_to_the_session() {
    let mut rng = ChaCha20Rng::seed_from_u64(14);
    let params = parameters(2, 3);
    let (secrets, round1, _) = rounds(&params, &mut rng);
    let other_session = Parameters {
        session: [8u8; 32],
        ..params
    };
    assert!(matches!(
        dkg::round2(&other_session, &secrets[0], &round1),
        Err(Error::InvalidProof(_))
    ));
}

#[test]
fn bad_share_names_the_dealer_and_the_complaint_verifies() {
    let mut rng = ChaCha20Rng::seed_from_u64(15);
    let params = parameters(2, 3);
    let (mut secrets, round1, mut round2) = rounds(&params, &mut rng);

    let tampered = round2
        .iter_mut()
        .find(|p| p.dealer == 3 && p.recipient == 1)
        .unwrap();
    tampered.share = SecretScalar::new(*tampered.share.expose() + Scalar::one());
    let evidence = tampered.share.clone();

    let member_1 = secrets.remove(0);
    assert_eq!(
        dkg::finalize(&params, member_1, &round1, &round2).err(),
        Some(Error::InvalidShare(3))
    );
    assert!(complaint_is_valid(&round1[2], 1, &evidence));

    let honest = round2
        .iter()
        .find(|p| p.dealer == 2 && p.recipient == 1)
        .unwrap();
    assert!(!complaint_is_valid(&round1[1], 1, &honest.share));
}

#[test]
fn rejects_wrong_threshold_and_missing_packages() {
    let mut rng = ChaCha20Rng::seed_from_u64(16);
    let params = parameters(2, 3);
    let (secrets, round1, _) = rounds(&params, &mut rng);

    let three = parameters(3, 3);
    assert_eq!(
        dkg::round2(&three, &secrets[0], &round1).err(),
        Some(Error::InvalidThreshold)
    );
    assert!(matches!(
        dkg::round2(&params, &secrets[0], &round1[..2]),
        Err(Error::InsufficientShares { .. })
    ));
    assert_eq!(
        dkg::round1(&parameters(4, 3), 1, &mut rng).err(),
        Some(Error::InvalidThreshold)
    );
    assert_eq!(
        dkg::round1(&params, 9, &mut rng).err(),
        Some(Error::InvalidIndex)
    );
}

#[test]
fn stored_shares_restore_and_wrong_shares_are_refused() {
    let mut rng = rand_chacha::ChaCha20Rng::seed_from_u64(91);
    let shares = common::run_dkg(2, 3, &mut rng);
    let share = &shares[1];
    let restored = tecpan_crypto::dkg::KeyShare::restore(
        share.index(),
        share.threshold(),
        share.participants().to_vec(),
        share.share().clone(),
        share.group_commitments().clone(),
        *share.transcript(),
    )
    .unwrap();
    assert_eq!(restored.group_public_key(), share.group_public_key());

    let wrong = tecpan_crypto::dkg::KeyShare::restore(
        share.index(),
        share.threshold(),
        share.participants().to_vec(),
        shares[0].share().clone(),
        share.group_commitments().clone(),
        *share.transcript(),
    );
    assert!(matches!(wrong, Err(tecpan_crypto::Error::InvalidShare(2))));

    // A manifest's verification shares rebuild the group commitments.
    let evaluations: Vec<_> = share
        .participants()
        .iter()
        .map(|&i| (i, share.verification_share(i).unwrap()))
        .collect();
    assert_eq!(
        &tecpan_crypto::shamir::Commitments::interpolate(2, &evaluations).unwrap(),
        share.group_commitments()
    );
}
