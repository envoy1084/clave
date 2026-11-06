//! Cross-language vectors: the same inputs and expected values are asserted in
//! `contracts/vault_contract/src/wire_vectors.nr`.

use tecpan_crypto::{Base, Point, Scalar, vault};

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
        "0x1814a2b2e68b9355116626143fe5d6549a64c5618c702464c1ccc982b659fe40"
    );
}

#[test]
fn committee_commitment_matches_noir() {
    let commitment = vault::committee_commitment(2, &points(3, 1000)).unwrap();
    assert_eq!(
        vault::field_hex(&commitment),
        "0x2bafa2a18e587afb28f4f7a6b48a24e94e39ca2b1d7c9caf151a03afe750f1a8"
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
        "0x1705887c43c37444a89a18523d5dd6729415b781cc5946946c9983f5a7371251"
    );
}

#[test]
fn domain_separators_match_the_contract_constants() {
    assert_eq!(
        vault::field_hex(&vault::domain_separator(vault::DOM_SEP_APPROVAL)),
        "0x000000000000000000000000000074656370616e2e617070726f76616c2e7630"
    );
    assert_eq!(
        vault::field_hex(&vault::domain_separator(vault::DOM_SEP_SIGNER_SET)),
        "0x00000000000000000000000000000074656370616e2e7369676e6572732e7630"
    );
    assert_eq!(
        vault::field_hex(&vault::domain_separator(vault::DOM_SEP_COMMITTEE)),
        "0x0000000000000000000000000074656370616e2e636f6d6d69747465652e7630"
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
        Err(tecpan_crypto::Error::TooManyMembers { max: 8, actual: 9 })
    ));
}
