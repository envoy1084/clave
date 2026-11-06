//! Benchmarks for the operations agents and engines run: partials, verify + combine per sync batch, ceremonies.
#![allow(missing_docs, clippy::unwrap_used)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use std::hint::black_box;
use tecpan_crypto::{
    Base, Point,
    aztec::AddressContext,
    dkg::{self, KeyShare, Parameters},
    ecdh::{AddressSecretShare, EcdhContext, verify_and_combine},
    reshare,
    shamir::{MemberIndex, random_scalar},
};

fn run_dkg(t: usize, n: u32, rng: &mut ChaCha20Rng) -> Vec<KeyShare> {
    let params = Parameters {
        session: [1u8; 32],
        threshold: t,
        participants: (1..=n).collect(),
    };
    let (secrets, round1): (Vec<_>, Vec<_>) = params
        .participants
        .iter()
        .map(|&i| dkg::round1(&params, i, rng).unwrap())
        .unzip();
    let round2: Vec<_> = secrets
        .iter()
        .flat_map(|s| dkg::round2(&params, s, &round1).unwrap())
        .collect();
    secrets
        .into_iter()
        .map(|s| dkg::finalize(&params, s, &round1, &round2).unwrap())
        .collect()
}

fn ecdh_benches(c: &mut Criterion) {
    let mut rng = ChaCha20Rng::seed_from_u64(1);
    let shares = run_dkg(2, 3, &mut rng);
    let context = AddressContext::new(Base::from(42u64), shares[0].group_public_key()).unwrap();
    let ctx = EcdhContext {
        vault: context.address(),
        epoch: 0,
    };
    let members: Vec<AddressSecretShare> = shares
        .iter()
        .map(|s| AddressSecretShare::new(s.index(), s.share(), &context).unwrap())
        .collect();
    let publics: Vec<Point> = members.iter().map(|m| *m.public()).collect();
    let lookup = |i: MemberIndex| publics.get((i as usize).checked_sub(1)?).copied();
    let input = Point::generator_mul_secret(&random_scalar(&mut rng)).unwrap();

    c.bench_function("ecdh/partial_with_proof", |b| {
        b.iter(|| {
            members[0]
                .partial(black_box(&input), &ctx, &mut rng)
                .unwrap()
        });
    });

    let mut group = c.benchmark_group("ecdh/verify_and_combine_t2");
    group.sample_size(10);
    for batch in [1usize, 100, 1000] {
        let inputs: Vec<Point> = (0..batch)
            .map(|_| Point::generator_mul_secret(&random_scalar(&mut rng)).unwrap())
            .collect();
        let partials: Vec<_> = inputs
            .iter()
            .map(|p| {
                [
                    members[0].partial(p, &ctx, &mut rng).unwrap(),
                    members[1].partial(p, &ctx, &mut rng).unwrap(),
                ]
            })
            .collect();
        group.bench_with_input(BenchmarkId::from_parameter(batch), &batch, |b, _| {
            b.iter(|| {
                for (input, pair) in inputs.iter().zip(&partials) {
                    black_box(verify_and_combine(pair, input, lookup, 2, &ctx).unwrap());
                }
            });
        });
    }
    group.finish();
}

fn ceremony_benches(c: &mut Criterion) {
    let mut group = c.benchmark_group("ceremony");
    group.sample_size(10);
    for (t, n) in [(2usize, 3u32), (3, 5), (5, 8)] {
        group.bench_with_input(
            BenchmarkId::new("dkg_all_members", format!("{t}-of-{n}")),
            &(t, n),
            |b, &(t, n)| {
                let mut rng = ChaCha20Rng::seed_from_u64(2);
                b.iter(|| run_dkg(t, n, &mut rng));
            },
        );
    }
    let mut rng = ChaCha20Rng::seed_from_u64(3);
    let old = run_dkg(2, 3, &mut rng);
    let params = reshare::Parameters {
        session: [2u8; 32],
        dealers: vec![1, 2],
        old_threshold: 2,
        new_participants: vec![1, 2, 3],
        new_threshold: 2,
    };
    group.bench_function("reshare_deal_and_receive_2-of-3", |b| {
        b.iter(|| {
            let dealt: Vec<_> = [0usize, 1]
                .iter()
                .map(|&k| reshare::deal(&params, &old[k], &mut rng).unwrap())
                .collect();
            let packages: Vec<_> = dealt.iter().map(|(p, _)| p.clone()).collect();
            for j in 1..=3 {
                let mine: Vec<_> = dealt
                    .iter()
                    .flat_map(|(_, s)| s.iter())
                    .filter(|s| s.recipient == j)
                    .map(|s| reshare::SubShare {
                        dealer: s.dealer,
                        recipient: j,
                        share: s.share.clone(),
                    })
                    .collect();
                black_box(
                    reshare::receive(&params, old[0].group_commitments(), j, &packages, &mine)
                        .unwrap(),
                );
            }
        });
    });
    group.finish();
}

criterion_group!(benches, ecdh_benches, ceremony_benches);
criterion_main!(benches);
