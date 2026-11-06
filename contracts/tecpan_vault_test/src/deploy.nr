use crate::utils::{COMMITTEE_SEED, create_signers, deploy_vault, setup};
use aztec::test::helpers::test_environment::TestEnvironment;
use tecpan_vault_contract::{
    committee::Committee,
    constants::MAX_SIGNERS,
    point::Point,
    signer_set::{SignerSet, test_utils::points},
    TecpanVault,
};

#[test]
unconstrained fn stores_config_and_signer_set() {
    let fixture = setup(3, 2, 2);
    let vault = TecpanVault::at(fixture.vault);

    let config = fixture.view(vault.get_config());
    let expected_signers = SignerSet { keys: fixture.keys, count: 3 };
    let expected_committee =
        Committee { viewing_threshold: 2, shares: fixture.committee_shares, member_count: 3 };
    assert_eq(config.threshold, 2);
    assert_eq(config.signer_count, 3);
    assert_eq(config.version, 0);
    assert_eq(config.viewing_threshold, 2);
    assert_eq(config.signer_set_commitment, expected_signers.commitment());
    assert_eq(config.committee_commitment, expected_committee.commitment());

    assert_eq(fixture.view(vault.get_signer_set()), expected_signers);
}

#[test]
unconstrained fn deploys_with_max_members() {
    let fixture = setup(MAX_SIGNERS, MAX_SIGNERS, MAX_SIGNERS);
    let config = fixture.view(TecpanVault::at(fixture.vault).get_config());
    assert_eq(config.signer_count, MAX_SIGNERS);
    assert_eq(config.threshold, MAX_SIGNERS);
    assert_eq(config.viewing_threshold, MAX_SIGNERS);
}

#[test(should_fail_with = "threshold out of range")]
unconstrained fn rejects_zero_threshold() {
    let _ = setup(3, 0, 2);
}

#[test(should_fail_with = "threshold out of range")]
unconstrained fn rejects_threshold_above_signer_count() {
    let _ = setup(3, 4, 2);
}

#[test(should_fail_with = "signer count out of range")]
unconstrained fn rejects_zero_signers() {
    let _ = setup(0, 0, 2);
}

#[test(should_fail_with = "signer count out of range")]
unconstrained fn rejects_signer_count_above_max() {
    let mut env = TestEnvironment::new();
    let (_accounts, keys) = create_signers(&mut env, MAX_SIGNERS);
    let _ = deploy_vault(
        &mut env,
        keys,
        MAX_SIGNERS + 1,
        1,
        2,
        points(MAX_SIGNERS, COMMITTEE_SEED),
    );
}

#[test(should_fail_with = "duplicate signer")]
unconstrained fn rejects_duplicate_signer() {
    let mut env = TestEnvironment::new();
    let (_accounts, mut keys) = create_signers(&mut env, 3);
    keys[2] = keys[0];
    let _ = deploy_vault(&mut env, keys, 3, 2, 2, points(3, COMMITTEE_SEED));
}

#[test(should_fail_with = "signer key not on curve")]
unconstrained fn rejects_off_curve_signer() {
    let mut env = TestEnvironment::new();
    let (_accounts, mut keys) = create_signers(&mut env, 3);
    keys[1] = Point { x: 1, y: 1 };
    let _ = deploy_vault(&mut env, keys, 3, 2, 2, points(3, COMMITTEE_SEED));
}

#[test(should_fail_with = "non-empty signer slot past count")]
unconstrained fn rejects_key_past_signer_count() {
    let mut env = TestEnvironment::new();
    let (_accounts, keys) = create_signers(&mut env, 3);
    let _ = deploy_vault(&mut env, keys, 2, 2, 2, points(2, COMMITTEE_SEED));
}

#[test(should_fail_with = "viewing threshold out of range")]
unconstrained fn rejects_viewing_threshold_one() {
    let _ = setup(3, 2, 1);
}

#[test(should_fail_with = "viewing threshold out of range")]
unconstrained fn rejects_viewing_threshold_above_members() {
    let _ = setup(3, 2, 4);
}

#[test(should_fail_with = "viewing threshold out of range")]
unconstrained fn rejects_single_member_vault() {
    let _ = setup(1, 1, 1);
}

#[test(should_fail_with = "committee key not on curve")]
unconstrained fn rejects_off_curve_committee_share() {
    let mut env = TestEnvironment::new();
    let (_accounts, keys) = create_signers(&mut env, 3);
    let mut shares = points(3, COMMITTEE_SEED);
    shares[0] = Point { x: 5, y: 5 };
    let _ = deploy_vault(&mut env, keys, 3, 2, 2, shares);
}

#[test(should_fail_with = "non-empty committee slot past count")]
unconstrained fn rejects_committee_share_past_member_count() {
    let mut env = TestEnvironment::new();
    let (_accounts, keys) = create_signers(&mut env, 3);
    let _ = deploy_vault(&mut env, keys, 3, 2, 2, points(4, COMMITTEE_SEED));
}
