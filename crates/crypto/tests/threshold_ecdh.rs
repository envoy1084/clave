//! Threshold ECDH: dealt shares of real Aztec accounts reproduce Aztec's shared secret; proofs reject tampering.
// Test helpers unwrap freely; `ivsk` / `ivpk` are Aztec's names.
#![allow(clippy::unwrap_used, clippy::similar_names)]

mod common;

use common::{base, point, scalar, vectors};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use tecpan_crypto::{
    Error, Point, SecretScalar,
    aztec::AddressContext,
    ecdh::{AddressSecretShare, EcdhContext, Partial, combine, verify_and_combine, verify_partial},
    shamir::{MemberIndex, Polynomial, random_scalar},
};

struct Dealt {
    shares: Vec<AddressSecretShare>,
    context: AddressContext,
}

/// Deals `ivsk` into `t`-of-`n` viewing shares and maps them to address-secret shares.
fn deal(
    ivsk: &SecretScalar,
    preaddress: tecpan_crypto::Base,
    t: usize,
    n: u32,
    rng: &mut ChaCha20Rng,
) -> Dealt {
    let ivpk = Point::generator_mul_secret(ivsk).unwrap();
    let context = AddressContext::new(preaddress, &ivpk).unwrap();
    let poly = Polynomial::random(ivsk.clone(), t, rng).unwrap();
    let shares = (1..=n)
        .map(|i| AddressSecretShare::new(i, &poly.evaluate(i).unwrap(), &context).unwrap())
        .collect();
    Dealt { shares, context }
}

fn subsets(n: u32, t: usize) -> Vec<Vec<MemberIndex>> {
    let mut out = Vec::new();
    for mask in 0u32..(1 << n) {
        if mask.count_ones() as usize == t {
            out.push((1..=n).filter(|i| mask & (1 << (i - 1)) != 0).collect());
        }
    }
    out
}

#[test]
fn every_qualified_subset_reproduces_aztec_shared_secret() {
    let v = vectors();
    let mut rng = ChaCha20Rng::seed_from_u64(1);
    for (t, n) in [(2, 3), (3, 5)] {
        for account in v["accounts"].as_array().unwrap() {
            let ivsk = SecretScalar::new(scalar(&account["ivsk"]));
            let dealt = deal(&ivsk, base(&account["preaddress"]), t, n, &mut rng);
            let eph_pk = point(&account["ecdh"]["ephPk"]);
            let expected = point(&account["ecdh"]["sharedPoint"]);
            let ctx = EcdhContext {
                vault: dealt.context.address(),
                epoch: 7,
            };

            for set in subsets(n, t) {
                let partials: Vec<Partial> = set
                    .iter()
                    .map(|i| {
                        dealt.shares[(*i - 1) as usize]
                            .partial(&eph_pk, &ctx, &mut rng)
                            .unwrap()
                    })
                    .collect();
                for (partial, i) in partials.iter().zip(&set) {
                    verify_partial(
                        partial,
                        &eph_pk,
                        dealt.shares[(*i - 1) as usize].public(),
                        &ctx,
                    )
                    .unwrap();
                }
                assert_eq!(combine(&partials, t).unwrap(), expected, "subset {set:?}");
            }
        }
    }
}

#[test]
fn member_public_shares_match_address_verification_shares() {
    let mut rng = ChaCha20Rng::seed_from_u64(2);
    let ivsk = random_scalar(&mut rng);
    let ivpk = Point::generator_mul_secret(&ivsk).unwrap();
    let context = AddressContext::new(tecpan_crypto::Base::from(99u64), &ivpk).unwrap();
    let poly = Polynomial::random(ivsk, 2, &mut rng).unwrap();
    let commitments = poly.commitments().unwrap();
    for i in 1..=3 {
        let share = AddressSecretShare::new(i, &poly.evaluate(i).unwrap(), &context).unwrap();
        let y_i = commitments.evaluate(i).unwrap();
        assert_eq!(
            context.address_verification_share(&y_i).unwrap(),
            *share.public()
        );
    }
}

fn setup() -> (Dealt, Point, EcdhContext, ChaCha20Rng) {
    let mut rng = ChaCha20Rng::seed_from_u64(3);
    let ivsk = random_scalar(&mut rng);
    let dealt = deal(&ivsk, tecpan_crypto::Base::from(1234u64), 2, 3, &mut rng);
    let input = Point::generator_mul_secret(&random_scalar(&mut rng)).unwrap();
    let ctx = EcdhContext {
        vault: dealt.context.address(),
        epoch: 1,
    };
    (dealt, input, ctx, rng)
}

#[test]
fn tampered_partial_is_rejected() {
    let (dealt, input, ctx, mut rng) = setup();
    let mut partial = dealt.shares[0].partial(&input, &ctx, &mut rng).unwrap();
    partial.point = Point::generator();
    assert_eq!(
        verify_partial(&partial, &input, dealt.shares[0].public(), &ctx),
        Err(Error::InvalidProof(1))
    );
}

#[test]
fn proof_is_bound_to_member_epoch_vault_and_input() {
    let (dealt, input, ctx, mut rng) = setup();
    let partial = dealt.shares[0].partial(&input, &ctx, &mut rng).unwrap();
    let public = dealt.shares[0].public();

    assert!(verify_partial(&partial, &input, dealt.shares[1].public(), &ctx).is_err());
    assert!(verify_partial(&partial, &input, public, &EcdhContext { epoch: 2, ..ctx }).is_err());
    let other_vault = EcdhContext {
        vault: tecpan_crypto::Base::from(5u64),
        ..ctx
    };
    assert!(verify_partial(&partial, &input, public, &other_vault).is_err());
    assert!(verify_partial(&partial, &Point::generator(), public, &ctx).is_err());

    let mut relabelled = partial;
    relabelled.index = 2;
    assert!(verify_partial(&relabelled, &input, dealt.shares[1].public(), &ctx).is_err());
}

#[test]
fn verify_and_combine_skips_bad_partials_and_needs_threshold() {
    let (dealt, input, ctx, mut rng) = setup();
    let publics: Vec<Point> = dealt.shares.iter().map(|s| *s.public()).collect();
    let lookup = |i: MemberIndex| publics.get((i as usize).checked_sub(1)?).copied();

    let mut bad = dealt.shares[0].partial(&input, &ctx, &mut rng).unwrap();
    bad.point = Point::generator();
    let good_2 = dealt.shares[1].partial(&input, &ctx, &mut rng).unwrap();
    let good_3 = dealt.shares[2].partial(&input, &ctx, &mut rng).unwrap();

    let (result, invalid) =
        verify_and_combine(&[bad, good_2, good_3], &input, lookup, 2, &ctx).unwrap();
    assert_eq!(invalid, vec![1]);
    assert_eq!(result, combine(&[good_2, good_3], 2).unwrap());

    assert_eq!(
        verify_and_combine(&[bad, good_2], &input, lookup, 2, &ctx).err(),
        Some(Error::InsufficientShares {
            needed: 2,
            actual: 1
        })
    );
    assert_eq!(
        combine(&[good_2, good_2], 2).err(),
        Some(Error::DuplicateIndex(2))
    );
}
