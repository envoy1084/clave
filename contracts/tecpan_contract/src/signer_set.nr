use crate::{constants::{DOM_SEP__SIGNER_SET, MAX_SIGNERS, SIGNER_KEY_LEN}, signer_key::SignerKey};
use aztec::protocol::{hash::poseidon2_hash_with_separator, traits::{Deserialize, Serialize}};

/// Ordered list of signer keys. Slots `[0, count)` hold distinct on-curve keys; the rest are empty.
///
/// The order is part of the commitment: approvals are matched to signers by slot.
#[derive(Deserialize, Eq, Serialize)]
pub struct SignerSet {
    pub keys: [SignerKey; MAX_SIGNERS],
    pub count: u32,
}

impl SignerSet {
    /// Builds a signer set and asserts it is well formed.
    pub fn new(keys: [SignerKey; MAX_SIGNERS], count: u32) -> Self {
        let set = Self { keys, count };
        set.assert_valid();
        set
    }

    /// Asserts the set is well formed: `1 <= count <= MAX_SIGNERS`, occupied slots hold distinct on-curve keys and
    /// every slot from `count` onwards is empty.
    pub fn assert_valid(self) {
        assert((self.count >= 1) & (self.count <= MAX_SIGNERS), "signer count out of range");

        for i in 0..MAX_SIGNERS {
            let key = self.keys[i];
            if i < self.count {
                assert(!key.is_empty(), "empty signer key");
                assert(key.is_on_curve(), "signer key not on curve");
                for j in 0..i {
                    assert(self.keys[j] != key, "duplicate signer");
                }
            } else {
                assert(key.is_empty(), "non-empty slot past signer count");
            }
        }
    }

    /// Commitment to the ordered keys, stored in the config note.
    pub fn commitment(self) -> Field {
        poseidon2_hash_with_separator(self.flat_keys(), DOM_SEP__SIGNER_SET)
    }

    fn flat_keys(self) -> [Field; MAX_SIGNERS * SIGNER_KEY_LEN] {
        let mut fields = [0; MAX_SIGNERS * SIGNER_KEY_LEN];
        for i in 0..MAX_SIGNERS {
            fields[i * SIGNER_KEY_LEN] = self.keys[i].x;
            fields[i * SIGNER_KEY_LEN + 1] = self.keys[i].y;
        }
        fields
    }
}

mod test {
    use crate::{constants::MAX_SIGNERS, signer_key::SignerKey};
    use super::SignerSet;
    use std::embedded_curve_ops::{EmbeddedCurvePoint, EmbeddedCurveScalar, fixed_base_scalar_mul};

    fn key(scalar: Field) -> SignerKey {
        let point: EmbeddedCurvePoint =
            fixed_base_scalar_mul(EmbeddedCurveScalar::from_field(scalar));
        SignerKey { x: point.x, y: point.y }
    }

    fn set_of(count: u32) -> SignerSet {
        let mut keys = [SignerKey::empty(); MAX_SIGNERS];
        for i in 0..MAX_SIGNERS {
            if i < count {
                keys[i] = key((i + 1) as Field);
            }
        }
        SignerSet { keys, count }
    }

    #[test]
    fn accepts_one_to_max_signers() {
        set_of(1).assert_valid();
        set_of(MAX_SIGNERS).assert_valid();
    }

    #[test(should_fail_with = "signer count out of range")]
    fn rejects_zero_signers() {
        set_of(0).assert_valid();
    }

    #[test(should_fail_with = "signer count out of range")]
    fn rejects_count_above_max() {
        let mut set = set_of(MAX_SIGNERS);
        set.count = MAX_SIGNERS + 1;
        set.assert_valid();
    }

    #[test(should_fail_with = "empty signer key")]
    fn rejects_empty_key_inside_count() {
        let mut set = set_of(3);
        set.keys[1] = SignerKey::empty();
        set.assert_valid();
    }

    #[test(should_fail_with = "signer key not on curve")]
    fn rejects_off_curve_key() {
        let mut set = set_of(3);
        set.keys[2] = SignerKey { x: 1, y: 1 };
        set.assert_valid();
    }

    #[test(should_fail_with = "duplicate signer")]
    fn rejects_duplicate_key() {
        let mut set = set_of(3);
        set.keys[2] = set.keys[0];
        set.assert_valid();
    }

    #[test(should_fail_with = "non-empty slot past signer count")]
    fn rejects_key_past_count() {
        let mut set = set_of(2);
        set.keys[MAX_SIGNERS - 1] = key(42);
        set.assert_valid();
    }

    #[test]
    fn commitment_depends_on_order() {
        let set = set_of(2);
        let mut swapped = set;
        swapped.keys[0] = set.keys[1];
        swapped.keys[1] = set.keys[0];
        assert(set.commitment() != swapped.commitment());
    }
}
