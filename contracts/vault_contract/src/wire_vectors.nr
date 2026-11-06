//! Cross-language vectors: the same inputs and expected values are asserted in `crates/crypto/tests/vault_wire.rs`.
//! A change to any wire format must update both, together with `architecture/contract.md`.

mod test {
    use crate::{
        authorization::approval_message,
        committee::Committee,
        signer_set::{SignerSet, test_utils::points},
    };
    use aztec::protocol::{address::AztecAddress, traits::FromField};

    #[test]
    fn signer_set_commitment_matches_rust() {
        let set = SignerSet { keys: points(3, 1), count: 3 };
        assert_eq(
            set.commitment(),
            0x1814a2b2e68b9355116626143fe5d6549a64c5618c702464c1ccc982b659fe40,
        );
    }

    #[test]
    fn committee_commitment_matches_rust() {
        let committee =
            Committee { viewing_threshold: 2, shares: points(3, 1000), member_count: 3 };
        assert_eq(
            committee.commitment(),
            0x2bafa2a18e587afb28f4f7a6b48a24e94e39ca2b1d7c9caf151a03afe750f1a8,
        );
    }

    #[test]
    fn approval_message_matches_rust() {
        let message = approval_message(AztecAddress::from_field(0x1234), 0x5678, 3, 1700000000);
        assert_eq(message, 0x1705887c43c37444a89a18523d5dd6729415b781cc5946946c9983f5a7371251);
    }
}
