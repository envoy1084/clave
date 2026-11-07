//! Byte-compatibility with Aztec 6.0.0: generator, encodings, scalar multiplication and Poseidon2.

mod common;

use clave_crypto::{Base, Point, SecretScalar, poseidon2};
use common::{assert_point, base, bytes, scalar, vectors};

#[test]
fn generator_and_encodings_match_aztec() {
    let v = vectors();
    assert_point(&Point::generator(), &v["generator"]);
    let encoded = Point::generator().to_bytes();
    assert_eq!(Point::from_bytes(&encoded).unwrap(), Point::generator());
}

#[test]
fn scalar_multiplication_matches_aztec_public_and_secret() {
    let v = vectors();
    for case in v["scalarMuls"].as_array().unwrap() {
        let k = scalar(&case["scalar"]);
        let public = Point::try_from(Point::generator().mul_public(&k)).unwrap();
        let secret = Point::generator_mul_secret(&SecretScalar::new(k)).unwrap();
        assert_point(&public, &case["result"]);
        assert_point(&secret, &case["result"]);
    }
}

#[test]
fn poseidon2_matches_aztec() {
    let v = vectors();
    let perm = &v["poseidon2"]["permutation"];
    let input: [Base; 4] = core::array::from_fn(|i| base(&perm["input"][i]));
    let output = poseidon2::permutation(input);
    for (i, lane) in output.iter().enumerate() {
        assert_eq!(*lane, base(&perm["output"][i]), "permutation lane {i}");
    }

    for case in v["poseidon2"]["hashes"].as_array().unwrap() {
        let input: Vec<Base> = case["input"].as_array().unwrap().iter().map(base).collect();
        assert_eq!(
            poseidon2::hash(&input),
            base(&case["output"]),
            "hash of {} inputs",
            input.len()
        );
    }

    for case in v["poseidon2"]["separated"].as_array().unwrap() {
        let separator = Base::from(case["separator"].as_u64().unwrap());
        let input: Vec<Base> = case["input"].as_array().unwrap().iter().map(base).collect();
        assert_eq!(
            poseidon2::hash_with_separator(separator, &input),
            base(&case["output"])
        );
    }
}

#[test]
fn rejects_invalid_encodings() {
    // x = 1, y = 1 is not on Grumpkin.
    let mut off_curve = [0u8; 64];
    off_curve[31] = 1;
    off_curve[63] = 1;
    assert_eq!(
        Point::from_bytes(&off_curve),
        Err(clave_crypto::Error::PointNotOnCurve)
    );

    // A coordinate equal to the modulus is not canonical.
    let modulus = bytes(vectors()["frModulus"].as_str().unwrap());
    let mut non_canonical = [0u8; 64];
    non_canonical[..32].copy_from_slice(&modulus);
    assert_eq!(
        Point::from_bytes(&non_canonical),
        Err(clave_crypto::Error::NonCanonicalEncoding)
    );
}
