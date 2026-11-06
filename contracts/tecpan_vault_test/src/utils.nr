use aztec::{
    context::{PrivateCall, UtilityCall},
    keys::getters::get_public_keys,
    protocol::{address::AztecAddress, traits::Deserialize},
    test::helpers::{
        test_environment::{CallPrivateOptions, ExecuteUtilityOptions, TestEnvironment},
        txe_oracles,
    },
};
use tecpan_vault_contract::{
    constants::MAX_SIGNERS, point::Point, signer_set::test_utils::points, TecpanVault,
};

/// Seed for deterministic viewing verification shares (any distinct on-curve points will do in contract tests).
pub global COMMITTEE_SEED: Field = 1000;

/// A deployed vault together with the test accounts whose keys it was created with.
pub struct Fixture {
    pub env: TestEnvironment,
    pub vault: AztecAddress,
    pub accounts: [AztecAddress; MAX_SIGNERS],
    pub keys: [Point; MAX_SIGNERS],
    pub committee_shares: [Point; MAX_SIGNERS],
    pub signer_count: u32,
    pub threshold: u32,
    pub viewing_threshold: u32,
}

/// Creates `count` signer accounts and returns their addresses and Grumpkin signing keys, zero-padded.
///
/// The TXE signs auth witnesses with an account's incoming viewing secret, so its `ivpk_m` is the signing key.
pub unconstrained fn create_signers(
    env: &mut TestEnvironment,
    count: u32,
) -> ([AztecAddress; MAX_SIGNERS], [Point; MAX_SIGNERS]) {
    let mut accounts = [AztecAddress::zero(); MAX_SIGNERS];
    let mut keys = [Point::empty(); MAX_SIGNERS];
    for i in 0..count {
        let account = env.create_light_account();
        let key = env.utility_context(|_context| get_public_keys(account).ivpk_m.inner);
        accounts[i] = account;
        keys[i] = Point { x: key.x, y: key.y };
    }
    (accounts, keys)
}

/// Deploys a vault. Like an Aztec.js account deployment, the deployer's call has the vault in scope so the
/// constructor can use the vault's own keys (`with_private_initializer` only scopes the call to `from`).
pub unconstrained fn deploy_vault(
    env: &mut TestEnvironment,
    keys: [Point; MAX_SIGNERS],
    signer_count: u32,
    threshold: u32,
    viewing_threshold: u32,
    committee_shares: [Point; MAX_SIGNERS],
) -> AztecAddress {
    let deployer = env.create_light_account();
    let deployment = env.deploy("@tecpan_vault_contract/TecpanVault");
    let constructor = TecpanVault::interface().constructor(
        keys,
        signer_count,
        threshold,
        viewing_threshold,
        committee_shares,
    );
    let vault = txe_oracles::deploy(
        deployment.path,
        constructor.name,
        constructor.args,
        deployment.secret,
        deployment.salt,
        AztecAddress::zero(),
        deployment.immutables_hash,
    )
        .to_address();

    let _ = env.call_private_opts(
        deployer,
        CallPrivateOptions::new().with_additional_scopes([vault]),
        PrivateCall::<_, _, ()>::new(
            vault,
            constructor.selector,
            constructor.name,
            constructor.args,
        ),
    );
    vault
}

/// Deploys a `threshold`-of-`signer_count` vault with viewing threshold `viewing_threshold` and fresh signer accounts.
pub unconstrained fn setup(signer_count: u32, threshold: u32, viewing_threshold: u32) -> Fixture {
    let mut env = TestEnvironment::new();
    let (accounts, keys) = create_signers(&mut env, signer_count);
    let committee_shares = points(signer_count, COMMITTEE_SEED);
    let vault = deploy_vault(
        &mut env,
        keys,
        signer_count,
        threshold,
        viewing_threshold,
        committee_shares,
    );

    let fixture = Fixture {
        env,
        vault,
        accounts,
        keys,
        committee_shares,
        signer_count,
        threshold,
        viewing_threshold,
    };
    // A PXE discovers the vault's notes before simulating a transaction. Inline private contexts skip that step, so
    // run a utility call once to sync them.
    let _ = fixture.view(TecpanVault::at(vault).get_config());
    fixture
}

impl Fixture {
    /// Runs a utility function of the vault with the vault in scope, as a member's PXE would.
    pub unconstrained fn view<let M: u32, let N: u32, T>(self, call: UtilityCall<M, N, T>) -> T
    where
        T: Deserialize,
    {
        self.env.execute_utility_opts(ExecuteUtilityOptions::new().with_from(self.vault), call)
    }
}
