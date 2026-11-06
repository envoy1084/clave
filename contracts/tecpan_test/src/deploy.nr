use crate::utils::{create_signers, deploy_multisig, setup};
use aztec::test::helpers::test_environment::TestEnvironment;
use tecpan_contract::{
    constants::MAX_SIGNERS, signer_key::SignerKey, signer_set::SignerSet, TecpanMultisig,
};

#[test]
unconstrained fn stores_config_and_signer_set() {
    let fixture = setup(3, 2);
    let multisig = TecpanMultisig::at(fixture.multisig);

    let config = fixture.view(multisig.get_config());
    let expected_set = SignerSet { keys: fixture.keys, count: 3 };
    assert_eq(config.threshold, 2);
    assert_eq(config.signer_count, 3);
    assert_eq(config.version, 0);
    assert_eq(config.signer_set_commitment, expected_set.commitment());

    let signer_set = fixture.view(multisig.get_signer_set());
    assert_eq(signer_set, expected_set);
}

#[test]
unconstrained fn deploys_with_max_signers() {
    let fixture = setup(MAX_SIGNERS, MAX_SIGNERS);

    let config = fixture.view(TecpanMultisig::at(fixture.multisig).get_config());
    assert_eq(config.signer_count, MAX_SIGNERS);
    assert_eq(config.threshold, MAX_SIGNERS);
}

#[test(should_fail_with = "threshold out of range")]
unconstrained fn rejects_zero_threshold() {
    let _ = setup(3, 0);
}

#[test(should_fail_with = "threshold out of range")]
unconstrained fn rejects_threshold_above_signer_count() {
    let _ = setup(3, 4);
}

#[test(should_fail_with = "signer count out of range")]
unconstrained fn rejects_zero_signers() {
    let _ = setup(0, 0);
}

#[test(should_fail_with = "signer count out of range")]
unconstrained fn rejects_signer_count_above_max() {
    let mut env = TestEnvironment::new();
    let deployer = env.create_light_account();
    let (_accounts, keys) = create_signers(&mut env, MAX_SIGNERS);
    let _ = deploy_multisig(&mut env, deployer, keys, MAX_SIGNERS + 1, 1);
}

#[test(should_fail_with = "duplicate signer")]
unconstrained fn rejects_duplicate_signer() {
    let mut env = TestEnvironment::new();
    let deployer = env.create_light_account();
    let (_accounts, mut keys) = create_signers(&mut env, 3);
    keys[2] = keys[0];
    let _ = deploy_multisig(&mut env, deployer, keys, 3, 2);
}

#[test(should_fail_with = "signer key not on curve")]
unconstrained fn rejects_off_curve_signer() {
    let mut env = TestEnvironment::new();
    let deployer = env.create_light_account();
    let (_accounts, mut keys) = create_signers(&mut env, 3);
    keys[1] = SignerKey { x: 1, y: 1 };
    let _ = deploy_multisig(&mut env, deployer, keys, 3, 2);
}

#[test(should_fail_with = "non-empty slot past signer count")]
unconstrained fn rejects_key_past_signer_count() {
    let mut env = TestEnvironment::new();
    let deployer = env.create_light_account();
    let (_accounts, keys) = create_signers(&mut env, 3);
    let _ = deploy_multisig(&mut env, deployer, keys, 2, 2);
}
