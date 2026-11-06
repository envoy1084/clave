use aztec::test::helpers::test_environment::TestEnvironment;
use tecpan_vault_contract::TecpanVault;

#[test]
unconstrained fn deploys() {
    let mut env = TestEnvironment::new();
    let deployer = env.create_light_account();

    let _vault = env.deploy("@tecpan_vault_contract/TecpanVault").with_private_initializer(
        deployer,
        TecpanVault::interface().constructor(),
    );
}
