use aztec::{
    context::{PrivateCall, UtilityCall},
    keys::getters::get_public_keys,
    protocol::{address::AztecAddress, traits::Deserialize},
    test::helpers::{
        test_environment::{CallPrivateOptions, ExecuteUtilityOptions, TestEnvironment},
        txe_oracles,
    },
};
use tecpan_contract::{
    authorization::approval_message,
    constants::{MAX_SIGNERS, SIGNATURE_LEN, SIGNER_KEY_LEN, WITNESS_LEN},
    signer_key::SignerKey,
    TecpanMultisig,
};
use test_signer_contract::TestSigner;

/// A deployed multisig together with the test accounts whose keys it was created with.
pub struct Fixture {
    pub env: TestEnvironment,
    pub deployer: AztecAddress,
    pub multisig: AztecAddress,
    pub test_signer: AztecAddress,
    pub accounts: [AztecAddress; MAX_SIGNERS],
    pub keys: [SignerKey; MAX_SIGNERS],
    pub signer_count: u32,
    pub threshold: u32,
}

/// Creates `count` signer accounts and returns their addresses and Grumpkin signing keys, zero-padded.
///
/// The TXE signs auth witnesses with an account's incoming viewing secret, so its `ivpk_m` is the signing key.
pub unconstrained fn create_signers(
    env: &mut TestEnvironment,
    count: u32,
) -> ([AztecAddress; MAX_SIGNERS], [SignerKey; MAX_SIGNERS]) {
    let mut accounts = [AztecAddress::zero(); MAX_SIGNERS];
    let mut keys = [SignerKey::empty(); MAX_SIGNERS];
    for i in 0..count {
        let account = env.create_light_account();
        let point = env.utility_context(|_context| get_public_keys(account).ivpk_m.inner);
        accounts[i] = account;
        keys[i] = SignerKey { x: point.x, y: point.y };
    }
    (accounts, keys)
}

pub unconstrained fn deploy_multisig(
    env: &mut TestEnvironment,
    deployer: AztecAddress,
    keys: [SignerKey; MAX_SIGNERS],
    signer_count: u32,
    threshold: u32,
) -> AztecAddress {
    // `with_private_initializer` only scopes the call to `from`. Like an Aztec.js account deployment, the deployer
    // also needs the multisig in scope so the constructor can use the multisig's own keys.
    let deployment = env.deploy("@tecpan_contract/TecpanMultisig");
    let constructor = TecpanMultisig::interface().constructor(keys, signer_count, threshold);
    let multisig = txe_oracles::deploy(
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
        CallPrivateOptions::new().with_additional_scopes([multisig]),
        PrivateCall::<_, _, ()>::new(
            multisig,
            constructor.selector,
            constructor.name,
            constructor.args,
        ),
    );
    multisig
}

/// Deploys a `threshold`-of-`signer_count` multisig with fresh signer accounts.
pub unconstrained fn setup(signer_count: u32, threshold: u32) -> Fixture {
    let mut env = TestEnvironment::new();
    let deployer = env.create_light_account();
    let (accounts, keys) = create_signers(&mut env, signer_count);
    let multisig = deploy_multisig(&mut env, deployer, keys, signer_count, threshold);
    let test_signer = env.deploy("@test_signer_contract/TestSigner").without_initializer();

    let fixture =
        Fixture { env, deployer, multisig, test_signer, accounts, keys, signer_count, threshold };
    // A PXE discovers the multisig's notes before simulating a transaction. Inline private contexts skip that step, so
    // run a utility call once to sync them.
    let _ = fixture.view(TecpanMultisig::at(multisig).get_config());
    fixture
}

impl Fixture {
    /// Runs a utility function of the multisig with the multisig itself in scope, as a co-signer's PXE would.
    pub unconstrained fn view<let M: u32, let N: u32, T>(self, call: UtilityCall<M, N, T>) -> T
    where
        T: Deserialize,
    {
        self.env.execute_utility_opts(ExecuteUtilityOptions::new().with_from(self.multisig), call)
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

    /// The message signers approve for `outer_hash` under the multisig's current config.
    pub unconstrained fn approval_message(self, outer_hash: Field, config_version: u32) -> Field {
        approval_message(self.multisig, outer_hash, config_version)
    }

    /// Witness with the multisig's signer set and an approval of `message` from each slot where `approving` is true.
    pub unconstrained fn witness(
        self,
        message: Field,
        approving: [bool; MAX_SIGNERS],
    ) -> [Field; WITNESS_LEN] {
        let mut signatures = [[0; SIGNATURE_LEN]; MAX_SIGNERS];
        for i in 0..MAX_SIGNERS {
            if approving[i] {
                signatures[i] = self.sign(self.accounts[i], message);
            }
        }
        encode_witness(self.keys, signatures)
    }
}

/// Encodes signer keys and per-slot signatures in the contract's witness layout.
pub fn encode_witness(
    keys: [SignerKey; MAX_SIGNERS],
    signatures: [[Field; SIGNATURE_LEN]; MAX_SIGNERS],
) -> [Field; WITNESS_LEN] {
    let signatures_offset = MAX_SIGNERS * SIGNER_KEY_LEN;
    let mut fields = [0; WITNESS_LEN];
    for i in 0..MAX_SIGNERS {
        fields[i * SIGNER_KEY_LEN] = keys[i].x;
        fields[i * SIGNER_KEY_LEN + 1] = keys[i].y;
        for j in 0..SIGNATURE_LEN {
            fields[signatures_offset + i * SIGNATURE_LEN + j] = signatures[i][j];
        }
    }
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
