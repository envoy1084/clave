//! Cross-language vectors: the same inputs and expected values are asserted in
//! `contracts/vault_contract/src/wire_vectors.nr`.

use clave_crypto::{Base, Point, Scalar, vault};

/// `count` points `(seed + i) · G`, matching the Noir `points(count, seed)` test helper.
fn points(count: u64, seed: u64) -> Vec<Point> {
    (0..count)
        .map(|i| {
            Point::try_from(Point::generator().mul_public(&Scalar::from(seed + i)))
                .expect("non-identity")
        })
        .collect()
}

#[test]
fn signer_set_commitment_matches_noir() {
    let commitment = vault::signer_set_commitment(&points(3, 1)).unwrap();
    assert_eq!(
        vault::field_hex(&commitment),
        "0x2da3848d05fa8a416557d6cbde36dff16699731c6115d91f412b5356bd7e0418"
    );
}

#[test]
fn committee_commitment_matches_noir() {
    let commitment = vault::committee_commitment(2, &points(3, 1000)).unwrap();
    assert_eq!(
        vault::field_hex(&commitment),
        "0x1322cdffa6bbec966aaa4b630b180459c06d4f2f0f59d507f4ddc9f2fce3d80b"
    );
}

#[test]
fn approval_message_matches_noir() {
    let message = vault::approval_message(
        Base::from(0x1234u64),
        Base::from(0x5678u64),
        3,
        1_700_000_000,
    );
    assert_eq!(
        vault::field_hex(&message),
        "0x1e76704d76265cda2e0fdf4073e61c88e9a7526b5a2a9b0068186a02f09841e1"
    );
}

#[test]
fn domain_separators_match_the_contract_constants() {
    assert_eq!(
        vault::field_hex(&vault::domain_separator(vault::DOM_SEP_APPROVAL)),
        "0x000000000000000000000000000000636c6176652e617070726f76616c2e7630"
    );
    assert_eq!(
        vault::field_hex(&vault::domain_separator(vault::DOM_SEP_SIGNER_SET)),
        "0x00000000000000000000000000000000636c6176652e7369676e6572732e7630"
    );
    assert_eq!(
        vault::field_hex(&vault::domain_separator(vault::DOM_SEP_COMMITTEE)),
        "0x0000000000000000000000000000636c6176652e636f6d6d69747465652e7630"
    );
}

#[test]
fn witness_layout_places_keys_signatures_and_deadline() {
    let keys = points(2, 1);
    let signature = [
        Base::from(11u64),
        Base::from(12u64),
        Base::from(13u64),
        Base::from(14u64),
    ];
    let witness = vault::encode_witness(&keys, &[None, Some(signature)], 99).unwrap();
    assert_eq!(witness.len(), vault::WITNESS_LEN);
    assert_eq!(witness[0], keys[0].x());
    assert_eq!(witness[3], keys[1].y());
    assert_eq!(witness[4], Base::from(0u64));
    assert_eq!(witness[16..20], [Base::from(0u64); 4]);
    assert_eq!(witness[20..24], signature);
    assert_eq!(witness[48], Base::from(99u64));
}

#[test]
fn rejects_more_than_max_members() {
    assert!(matches!(
        vault::signer_set_commitment(&points(9, 1)),
        Err(clave_crypto::Error::TooManyMembers { max: 8, actual: 9 })
    ));
}
