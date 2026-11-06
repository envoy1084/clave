use crate::utils::{create_signers, encode_witness, first, Fixture, setup};
use aztec::protocol::{address::AztecAddress, traits::FromField};
use std::test::OracleMock;
use tecpan_contract::{
    authorization::{approval_message, is_authorized},
    constants::{MAX_SIGNERS, WITNESS_LEN},
    TecpanMultisig,
};

global OUTER_HASH: Field = 0x7ec9a2;

/// Runs the contract's authorization check for `outer_hash` inside the multisig, serving `witness` as the auth witness.
unconstrained fn authorize(
    fixture: Fixture,
    outer_hash: Field,
    witness: [Field; WITNESS_LEN],
) -> bool {
    let _ = OracleMock::mock("aztec_utl_getAuthWitness").returns(witness);
    fixture.env.private_context_at(fixture.multisig, |context| {
        let config_slot = TecpanMultisig::storage_layout().config.slot;
        is_authorized(context, config_slot, outer_hash)
    })
}

#[test]
unconstrained fn authorizes_with_threshold_approvals() {
    let fixture = setup(3, 2);
    let message = fixture.approval_message(OUTER_HASH, 0);
    assert(authorize(fixture, OUTER_HASH, fixture.witness(message, first(2))));
}

#[test]
unconstrained fn authorizes_any_threshold_subset() {
    let fixture = setup(3, 2);
    let message = fixture.approval_message(OUTER_HASH, 0);
    let mut approving = [false; MAX_SIGNERS];
    approving[0] = true;
    approving[2] = true;
    assert(authorize(fixture, OUTER_HASH, fixture.witness(message, approving)));
}

#[test]
unconstrained fn authorizes_with_every_signer() {
    let fixture = setup(MAX_SIGNERS, MAX_SIGNERS);
    let message = fixture.approval_message(OUTER_HASH, 0);
    assert(
        authorize(
            fixture,
            OUTER_HASH,
            fixture.witness(message, first(MAX_SIGNERS)),
        ),
    );
}

#[test]
unconstrained fn rejects_below_threshold() {
    let fixture = setup(3, 2);
    let message = fixture.approval_message(OUTER_HASH, 0);
    assert(!authorize(fixture, OUTER_HASH, fixture.witness(message, first(1))));
}

#[test]
unconstrained fn rejects_without_approvals() {
    let fixture = setup(3, 1);
    let message = fixture.approval_message(OUTER_HASH, 0);
    assert(!authorize(fixture, OUTER_HASH, fixture.witness(message, first(0))));
}

#[test(should_fail_with = "invalid signature")]
unconstrained fn rejects_signature_over_other_hash() {
    let fixture = setup(3, 2);
    let message = fixture.approval_message(OUTER_HASH + 1, 0);
    let _ = authorize(fixture, OUTER_HASH, fixture.witness(message, first(2)));
}

#[test(should_fail_with = "invalid signature")]
unconstrained fn rejects_signature_in_another_signers_slot() {
    let fixture = setup(3, 2);
    let message = fixture.approval_message(OUTER_HASH, 0);
    let first_signature = fixture.sign(fixture.accounts[0], message);

    let mut signatures = [[0; 4]; MAX_SIGNERS];
    signatures[0] = first_signature;
    signatures[1] = first_signature;
    let _ = authorize(
        fixture,
        OUTER_HASH,
        encode_witness(fixture.keys, signatures),
    );
}

#[test(should_fail_with = "signature for empty signer slot")]
unconstrained fn rejects_signature_in_empty_slot() {
    let fixture = setup(2, 2);
    let message = fixture.approval_message(OUTER_HASH, 0);
    let mut signatures = [[0; 4]; MAX_SIGNERS];
    signatures[0] = fixture.sign(fixture.accounts[0], message);
    signatures[1] = fixture.sign(fixture.accounts[1], message);
    signatures[2] = [1, 0, 0, 0];
    let _ = authorize(
        fixture,
        OUTER_HASH,
        encode_witness(fixture.keys, signatures),
    );
}

#[test(should_fail_with = "signer set mismatch")]
unconstrained fn rejects_substituted_signer() {
    let mut fixture = setup(3, 2);
    let message = fixture.approval_message(OUTER_HASH, 0);
    let (outsiders, outsider_keys) = create_signers(&mut fixture.env, 1);

    let mut keys = fixture.keys;
    keys[2] = outsider_keys[0];
    let mut signatures = [[0; 4]; MAX_SIGNERS];
    signatures[0] = fixture.sign(fixture.accounts[0], message);
    signatures[2] = fixture.sign(outsiders[0], message);
    let _ = authorize(fixture, OUTER_HASH, encode_witness(keys, signatures));
}

#[test(should_fail_with = "invalid signature")]
unconstrained fn rejects_approval_for_another_account() {
    let fixture = setup(3, 2);
    let message = approval_message(AztecAddress::from_field(0x0dd), OUTER_HASH, 0);
    let _ = authorize(fixture, OUTER_HASH, fixture.witness(message, first(2)));
}

#[test(should_fail_with = "invalid signature")]
unconstrained fn rejects_approval_for_another_config_version() {
    let fixture = setup(3, 2);
    let message = fixture.approval_message(OUTER_HASH, 1);
    let _ = authorize(fixture, OUTER_HASH, fixture.witness(message, first(2)));
}
