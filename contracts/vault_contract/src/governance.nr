//! Config transitions. Each returns the next `ConfigNote` and asserts the invariants: `1 <= k <= n`,
//! `2 <= t_v <= n`, the version increases by one, and a rotation removes fewer than the current `t_v` members.

use crate::{
    committee::Committee, constants::MAX_SIGNERS, notes::config_note::ConfigNote, point::Point,
    signer_set::SignerSet,
};

/// Same policy, next version: invalidates every approval collected so far.
pub fn bump_version(config: ConfigNote) -> ConfigNote {
    ConfigNote {
        threshold: config.threshold,
        signer_count: config.signer_count,
        version: config.version + 1,
        signer_set_commitment: config.signer_set_commitment,
        viewing_threshold: config.viewing_threshold,
        committee_commitment: config.committee_commitment,
    }
}

/// New spending threshold `k`; the signer set and committee are unchanged.
pub fn with_threshold(config: ConfigNote, threshold: u32) -> ConfigNote {
    assert_threshold(threshold, config.signer_count);
    ConfigNote {
        threshold,
        signer_count: config.signer_count,
        version: config.version + 1,
        signer_set_commitment: config.signer_set_commitment,
        viewing_threshold: config.viewing_threshold,
        committee_commitment: config.committee_commitment,
    }
}

/// New signer set, spending threshold and viewing committee, as one atomic change.
///
/// `current_keys` must be the stored signer set (checked against its commitment). At most `t_v - 1` current members
/// may be absent from the new set, so the viewing reshare that accompanies the rotation keeps the viewing secret safe.
pub fn rotate(
    config: ConfigNote,
    current_keys: [Point; MAX_SIGNERS],
    new_keys: [Point; MAX_SIGNERS],
    new_count: u32,
    threshold: u32,
    viewing_threshold: u32,
    committee_shares: [Point; MAX_SIGNERS],
) -> ConfigNote {
    let current = SignerSet { keys: current_keys, count: config.signer_count };
    assert(current.commitment() == config.signer_set_commitment, "current signer set mismatch");

    let next = SignerSet::new(new_keys, new_count);
    assert_threshold(threshold, new_count);
    let committee = Committee::new(viewing_threshold, committee_shares, new_count);
    assert(removed_members(current, next) < config.viewing_threshold, "too many members removed");

    ConfigNote {
        threshold,
        signer_count: new_count,
        version: config.version + 1,
        signer_set_commitment: next.commitment(),
        viewing_threshold,
        committee_commitment: committee.commitment(),
    }
}

/// Number of members of `current` whose key does not appear in `next`.
pub fn removed_members(current: SignerSet, next: SignerSet) -> u32 {
    let mut removed: u32 = 0;
    for i in 0..MAX_SIGNERS {
        if i < current.count {
            let mut kept = false;
            for j in 0..MAX_SIGNERS {
                kept |= (j < next.count) & (next.keys[j] == current.keys[i]);
            }
            removed += (!kept) as u32;
        }
    }
    removed
}

/// Asserts the spending threshold invariant `1 <= k <= n`.
pub fn assert_threshold(threshold: u32, signer_count: u32) {
    assert((threshold >= 1) & (threshold <= signer_count), "threshold out of range");
}

mod test {
    use crate::{
        committee::Committee,
        constants::MAX_SIGNERS,
        notes::config_note::ConfigNote,
        point::Point,
        signer_set::{SignerSet, test_utils::{point_from_scalar, points}},
    };
    use super::{removed_members, rotate, with_threshold};

    global SHARE_SEED: Field = 1000;

    /// A 3-member vault: k = 2, t_v = 2, version 5.
    fn config() -> ConfigNote {
        ConfigNote {
            threshold: 2,
            signer_count: 3,
            version: 5,
            signer_set_commitment: SignerSet { keys: points(3, 1), count: 3 }.commitment(),
            viewing_threshold: 2,
            committee_commitment: Committee {
                viewing_threshold: 2,
                shares: points(3, SHARE_SEED),
                member_count: 3,
            }
                .commitment(),
        }
    }

    fn rotate_to(keys: [Point; MAX_SIGNERS], count: u32, k: u32, t_v: u32) -> ConfigNote {
        rotate(
            config(),
            points(3, 1),
            keys,
            count,
            k,
            t_v,
            points(count, SHARE_SEED + 50),
        )
    }

    #[test]
    fn threshold_change_bumps_version_and_keeps_sets() {
        let next = with_threshold(config(), 3);
        assert_eq(next.threshold, 3);
        assert_eq(next.version, 6);
        assert_eq(next.signer_set_commitment, config().signer_set_commitment);
        assert_eq(next.committee_commitment, config().committee_commitment);
    }

    #[test(should_fail_with = "threshold out of range")]
    fn threshold_change_rejects_zero() {
        let _ = with_threshold(config(), 0);
    }

    #[test(should_fail_with = "threshold out of range")]
    fn threshold_change_rejects_above_signer_count() {
        let _ = with_threshold(config(), 4);
    }

    #[test]
    fn rotation_adds_a_member() {
        let next = rotate_to(points(4, 1), 4, 3, 3);
        assert_eq(next.signer_count, 4);
        assert_eq(next.threshold, 3);
        assert_eq(next.viewing_threshold, 3);
        assert_eq(next.version, 6);
        assert_eq(
            next.signer_set_commitment,
            SignerSet { keys: points(4, 1), count: 4 }.commitment(),
        );
    }

    #[test]
    fn rotation_removes_one_member_when_t_v_is_two() {
        let next = rotate_to(points(2, 1), 2, 2, 2);
        assert_eq(next.signer_count, 2);
    }

    #[test]
    fn rotation_replaces_a_key() {
        let mut keys = points(3, 1);
        keys[1] = point_from_scalar(77);
        assert_eq(rotate_to(keys, 3, 2, 2).signer_count, 3);
    }

    #[test(should_fail_with = "too many members removed")]
    fn rotation_rejects_removing_t_v_members() {
        // Keep member 0, drop members 1 and 2, add one newcomer: two removals with t_v = 2.
        let mut keys = points(2, 1);
        keys[1] = point_from_scalar(90);
        let _ = rotate_to(keys, 2, 2, 2);
    }

    #[test(should_fail_with = "too many members removed")]
    fn rotation_rejects_replacing_t_v_keys() {
        let mut keys = points(3, 1);
        keys[0] = point_from_scalar(70);
        keys[1] = point_from_scalar(71);
        let _ = rotate_to(keys, 3, 2, 2);
    }

    #[test(should_fail_with = "current signer set mismatch")]
    fn rotation_rejects_stale_current_set() {
        let _ = rotate(
            config(),
            points(3, 2),
            points(3, 1),
            3,
            2,
            2,
            points(3, SHARE_SEED),
        );
    }

    #[test(should_fail_with = "threshold out of range")]
    fn rotation_rejects_threshold_above_new_count() {
        let _ = rotate_to(points(3, 1), 3, 4, 2);
    }

    #[test(should_fail_with = "viewing threshold out of range")]
    fn rotation_rejects_viewing_threshold_one() {
        let _ = rotate_to(points(3, 1), 3, 2, 1);
    }

    #[test(should_fail_with = "duplicate signer")]
    fn rotation_rejects_invalid_new_set() {
        let mut keys = points(3, 1);
        keys[2] = keys[0];
        let _ = rotate_to(keys, 3, 2, 2);
    }

    #[test]
    fn removed_members_ignores_order() {
        let current = SignerSet { keys: points(3, 1), count: 3 };
        let mut reordered = current;
        reordered.keys[0] = current.keys[2];
        reordered.keys[2] = current.keys[0];
        assert_eq(removed_members(current, reordered), 0);
        assert_eq(removed_members(current, SignerSet { keys: points(1, 1), count: 1 }), 2);
    }
}
