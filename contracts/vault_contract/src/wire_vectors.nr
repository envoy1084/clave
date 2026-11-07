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
            0x2da3848d05fa8a416557d6cbde36dff16699731c6115d91f412b5356bd7e0418,
        );
    }

    #[test]
    fn committee_commitment_matches_rust() {
        let committee =
            Committee { viewing_threshold: 2, shares: points(3, 1000), member_count: 3 };
        assert_eq(
            committee.commitment(),
            0x1322cdffa6bbec966aaa4b630b180459c06d4f2f0f59d507f4ddc9f2fce3d80b,
        );
    }

    #[test]
    fn approval_message_matches_rust() {
        let message = approval_message(AztecAddress::from_field(0x1234), 0x5678, 3, 1700000000);
        assert_eq(message, 0x1e76704d76265cda2e0fdf4073e61c88e9a7526b5a2a9b0068186a02f09841e1);
    }
}
