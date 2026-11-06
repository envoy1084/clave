//! Resharing: same group key and ECDH output across committees; old and new shares never mix; culprits named.
#![allow(clippy::unwrap_used, clippy::similar_names)]

mod common;

use ark_ff::One;
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use tecpan_crypto::{
    Base, Error, Point, Scalar, SecretScalar,
    aztec::AddressContext,
    dkg::KeyShare,
    ecdh::{AddressSecretShare, EcdhContext, Partial, combine},
    reshare::{self, Parameters, SubShare},
    shamir::{MemberIndex, random_scalar},
    vault,
};

fn parameters(dealers: Vec<MemberIndex>, old_t: usize, new_n: u32, new_t: usize) -> Parameters {
    Parameters {
        session: [9u8; 32],
        dealers,
        old_threshold: old_t,
        new_participants: (1..=new_n).collect(),
        new_threshold: new_t,
    }
}

/// Runs a reshare from `old` (dealers by index) to a new committee of `new_n` members.
fn run_reshare(old: &[KeyShare], params: &Parameters, rng: &mut ChaCha20Rng) -> Vec<KeyShare> {
    let dealt: Vec<_> = params
        .dealers
        .iter()
        .map(|d| reshare::deal(params, old.iter().find(|s| s.index() == *d).unwrap(), rng).unwrap())
        .collect();
    let packages: Vec<_> = dealt.iter().map(|(p, _)| p.clone()).collect();
    let sub_shares: Vec<&SubShare> = dealt.iter().flat_map(|(_, s)| s.iter()).collect();
    params
        .new_participants
        .iter()
        .map(|&j| {
            let mine: Vec<SubShare> = sub_shares
                .iter()
                .filter(|s| s.recipient == j)
                .map(|s| SubShare {
                    dealer: s.dealer,
                    recipient: s.recipient,
                    share: s.share.clone(),
                })
                .collect();
            reshare::receive(params, old[0].group_commitments(), j, &packages, &mine).unwrap()
        })
        .collect()
}

fn ecdh_with(
    shares: &[&KeyShare],
    context: &AddressContext,
    input: &Point,
    rng: &mut ChaCha20Rng,
) -> Point {
    let ctx = EcdhContext {
        vault: context.address(),
        epoch: 0,
    };
    let partials: Vec<Partial> = shares
        .iter()
        .map(|s| {
            AddressSecretShare::new(s.index(), s.share(), context)
                .unwrap()
                .partial(input, &ctx, rng)
                .unwrap()
        })
        .collect();
    combine(&partials, shares.len()).unwrap()
}

#[test]
fn removing_and_adding_a_member_keeps_the_key_and_the_ecdh_output() {
    let mut rng = ChaCha20Rng::seed_from_u64(21);
    let old = common::run_dkg(2, 3, &mut rng);
    let context = AddressContext::new(Base::from(77u64), old[0].group_public_key()).unwrap();
    let input = Point::generator_mul_secret(&random_scalar(&mut rng)).unwrap();
    let before = ecdh_with(&[&old[0], &old[1]], &context, &input, &mut rng);

    // Members 1 and 2 stay and deal; member 3 leaves; a newcomer joins. New committee: 1, 2, 3 (newcomer is 3).
    let params = parameters(vec![1, 2], 2, 3, 2);
    let new = run_reshare(&old, &params, &mut rng);

    for share in &new {
        assert_eq!(share.group_public_key(), old[0].group_public_key());
        assert_eq!(share.transcript(), new[0].transcript());
    }
    for pair in [[0, 1], [0, 2], [1, 2]] {
        assert_eq!(
            ecdh_with(&[&new[pair[0]], &new[pair[1]]], &context, &input, &mut rng),
            before
        );
    }

    // The leaver's old share does not combine with a new share.
    let ctx = EcdhContext {
        vault: context.address(),
        epoch: 0,
    };
    let old_partial = AddressSecretShare::new(3, old[2].share(), &context)
        .unwrap()
        .partial(&input, &ctx, &mut rng)
        .unwrap();
    let new_partial = AddressSecretShare::new(1, new[0].share(), &context)
        .unwrap()
        .partial(&input, &ctx, &mut rng)
        .unwrap();
    assert_ne!(combine(&[new_partial, old_partial], 2).unwrap(), before);
}

#[test]
fn changes_threshold_in_both_directions() {
    let mut rng = ChaCha20Rng::seed_from_u64(22);
    let input = Point::generator_mul_secret(&random_scalar(&mut rng)).unwrap();

    let old = common::run_dkg(2, 3, &mut rng);
    let context = AddressContext::new(Base::from(5u64), old[0].group_public_key()).unwrap();
    let before = ecdh_with(&[&old[1], &old[2]], &context, &input, &mut rng);
    let grown = run_reshare(&old, &parameters(vec![2, 3], 2, 4, 3), &mut rng);
    assert_eq!(
        ecdh_with(
            &[&grown[0], &grown[2], &grown[3]],
            &context,
            &input,
            &mut rng
        ),
        before
    );

    let shrunk = run_reshare(&grown, &parameters(vec![1, 2, 4], 3, 2, 2), &mut rng);
    assert_eq!(
        ecdh_with(&[&shrunk[0], &shrunk[1]], &context, &input, &mut rng),
        before
    );
}

#[test]
fn new_members_agree_on_the_committee_commitment() {
    let mut rng = ChaCha20Rng::seed_from_u64(23);
    let old = common::run_dkg(2, 3, &mut rng);
    let new = run_reshare(&old, &parameters(vec![1, 3], 2, 3, 2), &mut rng);
    let commitments: Vec<Base> = new
        .iter()
        .map(|s| vault::committee_commitment(2, &s.verification_shares().unwrap()).unwrap())
        .collect();
    assert!(commitments.iter().all(|c| *c == commitments[0]));
    let old_commitment =
        vault::committee_commitment(2, &old[0].verification_shares().unwrap()).unwrap();
    assert_ne!(commitments[0], old_commitment);
}

#[test]
fn dealer_with_a_wrong_constant_term_is_named() {
    let mut rng = ChaCha20Rng::seed_from_u64(24);
    let old = common::run_dkg(2, 3, &mut rng);
    let params = parameters(vec![1, 2], 2, 3, 2);
    let (mut bad, _) = reshare::deal(&params, &old[0], &mut rng).unwrap();
    let (other, _) = reshare::deal(&params, &old[1], &mut rng).unwrap();
    bad.commitments = other.commitments;
    assert_eq!(
        reshare::verify_dealer(&params, old[0].group_commitments(), &bad),
        Err(Error::InvalidProof(1))
    );
}

#[test]
fn bad_sub_share_is_named() {
    let mut rng = ChaCha20Rng::seed_from_u64(25);
    let old = common::run_dkg(2, 3, &mut rng);
    let params = parameters(vec![1, 2], 2, 3, 2);
    let (p1, s1) = reshare::deal(&params, &old[0], &mut rng).unwrap();
    let (p2, s2) = reshare::deal(&params, &old[1], &mut rng).unwrap();

    let mut for_member_1: Vec<SubShare> = s1
        .into_iter()
        .chain(s2)
        .filter(|s| s.recipient == 1)
        .collect();
    for_member_1[1].share = SecretScalar::new(*for_member_1[1].share.expose() + Scalar::one());
    assert_eq!(
        reshare::receive(
            &params,
            old[0].group_commitments(),
            1,
            &[p1, p2],
            &for_member_1
        )
        .err(),
        Some(Error::InvalidShare(2))
    );
}

#[test]
fn rejects_wrong_dealer_sets() {
    let mut rng = ChaCha20Rng::seed_from_u64(26);
    let old = common::run_dkg(2, 3, &mut rng);
    assert_eq!(
        reshare::deal(&parameters(vec![1], 2, 3, 2), &old[0], &mut rng).err(),
        Some(Error::InsufficientShares {
            needed: 2,
            actual: 1
        })
    );
    assert_eq!(
        reshare::deal(&parameters(vec![1, 2], 2, 3, 2), &old[2], &mut rng).err(),
        Some(Error::InvalidIndex)
    );
    assert_eq!(
        reshare::deal(&parameters(vec![1, 2], 2, 3, 4), &old[0], &mut rng).err(),
        Some(Error::InvalidThreshold)
    );
}
