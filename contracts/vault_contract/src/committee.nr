use crate::{
    constants::{DOM_SEP__COMMITTEE, MAX_SIGNERS, MIN_VIEWING_THRESHOLD, POINT_LEN},
    point::{assert_ordered_points, Point},
};
use aztec::protocol::hash::poseidon2_hash_with_separator;

/// Viewing committee: the threshold `t_v` and each member's public verification share `Y_i = s_i · G` from the
/// distributed key generation. Member `i` of the committee is signer `i` of the vault.
///
/// Only the commitment is stored on-chain; member agents check every threshold ECDH request and reshare against it.
pub struct Committee {
    pub viewing_threshold: u32,
    pub shares: [Point; MAX_SIGNERS],
    pub member_count: u32,
}

impl Committee {
    /// Builds a committee for `member_count` members and asserts it is well formed.
    pub fn new(viewing_threshold: u32, shares: [Point; MAX_SIGNERS], member_count: u32) -> Self {
        let committee = Self { viewing_threshold, shares, member_count };
        committee.assert_valid();
        committee
    }

    /// Asserts `MIN_VIEWING_THRESHOLD <= t_v <= member_count` and that shares are ordered, distinct, on-curve and
    /// zero-padded.
    pub fn assert_valid(self) {
        assert(
            (self.viewing_threshold >= MIN_VIEWING_THRESHOLD)
                & (self.viewing_threshold <= self.member_count),
            "viewing threshold out of range",
        );
        assert_ordered_points(self.shares, self.member_count, "committee");
    }

    /// `Poseidon2("tecpan.committee.v0", t_v, Y_0.x, Y_0.y, ..., Y_{MAX-1}.y)`.
    pub fn commitment(self) -> Field {
        let mut fields = [0; MAX_SIGNERS * POINT_LEN + 1];
        fields[0] = self.viewing_threshold as Field;
        for i in 0..MAX_SIGNERS {
            fields[1 + i * POINT_LEN] = self.shares[i].x;
            fields[1 + i * POINT_LEN + 1] = self.shares[i].y;
        }
        poseidon2_hash_with_separator(fields, DOM_SEP__COMMITTEE)
    }
}

mod test {
    use crate::{constants::MAX_SIGNERS, point::Point, signer_set::test_utils::points};
    use super::Committee;

    fn committee(t_v: u32, count: u32) -> Committee {
        Committee { viewing_threshold: t_v, shares: points(count, 1000), member_count: count }
    }

    #[test]
    fn accepts_threshold_two_to_member_count() {
        committee(2, 2).assert_valid();
        committee(2, MAX_SIGNERS).assert_valid();
        committee(MAX_SIGNERS, MAX_SIGNERS).assert_valid();
    }

    #[test(should_fail_with = "viewing threshold out of range")]
    fn rejects_threshold_one() {
        committee(1, 3).assert_valid();
    }

    #[test(should_fail_with = "viewing threshold out of range")]
    fn rejects_threshold_above_members() {
        committee(4, 3).assert_valid();
    }

    #[test(should_fail_with = "committee key not on curve")]
    fn rejects_off_curve_share() {
        let mut c = committee(2, 3);
        c.shares[1] = Point { x: 3, y: 3 };
        c.assert_valid();
    }

    #[test(should_fail_with = "duplicate committee")]
    fn rejects_duplicate_share() {
        let mut c = committee(2, 3);
        c.shares[2] = c.shares[0];
        c.assert_valid();
    }

    #[test(should_fail_with = "non-empty committee slot past count")]
    fn rejects_share_past_member_count() {
        let mut c = committee(2, 3);
        c.member_count = 2;
        c.assert_valid();
    }

    #[test]
    fn commitment_binds_threshold_and_shares() {
        let base = committee(2, 3);
        let mut other_threshold = base;
        other_threshold.viewing_threshold = 3;
        let mut other_share = base;
        other_share.shares[0] = base.shares[1];
        other_share.shares[1] = base.shares[0];

        assert(base.commitment() != other_threshold.commitment());
        assert(base.commitment() != other_share.commitment());
    }
}
