use aztec::test::helpers::test_environment::TestEnvironment;
use tecpan_contract::TecpanMultisig;

#[test]
unconstrained fn deploys() {
    let mut env = TestEnvironment::new();
    let deployer = env.create_light_account();

    let _multisig = env.deploy("@tecpan_contract/TecpanMultisig").with_private_initializer(
        deployer,
        TecpanMultisig::interface().constructor(),
    );
}
