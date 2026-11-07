//! Property tests over random thresholds, committees, subsets and inputs.
#![allow(
    clippy::unwrap_used,
    clippy::similar_names,
    clippy::cast_possible_truncation
)]

mod common;

use ark_ff::PrimeField;
use clave_crypto::{
    Base, Point, Scalar, SecretScalar,
    aztec::{AddressContext, single_key_address_secret},
    curve,
    dkg::KeyShare,
    ecdh::{AddressSecretShare, EcdhContext, Partial, combine},
    reshare::{self, Parameters, SubShare},
    shamir::{MemberIndex, Polynomial, lagrange_coefficient, random_scalar},
};
use proptest::prelude::*;
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

/// Picks `k` distinct members of `1..=n` using `seed`.
fn subset(n: u32, k: usize, seed: u64) -> Vec<MemberIndex> {
    let mut all: Vec<MemberIndex> = (1..=n).collect();
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    for i in (1..all.len()).rev() {
        let j = (rand_core::Rng::next_u64(&mut rng) % (i as u64 + 1)) as usize;
        all.swap(i, j);
    }
    all.truncate(k);
    all
}

fn ecdh(
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

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, ..ProptestConfig::default() })]

    #[test]
    fn shamir_reconstructs_from_any_threshold_subset(t in 1usize..=6, extra in 0u32..=3, seed: u64) {
        let n = t as u32 + extra;
        let mut rng = ChaCha20Rng::seed_from_u64(seed);
        let secret = random_scalar(&mut rng);
        let poly = Polynomial::random(secret.clone(), t, &mut rng).unwrap();
        let set = subset(n, t, seed ^ 1);
        let value: Scalar = set
            .iter()
            .map(|&i| lagrange_coefficient(i, &set).unwrap() * poly.evaluate(i).unwrap().expose())
            .sum();
        prop_assert_eq!(value, *secret.expose());
    }

    #[test]
    fn threshold_ecdh_equals_single_key_ecdh(t in 2usize..=5, extra in 0u32..=3, seed: u64, preaddress: u64) {
        let n = t as u32 + extra;
        let mut rng = ChaCha20Rng::seed_from_u64(seed);
        let ivsk = random_scalar(&mut rng);
        let ivpk = Point::generator_mul_secret(&ivsk).unwrap();
        let context = AddressContext::new(Base::from(preaddress), &ivpk).unwrap();
        let poly = Polynomial::random(ivsk.clone(), t, &mut rng).unwrap();
        let input = Point::generator_mul_secret(&random_scalar(&mut rng)).unwrap();
        let ctx = EcdhContext { vault: context.address(), epoch: 1 };

        let partials: Vec<Partial> = subset(n, t, seed ^ 2)
            .into_iter()
            .map(|i| {
                let share = AddressSecretShare::new(i, &poly.evaluate(i).unwrap(), &context).unwrap();
                share.partial(&input, &ctx, &mut rng).unwrap()
            })
            .collect();
        let expected = input.mul_secret(&single_key_address_secret(Base::from(preaddress), &ivsk).unwrap()).unwrap();
        prop_assert_eq!(combine(&partials, t).unwrap(), expected);
    }

    #[test]
    fn rotation_preserves_group_key_and_ecdh(
        t in 2usize..=4,
        extra in 0u32..=2,
        removed in 0usize..=3,
        joined in 0u32..=2,
        new_t_offset in 0usize..=2,
        seed: u64,
    ) {
        let n = t as u32 + extra;
        let removed = removed.min(t - 1).min(n as usize - 1);
        let mut rng = ChaCha20Rng::seed_from_u64(seed);
        let old = common::run_dkg(t, n, &mut rng);
        let context = AddressContext::new(Base::from(seed), old[0].group_public_key()).unwrap();
        let input = Point::generator_mul_secret(&random_scalar(&mut rng)).unwrap();
        let before = ecdh(&old.iter().take(t).collect::<Vec<_>>(), &context, &input, &mut rng);

        // Stayers are a random subset of n - removed members; t of them deal.
        let stayers = subset(n, n as usize - removed, seed ^ 3);
        let new_n = stayers.len() as u32 + joined;
        let new_t = (2 + new_t_offset).min(new_n as usize).max(1);
        let params = Parameters {
            session: [5u8; 32],
            dealers: stayers[..t.min(stayers.len())].to_vec(),
            old_threshold: t,
            new_participants: (1..=new_n).collect(),
            new_threshold: new_t,
        };
        prop_assume!(params.dealers.len() == t);

        let dealt: Vec<_> = params
            .dealers
            .iter()
            .map(|d| reshare::deal(&params, old.iter().find(|s| s.index() == *d).unwrap(), &mut rng).unwrap())
            .collect();
        let packages: Vec<_> = dealt.iter().map(|(p, _)| p.clone()).collect();
        let new: Vec<KeyShare> = (1..=new_n)
            .map(|j| {
                let mine: Vec<SubShare> = dealt
                    .iter()
                    .flat_map(|(_, s)| s.iter())
                    .filter(|s| s.recipient == j)
                    .map(|s| SubShare { dealer: s.dealer, recipient: j, share: s.share.clone() })
                    .collect();
                reshare::receive(&params, old[0].group_commitments(), j, &packages, &mine).unwrap()
            })
            .collect();

        prop_assert_eq!(new[0].group_public_key(), old[0].group_public_key());
        let chosen: Vec<&KeyShare> = subset(new_n, new_t, seed ^ 4).into_iter().map(|j| &new[(j - 1) as usize]).collect();
        prop_assert_eq!(ecdh(&chosen, &context, &input, &mut rng), before);
    }

    #[test]
    fn constant_time_multiplication_matches_arkworks(seed: u64, k_bytes: [u8; 32]) {
        let mut rng = ChaCha20Rng::seed_from_u64(seed);
        let base_point = Point::generator_mul_secret(&random_scalar(&mut rng)).unwrap();
        let k = Scalar::from_le_bytes_mod_order(&k_bytes);
        let public = Point::try_from(base_point.mul_public(&k));
        let secret = base_point.mul_secret(&SecretScalar::new(k));
        prop_assert_eq!(public, secret);
    }

    #[test]
    fn decoders_never_panic_and_round_trip(bytes in proptest::collection::vec(any::<u8>(), 0..80)) {
        if let Ok(array) = <[u8; 64]>::try_from(bytes.as_slice())
            && let Ok(point) = Point::from_bytes(&array)
        {
            prop_assert_eq!(Point::from_bytes(&point.to_bytes()).unwrap(), point);
        }
        if let Ok(value) = curve::scalar_from_bytes(&bytes) {
            prop_assert_eq!(curve::scalar_to_bytes(&value).to_vec(), bytes.clone());
        }
        if let Ok(value) = curve::base_from_bytes(&bytes) {
            prop_assert_eq!(curve::base_to_bytes(&value).to_vec(), bytes);
        }
    }
}
