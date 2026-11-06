use crate::utils::{create_signers, encode_witness, first, Fixture, setup};
use aztec::protocol::{address::AztecAddress, traits::FromField};
use std::test::OracleMock;
use vault_contract::{
    authorization::{approval_message, is_authorized},
    constants::{MAX_SIGNERS, SIGNATURE_LEN, WITNESS_LEN},
    TecpanVault,
};

global OUTER_HASH: Field = 0x7ec9a2;
global ONE_DAY: u64 = 86400;

/// Runs the vault's authorization check for `outer_hash` in a private context at the vault, serving `witness` as the
/// auth witness.
unconstrained fn authorize(
    fixture: Fixture,
    outer_hash: Field,
    witness: [Field; WITNESS_LEN],
) -> bool {
    let _ = OracleMock::mock("aztec_utl_getAuthWitness").returns(witness);
    fixture.env.private_context_at(fixture.vault, |context| {
        let config_slot = TecpanVault::storage_layout().config.slot;
        is_authorized(context, config_slot, outer_hash)
    })
}

/// Witness approving `OUTER_HASH` at config version 0 by the given slots, valid for a day.
unconstrained fn approved_by(
    fixture: Fixture,
    approving: [bool; MAX_SIGNERS],
) -> [Field; WITNESS_LEN] {
    let expires_at = fixture.now() + ONE_DAY;
    fixture.witness(fixture.approval_message(OUTER_HASH, 0, expires_at), approving, expires_at)
}

#[test]
unconstrained fn authorizes_with_threshold_approvals() {
    let fixture = setup(3, 2, 2);
    assert(authorize(fixture, OUTER_HASH, approved_by(fixture, first(2))));
}

#[test]
unconstrained fn authorizes_any_threshold_subset() {
    let fixture = setup(3, 2, 2);
    let mut approving = [false; MAX_SIGNERS];
    approving[0] = true;
    approving[2] = true;
    assert(authorize(fixture, OUTER_HASH, approved_by(fixture, approving)));
}

#[test]
unconstrained fn authorizes_with_every_signer() {
    let fixture = setup(MAX_SIGNERS, MAX_SIGNERS, 2);
    assert(
        authorize(
            fixture,
            OUTER_HASH,
            approved_by(fixture, first(MAX_SIGNERS)),
        ),
    );
}

#[test]
unconstrained fn authorizes_when_deadline_equals_anchor_timestamp() {
    let fixture = setup(3, 2, 2);
    let expires_at = fixture.now();
    let witness =
        fixture.witness(fixture.approval_message(OUTER_HASH, 0, expires_at), first(2), expires_at);
    assert(authorize(fixture, OUTER_HASH, witness));
}

#[test]
unconstrained fn rejects_below_threshold() {
    let fixture = setup(3, 2, 2);
    assert(!authorize(fixture, OUTER_HASH, approved_by(fixture, first(1))));
}

#[test]
unconstrained fn rejects_without_approvals() {
    let fixture = setup(3, 1, 2);
    assert(!authorize(fixture, OUTER_HASH, approved_by(fixture, first(0))));
}

#[test(should_fail_with = "approval expired")]
unconstrained fn rejects_expired_approval() {
    let fixture = setup(3, 2, 2);
    let expires_at = fixture.now() - 1;
    let witness =
        fixture.witness(fixture.approval_message(OUTER_HASH, 0, expires_at), first(2), expires_at);
    let _ = authorize(fixture, OUTER_HASH, witness);
}

#[test(should_fail_with = "invalid signature")]
unconstrained fn rejects_deadline_different_from_signed_one() {
    let fixture = setup(3, 2, 2);
    let signed_deadline = fixture.now() + ONE_DAY;
    let witness = fixture.witness(
        fixture.approval_message(OUTER_HASH, 0, signed_deadline),
        first(2),
        signed_deadline + ONE_DAY,
    );
    let _ = authorize(fixture, OUTER_HASH, witness);
}

#[test(should_fail_with = "invalid signature")]
unconstrained fn rejects_signature_over_other_hash() {
    let fixture = setup(3, 2, 2);
    let expires_at = fixture.now() + ONE_DAY;
    let witness = fixture.witness(
        fixture.approval_message(OUTER_HASH + 1, 0, expires_at),
        first(2),
        expires_at,
    );
    let _ = authorize(fixture, OUTER_HASH, witness);
}

#[test(should_fail_with = "invalid signature")]
unconstrained fn rejects_signature_in_another_signers_slot() {
    let fixture = setup(3, 2, 2);
    let expires_at = fixture.now() + ONE_DAY;
    let first_signature =
        fixture.sign(fixture.accounts[0], fixture.approval_message(OUTER_HASH, 0, expires_at));

    let mut signatures = [[0; SIGNATURE_LEN]; MAX_SIGNERS];
    signatures[0] = first_signature;
    signatures[1] = first_signature;
    let _ = authorize(
        fixture,
        OUTER_HASH,
        encode_witness(fixture.keys, signatures, expires_at),
    );
}

#[test(should_fail_with = "signature for empty signer slot")]
unconstrained fn rejects_signature_in_empty_slot() {
    let fixture = setup(2, 2, 2);
    let expires_at = fixture.now() + ONE_DAY;
    let message = fixture.approval_message(OUTER_HASH, 0, expires_at);
    let mut signatures = [[0; SIGNATURE_LEN]; MAX_SIGNERS];
    signatures[0] = fixture.sign(fixture.accounts[0], message);
    signatures[1] = fixture.sign(fixture.accounts[1], message);
    signatures[2] = [1, 0, 0, 0];
    let _ = authorize(
        fixture,
        OUTER_HASH,
        encode_witness(fixture.keys, signatures, expires_at),
    );
}

#[test(should_fail_with = "signer set mismatch")]
unconstrained fn rejects_substituted_signer() {
    let mut fixture = setup(3, 2, 2);
    let expires_at = fixture.now() + ONE_DAY;
    let message = fixture.approval_message(OUTER_HASH, 0, expires_at);
    let (outsiders, outsider_keys) = create_signers(&mut fixture.env, 1);

    let mut keys = fixture.keys;
    keys[2] = outsider_keys[0];
    let mut signatures = [[0; SIGNATURE_LEN]; MAX_SIGNERS];
    signatures[0] = fixture.sign(fixture.accounts[0], message);
    signatures[2] = fixture.sign(outsiders[0], message);
    let _ = authorize(
        fixture,
        OUTER_HASH,
        encode_witness(keys, signatures, expires_at),
    );
}

#[test(should_fail_with = "invalid signature")]
unconstrained fn rejects_approval_for_another_vault() {
    let fixture = setup(3, 2, 2);
    let expires_at = fixture.now() + ONE_DAY;
    let message = approval_message(AztecAddress::from_field(0x0dd), OUTER_HASH, 0, expires_at);
    let _ = authorize(
        fixture,
        OUTER_HASH,
        fixture.witness(message, first(2), expires_at),
    );
}

#[test(should_fail_with = "invalid signature")]
unconstrained fn rejects_approval_for_another_config_version() {
    let fixture = setup(3, 2, 2);
    let expires_at = fixture.now() + ONE_DAY;
    let witness =
        fixture.witness(fixture.approval_message(OUTER_HASH, 1, expires_at), first(2), expires_at);
    let _ = authorize(fixture, OUTER_HASH, witness);
}
