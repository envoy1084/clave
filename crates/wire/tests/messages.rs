//! Message validation and conversions to and from `tecpan-crypto` types, with real cryptography.
#![allow(clippy::unwrap_used)]

mod common;

use common::{point, proposal, samples, wire_point};
use rand_core::SeedableRng;
use tecpan_crypto::{
    Point, Scalar, SecretScalar,
    aztec::AddressContext,
    dkg,
    ecdh::{self, AddressSecretShare, EcdhContext},
    schnorr,
    shamir::Commitments,
    vault,
};
use tecpan_wire::{
    Error, Message,
    messages::{
        Approval, Ceremony, DkgRound1, EcdhResponse, Justification, MAX_REASON_BYTES, Manifest,
        Rejection, ReshareDeal,
    },
    types::{field, scalar},
};

fn rng() -> rand_chacha::ChaCha20Rng {
    rand_chacha::ChaCha20Rng::seed_from_u64(42)
}

fn manifest() -> Manifest {
    match samples().remove(0).1 {
        Message::Manifest(manifest) => manifest,
        _ => unreachable!(),
    }
}

#[test]
fn manifest_structure_is_validated() {
    manifest().validate().unwrap();

    let mut m = manifest();
    m.members.swap(0, 1);
    assert!(matches!(
        m.validate(),
        Err(Error::OutOfBounds {
            field: "members.index"
        })
    ));

    let mut m = manifest();
    m.threshold = 4;
    assert!(matches!(
        m.validate(),
        Err(Error::OutOfBounds { field: "threshold" })
    ));

    let mut m = manifest();
    m.viewing_threshold = 1;
    assert!(matches!(
        m.validate(),
        Err(Error::OutOfBounds {
            field: "viewing_threshold"
        })
    ));

    let mut m = manifest();
    m.members[2].identity = m.members[0].identity;
    assert!(matches!(
        m.validate(),
        Err(Error::OutOfBounds {
            field: "members.identity"
        })
    ));

    let mut m = manifest();
    m.members[1].addresses = vec!["x".repeat(257)];
    assert!(matches!(
        m.validate(),
        Err(Error::OutOfBounds {
            field: "members.addresses"
        })
    ));

    let mut m = manifest();
    m.members.clear();
    assert!(matches!(
        m.validate(),
        Err(Error::OutOfBounds { field: "members" })
    ));
}

#[test]
fn manifest_commitments_match_the_contract_vectors() {
    // `wire_vectors.nr` / `tests/vault_wire.rs`: signer keys are `i·G` and the committee is `t_v = 2` over `(1000+i)·G`
    // for i in 1..=3.
    let m = manifest();
    let signers: Vec<Point> = (1..=3).map(point).collect();
    let shares: Vec<Point> = (1001..=1003).map(point).collect();
    assert_eq!(
        m.signer_set_commitment().unwrap(),
        vault::signer_set_commitment(&signers).unwrap()
    );
    assert_eq!(
        m.committee_commitment().unwrap(),
        vault::committee_commitment(2, &shares).unwrap()
    );
}

#[test]
fn manifest_address_must_match_preaddress_and_group_key() {
    let mut m = manifest();
    assert!(matches!(
        m.address_context(),
        Err(Error::OutOfBounds { field: "vault" })
    ));
    let preaddress = tecpan_crypto::Base::from(77u64);
    let context = AddressContext::new(preaddress, &point(1000)).unwrap();
    m.preaddress = field(&preaddress);
    m.vault = field(&context.address());
    assert_eq!(m.address_context().unwrap().address(), context.address());
}

#[test]
fn approvals_verify_against_their_proposal() {
    let mut rng = rng();
    let key = SecretScalar::new(Scalar::from(31u64));
    let public = Point::generator_mul_secret(&key).unwrap();
    let vault = tecpan_crypto::Base::from(0x1234u64);
    let proposal = proposal();
    let id = proposal.id().unwrap();
    let signature =
        schnorr::sign(&key, proposal.approval_message(vault).unwrap(), &mut rng).unwrap();
    let approval = Approval::new(id, 0, &signature);

    assert!(approval.verify(&proposal, vault, &public).unwrap());
    // Other vault, other signer, other proposal.
    assert!(
        !approval
            .verify(&proposal, tecpan_crypto::Base::from(1u64), &public)
            .unwrap()
    );
    assert!(!approval.verify(&proposal, vault, &point(5)).unwrap());
    let mut other = proposal.clone();
    other.summary.push('!');
    assert!(!approval.verify(&other, vault, &public).unwrap());
}

#[test]
fn rejections_verify_against_their_proposal_and_vault() {
    let mut rng = rng();
    let key = SecretScalar::new(Scalar::from(31u64));
    let public = Point::generator_mul_secret(&key).unwrap();
    let vault = tecpan_crypto::Base::from(0x1234u64);
    let id = proposal().id().unwrap();
    let signature = schnorr::sign(&key, vault::rejection_message(vault, &id), &mut rng).unwrap();
    let limbs = signature.to_limbs();
    let rejection = Rejection {
        proposal: id,
        slot: 0,
        reason: "wrong recipient".into(),
        signature: [
            field(&limbs[0]),
            field(&limbs[1]),
            field(&limbs[2]),
            field(&limbs[3]),
        ],
    };

    assert!(rejection.verify(vault, &public).unwrap());
    // Other vault, other signer, other proposal.
    assert!(
        !rejection
            .verify(tecpan_crypto::Base::from(1u64), &public)
            .unwrap()
    );
    assert!(!rejection.verify(vault, &point(5)).unwrap());
    let mut other = rejection.clone();
    other.proposal[31] ^= 1;
    assert!(!other.verify(vault, &public).unwrap());
    // An approval signature is not a rejection (different domain).
    let approval =
        schnorr::sign(&key, proposal().approval_message(vault).unwrap(), &mut rng).unwrap();
    let limbs = approval.to_limbs();
    let mut swapped = rejection.clone();
    swapped.signature = [
        field(&limbs[0]),
        field(&limbs[1]),
        field(&limbs[2]),
        field(&limbs[3]),
    ];
    assert!(!swapped.verify(vault, &public).unwrap());
}

#[test]
fn rejection_reasons_are_bounded() {
    let body = Message::Rejection(Rejection {
        proposal: [1; 32],
        slot: 0,
        reason: "x".repeat(MAX_REASON_BYTES + 1),
        signature: [[0; 32]; 4],
    });
    assert!(matches!(
        body.validate(),
        Err(Error::OutOfBounds { field: "reason" })
    ));
}

#[test]
fn proposal_id_covers_every_field() {
    let base = proposal().id().unwrap();
    let mut changed = proposal();
    changed.payload.push(0);
    assert_ne!(changed.id().unwrap(), base);
    let mut changed = proposal();
    changed.fee_method = 0;
    assert_ne!(changed.id().unwrap(), base);
    let mut changed = proposal();
    changed.tx_nonce[0] ^= 1;
    assert_ne!(changed.id().unwrap(), base);
}

#[test]
fn dkg_packages_survive_the_wire() {
    let mut rng = rng();
    let parameters = dkg::Parameters {
        session: [7; 32],
        threshold: 2,
        participants: vec![1, 2, 3],
    };
    let packages: Vec<_> = (1..=3)
        .map(|i| dkg::round1(&parameters, i, &mut rng).unwrap())
        .collect();
    let decoded: Vec<_> = packages
        .iter()
        .map(|(_, package)| dkg::Round1Package::try_from(&DkgRound1::from(package)).unwrap())
        .collect();
    // The decoded packages still pass round-2 verification.
    let (secret, _) = &packages[0];
    dkg::round2(&parameters, secret, &decoded).unwrap();

    let mut bad = DkgRound1::from(&packages[1].1);
    bad.commitments[0].y[31] ^= 1;
    assert!(matches!(
        dkg::Round1Package::try_from(&bad),
        Err(Error::Crypto(_))
    ));
}

#[test]
fn reshare_deals_survive_the_wire() {
    let commitments = Commitments::new(vec![point(3), point(4)]).unwrap();
    let package = tecpan_crypto::reshare::DealerPackage {
        dealer: 2,
        commitments,
    };
    let decoded =
        tecpan_crypto::reshare::DealerPackage::try_from(&ReshareDeal::from(&package)).unwrap();
    assert_eq!(decoded.dealer, 2);
    assert_eq!(decoded.commitments.points(), package.commitments.points());
}

#[test]
fn justifications_settle_complaints() {
    let commitments = Commitments::new(vec![point(10), point(20)]).unwrap();
    // f(x) = 10 + 20x, so the share for member 3 is 70.
    let good = Justification {
        ceremony: Ceremony::Dkg,
        complainant: 3,
        share: scalar(&Scalar::from(70u64)),
    };
    assert!(good.holds(&commitments).unwrap());
    let bad = Justification {
        share: scalar(&Scalar::from(71u64)),
        ..good
    };
    assert!(!bad.holds(&commitments).unwrap());
    let noncanonical = Justification {
        share: [0xff; 32],
        ..good
    };
    assert!(noncanonical.holds(&commitments).is_err());
}

#[test]
fn ecdh_partials_verify_after_the_wire() {
    let mut rng = rng();
    let shares = {
        let parameters = dkg::Parameters {
            session: [7; 32],
            threshold: 2,
            participants: vec![1, 2, 3],
        };
        let (secrets, round1): (Vec<_>, Vec<_>) = (1..=3)
            .map(|i| dkg::round1(&parameters, i, &mut rng).unwrap())
            .unzip();
        let round2: Vec<_> = secrets
            .iter()
            .flat_map(|s| dkg::round2(&parameters, s, &round1).unwrap())
            .collect();
        secrets
            .into_iter()
            .map(|s| dkg::finalize(&parameters, s, &round1, &round2).unwrap())
            .collect::<Vec<_>>()
    };
    let context = AddressContext::new(
        tecpan_crypto::Base::from(5u64),
        shares[0].group_public_key(),
    )
    .unwrap();
    let ecdh_context = EcdhContext {
        vault: context.address(),
        epoch: 3,
    };
    let inputs = [point(77), point(78)];

    let responses: Vec<EcdhResponse> = shares
        .iter()
        .map(|share| {
            let member = AddressSecretShare::new(share.index(), share.share(), &context).unwrap();
            EcdhResponse::new(
                [1; 16],
                share.index(),
                &member.partials(&inputs, &ecdh_context, &mut rng).unwrap(),
            )
        })
        .collect();
    let decoded: Vec<Vec<_>> = responses.iter().map(|r| r.partials().unwrap()).collect();
    let publics = shares[0].verification_shares().unwrap();
    let member_public = |index: u32| {
        publics
            .get(usize::try_from(index).ok()? - 1)
            .map(|y| context.address_verification_share(y).unwrap())
    };
    for (position, input) in inputs.iter().enumerate() {
        let combine = |members: [usize; 2]| {
            let partials: Vec<_> = members.iter().map(|&m| decoded[m][position]).collect();
            ecdh::verify_and_combine(&partials, input, member_public, 2, &ecdh_context)
                .unwrap()
                .0
        };
        // Any two members reach the same `address_secret · input`.
        assert_eq!(combine([0, 1]), combine([1, 2]));
    }

    let mut tampered = responses[0].clone();
    tampered.partials[0].point = wire_point(5);
    let partial = tampered.partials().unwrap()[0];
    assert!(
        ecdh::verify_partial(
            &partial,
            &inputs[0],
            &member_public(1).unwrap(),
            &ecdh_context
        )
        .is_err()
    );
}
