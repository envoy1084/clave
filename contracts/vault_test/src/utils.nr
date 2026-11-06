use aztec::{
    context::{PrivateCall, UtilityCall},
    keys::getters::get_public_keys,
    protocol::{address::AztecAddress, traits::Deserialize},
    test::helpers::{
        test_environment::{CallPrivateOptions, ExecuteUtilityOptions, TestEnvironment},
        txe_oracles,
    },
};
use std::test::OracleMock;
use test_signer_contract::TestSigner;
use vault_contract::{
    authorization::{approval_message, is_authorized},
    constants::{MAX_SIGNERS, POINT_LEN, SIGNATURE_LEN, WITNESS_LEN},
    point::Point,
    signer_set::test_utils::points,
    TecpanVault,
};

/// Outer hash approved in authorization tests (any field works: the vault only signs over it).
pub global OUTER_HASH: Field = 0x7ec9a2;

pub global ONE_DAY: u64 = 86400;

/// Seed for deterministic viewing verification shares (any distinct on-curve points will do in contract tests).
pub global COMMITTEE_SEED: Field = 1000;

/// A deployed vault together with the test accounts whose keys it was created with.
pub struct Fixture {
    pub env: TestEnvironment,
    pub vault: AztecAddress,
    pub test_signer: AztecAddress,
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
    let deployment = env.deploy("@vault_contract/TecpanVault");
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
    let test_signer = env.deploy("@test_signer_contract/TestSigner").without_initializer();

    let fixture = Fixture {
        env,
        vault,
        test_signer,
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

    /// Rotates to `new_accounts` / `new_keys` (first `new_count` slots) as the vault itself, with fresh committee
    /// shares, and returns the fixture describing the new membership.
    pub unconstrained fn rotate(
        self,
        new_accounts: [AztecAddress; MAX_SIGNERS],
        new_keys: [Point; MAX_SIGNERS],
        new_count: u32,
        threshold: u32,
        viewing_threshold: u32,
    ) -> Fixture {
        let committee_shares = points(new_count, COMMITTEE_SEED + 100);
        self.self_call(TecpanVault::at(self.vault).rotate_membership(
            self.keys,
            new_keys,
            new_count,
            threshold,
            viewing_threshold,
            committee_shares,
        ));
        Fixture {
            env: self.env,
            vault: self.vault,
            test_signer: self.test_signer,
            accounts: new_accounts,
            keys: new_keys,
            committee_shares,
            signer_count: new_count,
            threshold,
            viewing_threshold,
        }
    }

    /// Calls a vault function as the vault itself (as its `entrypoint` would after approval), then syncs its notes.
    pub unconstrained fn self_call<let M: u32, let N: u32>(self, call: PrivateCall<M, N, ()>) {
        self.env.call_private(self.vault, call);
        let _ = self.view(TecpanVault::at(self.vault).get_config());
    }

    /// Runs the vault's authorization check for `outer_hash` in a private context at the vault, serving `witness` as
    /// the auth witness.
    pub unconstrained fn authorize(self, outer_hash: Field, witness: [Field; WITNESS_LEN]) -> bool {
        // One answer per check: an earlier mock would otherwise keep answering later checks in the same test.
        let _ = OracleMock::mock("aztec_utl_getAuthWitness").returns(witness).times(1);
        self.env.private_context_at(self.vault, |context| {
            let config_slot = TecpanVault::storage_layout().config.slot;
            is_authorized(context, config_slot, outer_hash)
        })
    }

    /// Witness approving `OUTER_HASH` under `config_version` by the given slots, valid for a day.
    pub unconstrained fn approved(
        self,
        config_version: u32,
        approving: [bool; MAX_SIGNERS],
    ) -> [Field; WITNESS_LEN] {
        let expires_at = self.now() + ONE_DAY;
        self.witness(
            self.approval_message(OUTER_HASH, config_version, expires_at),
            approving,
            expires_at,
        )
    }

    /// Timestamp of the latest block, which inline private contexts use as their anchor.
    pub unconstrained fn now(self) -> u64 {
        self.env.last_block_timestamp()
    }

    /// Schnorr signature by `account` over `message`.
    pub unconstrained fn sign(
        self,
        account: AztecAddress,
        message: Field,
    ) -> [Field; SIGNATURE_LEN] {
        txe_oracles::add_authwit(account, message);
        self.env.call_private(account, TestSigner::at(self.test_signer).signature(message))
    }

    /// The message members sign to approve `outer_hash` under `config_version` until `expires_at`.
    pub unconstrained fn approval_message(
        self,
        outer_hash: Field,
        config_version: u32,
        expires_at: u64,
    ) -> Field {
        approval_message(self.vault, outer_hash, config_version, expires_at)
    }

    /// Witness with the vault's signer set, an approval of `message` from each slot where `approving` is true, and
    /// `expires_at`.
    pub unconstrained fn witness(
        self,
        message: Field,
        approving: [bool; MAX_SIGNERS],
        expires_at: u64,
    ) -> [Field; WITNESS_LEN] {
        let mut signatures = [[0; SIGNATURE_LEN]; MAX_SIGNERS];
        for i in 0..MAX_SIGNERS {
            if approving[i] {
                signatures[i] = self.sign(self.accounts[i], message);
            }
        }
        encode_witness(self.keys, signatures, expires_at)
    }
}

/// Encodes signer keys, per-slot signatures and the deadline in the contract's witness layout.
pub fn encode_witness(
    keys: [Point; MAX_SIGNERS],
    signatures: [[Field; SIGNATURE_LEN]; MAX_SIGNERS],
    expires_at: u64,
) -> [Field; WITNESS_LEN] {
    let signatures_offset = MAX_SIGNERS * POINT_LEN;
    let mut fields = [0; WITNESS_LEN];
    for i in 0..MAX_SIGNERS {
        fields[i * POINT_LEN] = keys[i].x;
        fields[i * POINT_LEN + 1] = keys[i].y;
        for j in 0..SIGNATURE_LEN {
            fields[signatures_offset + i * SIGNATURE_LEN + j] = signatures[i][j];
        }
    }
    fields[WITNESS_LEN - 1] = expires_at as Field;
    fields
}

/// `[true; n]` followed by `false` for the remaining slots.
pub fn first(n: u32) -> [bool; MAX_SIGNERS] {
    let mut approving = [false; MAX_SIGNERS];
    for i in 0..MAX_SIGNERS {
        approving[i] = i < n;
    }
    approving
}
