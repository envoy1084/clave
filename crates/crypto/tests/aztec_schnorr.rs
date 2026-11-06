//! Byte-compatibility with Aztec's Schnorr: signatures produced by Aztec 6.0.0 verify, tampered ones do not, limbs
//! round-trip.

mod common;

use common::{base, bytes, point, vectors};
use tecpan_crypto::schnorr::{self, Signature};

#[test]
fn aztec_signatures_verify_and_limbs_round_trip() {
    let v = vectors();
    for case in v["schnorr"].as_array().unwrap() {
        let public_key = point(&case["publicKey"]);
        let encoded: [u8; 64] = bytes(case["signature"].as_str().unwrap())
            .try_into()
            .unwrap();
        let signature = Signature::from_bytes(&encoded).unwrap();
        assert_eq!(signature.to_bytes(), encoded);

        let limbs: [tecpan_crypto::Base; 4] = core::array::from_fn(|i| base(&case["limbs"][i]));
        assert_eq!(signature.to_limbs(), limbs);
        assert_eq!(Signature::from_limbs(&limbs).unwrap(), signature);

        assert_eq!(
            schnorr::verify(&public_key, base(&case["message"]), &signature),
            case["verifies"].as_bool().unwrap()
        );
        assert_eq!(
            schnorr::verify(&public_key, base(&case["tamperedMessage"]), &signature),
            case["tamperedVerifies"].as_bool().unwrap()
        );
    }
}

#[test]
fn rejects_wrong_key_zero_components_and_wide_limbs() {
    let v = vectors();
    let case = &v["schnorr"][0];
    let other_key = point(&v["schnorr"][1]["publicKey"]);
    let encoded: [u8; 64] = bytes(case["signature"].as_str().unwrap())
        .try_into()
        .unwrap();
    let signature = Signature::from_bytes(&encoded).unwrap();
    let message = base(&case["message"]);

    assert!(!schnorr::verify(&other_key, message, &signature));
    assert!(!schnorr::verify(
        &point(&case["publicKey"]),
        message,
        &Signature {
            e: tecpan_crypto::Base::from(0u64),
            ..signature
        }
    ));

    let mut limbs = signature.to_limbs();
    limbs[1] += tecpan_crypto::Base::from(1u128 << 127) * tecpan_crypto::Base::from(2u64);
    assert!(Signature::from_limbs(&limbs).is_err());
}

#[cfg(feature = "test-utils")]
#[test]
fn test_signer_produces_aztec_compatible_signatures() {
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;
    use tecpan_crypto::{Point, shamir::random_scalar};

    let mut rng = ChaCha20Rng::seed_from_u64(41);
    let key = random_scalar(&mut rng);
    let public_key = Point::generator_mul_secret(&key).unwrap();
    let message = tecpan_crypto::Base::from(99u64);
    let signature = schnorr::sign(&key, message, &mut rng).unwrap();
    assert!(schnorr::verify(&public_key, message, &signature));
}
