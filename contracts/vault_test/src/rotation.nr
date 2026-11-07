use crate::utils::{create_signers, first, OUTER_HASH, setup};
use aztec::protocol::address::AztecAddress;
use vault_contract::{
    ClaveVault,
    committee::Committee,
    constants::MAX_SIGNERS,
    point::Point,
    signer_set::{SignerSet, test_utils::points},
};

#[test]
unconstrained fn adds_a_member() {
    let mut fixture = setup(3, 2, 2);
    let (newcomer, newcomer_key) = create_signers(&mut fixture.env, 1);
    let mut accounts = fixture.accounts;
    let mut keys = fixture.keys;
    accounts[3] = newcomer[0];
    keys[3] = newcomer_key[0];

    let rotated = fixture.rotate(accounts, keys, 4, 3, 3);

    let config = rotated.view(ClaveVault::at(rotated.vault).get_config());
    assert_eq(config.version, 1);
    assert_eq(config.signer_count, 4);
    assert_eq(config.threshold, 3);
    assert_eq(config.viewing_threshold, 3);
    assert_eq(config.signer_set_commitment, SignerSet { keys, count: 4 }.commitment());
    assert_eq(
        config.committee_commitment,
        Committee { viewing_threshold: 3, shares: rotated.committee_shares, member_count: 4 }
            .commitment(),
    );
    assert_eq(
        rotated.view(ClaveVault::at(rotated.vault).get_signer_set()),
        SignerSet { keys, count: 4 },
    );

    // The newcomer's approval counts towards the new threshold.
    let mut approving = [false; MAX_SIGNERS];
    approving[0] = true;
    approving[1] = true;
    approving[3] = true;
    assert(rotated.authorize(OUTER_HASH, rotated.approved(1, approving)));
}

#[test]
unconstrained fn removes_a_member() {
    let fixture = setup(3, 2, 2);
    let mut accounts = fixture.accounts;
    let mut keys = fixture.keys;
    accounts[2] = AztecAddress::zero();
    keys[2] = Point::empty();

    let rotated = fixture.rotate(accounts, keys, 2, 2, 2);

    assert_eq(
        rotated.view(ClaveVault::at(rotated.vault).get_signer_set()),
        SignerSet { keys, count: 2 },
    );
    assert(rotated.authorize(OUTER_HASH, rotated.approved(1, first(2))));
}

#[test(should_fail_with = "signer set mismatch")]
unconstrained fn removed_member_cannot_approve() {
    let fixture = setup(3, 2, 2);
    let mut accounts = fixture.accounts;
    let mut keys = fixture.keys;
    accounts[2] = AztecAddress::zero();
    keys[2] = Point::empty();
    let _ = fixture.rotate(accounts, keys, 2, 2, 2);

    // A witness that still lists the removed member no longer matches the stored signer set.
    let _ = fixture.authorize(OUTER_HASH, fixture.approved(1, first(3)));
}

#[test]
unconstrained fn replaces_a_members_key() {
    let mut fixture = setup(3, 2, 2);
    let (device, device_key) = create_signers(&mut fixture.env, 1);
    let mut accounts = fixture.accounts;
    let mut keys = fixture.keys;
    accounts[1] = device[0];
    keys[1] = device_key[0];

    let rotated = fixture.rotate(accounts, keys, 3, 2, 2);

    let mut approving = [false; MAX_SIGNERS];
    approving[1] = true;
    approving[2] = true;
    assert(rotated.authorize(OUTER_HASH, rotated.approved(1, approving)));
}

#[test]
unconstrained fn reorders_members() {
    let fixture = setup(3, 2, 2);
    let mut accounts = fixture.accounts;
    let mut keys = fixture.keys;
    accounts[0] = fixture.accounts[2];
    accounts[2] = fixture.accounts[0];
    keys[0] = fixture.keys[2];
    keys[2] = fixture.keys[0];

    let rotated = fixture.rotate(accounts, keys, 3, 2, 2);
    assert(rotated.authorize(OUTER_HASH, rotated.approved(1, first(2))));
}

#[test]
unconstrained fn rewrites_a_full_vault_within_call_limits() {
    let mut fixture = setup(MAX_SIGNERS, 5, 3);
    let (devices, device_keys) = create_signers(&mut fixture.env, 2);
    let mut accounts = fixture.accounts;
    let mut keys = fixture.keys;
    accounts[0] = devices[0];
    keys[0] = device_keys[0];
    accounts[7] = devices[1];
    keys[7] = device_keys[1];

    let rotated = fixture.rotate(accounts, keys, MAX_SIGNERS, 5, 3);
    assert_eq(
        rotated.view(ClaveVault::at(rotated.vault).get_signer_set()),
        SignerSet { keys, count: MAX_SIGNERS },
    );
    assert(rotated.authorize(OUTER_HASH, rotated.approved(1, first(5))));
}

#[test]
unconstrained fn successive_rotations_advance_the_version() {
    let fixture = setup(3, 2, 2);
    let once = fixture.rotate(fixture.accounts, fixture.keys, 3, 3, 2);
    let twice = once.rotate(once.accounts, once.keys, 3, 2, 3);

    let config = twice.view(ClaveVault::at(twice.vault).get_config());
    assert_eq(config.version, 2);
    assert_eq(config.threshold, 2);
    assert_eq(config.viewing_threshold, 3);
    assert(twice.authorize(OUTER_HASH, twice.approved(2, first(2))));
}

#[test(should_fail_with = "invalid signature")]
unconstrained fn rotation_invalidates_earlier_approvals() {
    let fixture = setup(3, 2, 2);
    let collected = fixture.approved(0, first(3));
    let _ = fixture.rotate(fixture.accounts, fixture.keys, 3, 2, 2);
    let _ = fixture.authorize(OUTER_HASH, collected);
}

#[test(should_fail_with = "too many members removed")]
unconstrained fn rejects_removing_viewing_threshold_members() {
    let fixture = setup(3, 2, 2);
    let mut accounts = [AztecAddress::zero(); MAX_SIGNERS];
    let mut keys = [Point::empty(); MAX_SIGNERS];
    accounts[0] = fixture.accounts[0];
    keys[0] = fixture.keys[0];
    accounts[1] = fixture.accounts[1];
    keys[1] = points(1, 777)[0];
    let _ = fixture.rotate(accounts, keys, 2, 2, 2);
}

#[test(should_fail_with = "current signer set mismatch")]
unconstrained fn rejects_stale_current_signer_set() {
    let fixture = setup(3, 2, 2);
    fixture.self_call(ClaveVault::at(fixture.vault).rotate_membership(
        points(3, 5),
        fixture.keys,
        3,
        2,
        2,
        points(3, 1100),
    ));
}

#[test(should_fail_with = "threshold out of range")]
unconstrained fn rejects_threshold_above_new_member_count() {
    let fixture = setup(3, 2, 2);
    let _ = fixture.rotate(fixture.accounts, fixture.keys, 3, 4, 2);
}

#[test(should_fail_with = "viewing threshold out of range")]
unconstrained fn rejects_viewing_threshold_one() {
    let fixture = setup(3, 2, 2);
    let _ = fixture.rotate(fixture.accounts, fixture.keys, 3, 2, 1);
}

#[test(should_fail_with = "duplicate signer")]
unconstrained fn rejects_duplicate_new_signer() {
    let fixture = setup(3, 2, 2);
    let mut keys = fixture.keys;
    keys[2] = keys[0];
    let _ = fixture.rotate(fixture.accounts, keys, 3, 2, 2);
}

#[test(should_fail_with = "can only be called by the same contract")]
unconstrained fn rejects_outside_caller() {
    let fixture = setup(3, 2, 2);
    fixture.env.call_private(
        fixture.accounts[0],
        ClaveVault::at(fixture.vault).rotate_membership(
            fixture.keys,
            fixture.keys,
            3,
            2,
            2,
            points(3, 1100),
        ),
    );
}
