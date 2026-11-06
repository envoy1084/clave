use crate::{
    constants::{DOM_SEP__SIGNER_SET, MAX_SIGNERS, POINT_LEN},
    point::{assert_ordered_points, Point},
};
use aztec::protocol::{hash::poseidon2_hash_with_separator, traits::{Deserialize, Serialize}};

/// Ordered signer keys. Slots `[0, count)` hold distinct on-curve keys; the rest are empty.
///
/// The order is part of the commitment: approvals are matched to signers by slot, and member `i` is both signer `i`
/// and viewing-committee member `i`.
#[derive(Deserialize, Eq, Serialize)]
pub struct SignerSet {
    pub keys: [Point; MAX_SIGNERS],
    pub count: u32,
}

impl SignerSet {
    /// Builds a signer set and asserts it is well formed.
    pub fn new(keys: [Point; MAX_SIGNERS], count: u32) -> Self {
        let set = Self { keys, count };
        set.assert_valid();
        set
    }

    /// Asserts `1 <= count <= MAX_SIGNERS` and that the keys are ordered, distinct, on-curve and zero-padded.
    pub fn assert_valid(self) {
        assert((self.count >= 1) & (self.count <= MAX_SIGNERS), "signer count out of range");
        assert_ordered_points(self.keys, self.count, "signer");
    }

    /// `Poseidon2("tecpan.signers.v0", x_0, y_0, ..., x_{MAX-1}, y_{MAX-1})`.
    pub fn commitment(self) -> Field {
        let mut fields = [0; MAX_SIGNERS * POINT_LEN];
        for i in 0..MAX_SIGNERS {
            fields[i * POINT_LEN] = self.keys[i].x;
            fields[i * POINT_LEN + 1] = self.keys[i].y;
        }
        poseidon2_hash_with_separator(fields, DOM_SEP__SIGNER_SET)
    }
}

pub mod test_utils {
    use crate::{constants::MAX_SIGNERS, point::Point};
    use std::embedded_curve_ops::{EmbeddedCurveScalar, fixed_base_scalar_mul};

    /// Deterministic on-curve point `scalar · G`.
    pub fn point_from_scalar(scalar: Field) -> Point {
        let p = fixed_base_scalar_mul(EmbeddedCurveScalar::from_field(scalar));
        Point { x: p.x, y: p.y }
    }

    /// `count` distinct points derived from `seed`, zero-padded.
    pub fn points(count: u32, seed: Field) -> [Point; MAX_SIGNERS] {
        let mut points = [Point::empty(); MAX_SIGNERS];
        for i in 0..MAX_SIGNERS {
            if i < count {
                points[i] = point_from_scalar(seed + i as Field);
            }
        }
        points
    }
}

mod test {
    use crate::{constants::MAX_SIGNERS, point::Point};
    use super::{SignerSet, test_utils::{point_from_scalar, points}};

    fn set_of(count: u32) -> SignerSet {
        SignerSet { keys: points(count, 1), count }
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
        set.keys[1] = Point::empty();
        set.assert_valid();
    }

    #[test(should_fail_with = "signer key not on curve")]
    fn rejects_off_curve_key() {
        let mut set = set_of(3);
        set.keys[2] = Point { x: 1, y: 1 };
        set.assert_valid();
    }

    #[test(should_fail_with = "duplicate signer")]
    fn rejects_duplicate_key() {
        let mut set = set_of(3);
        set.keys[2] = set.keys[0];
        set.assert_valid();
    }

    #[test(should_fail_with = "non-empty signer slot past count")]
    fn rejects_key_past_count() {
        let mut set = set_of(2);
        set.keys[MAX_SIGNERS - 1] = point_from_scalar(42);
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
