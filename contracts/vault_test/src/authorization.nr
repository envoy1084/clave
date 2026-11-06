use crate::utils::{create_signers, encode_witness, first, ONE_DAY, OUTER_HASH, setup};
use aztec::protocol::{address::AztecAddress, traits::FromField};
use vault_contract::{authorization::approval_message, constants::{MAX_SIGNERS, SIGNATURE_LEN}};

#[test]
unconstrained fn authorizes_with_threshold_approvals() {
    let fixture = setup(3, 2, 2);
    assert(fixture.authorize(OUTER_HASH, fixture.approved(0, first(2))));
}

#[test]
unconstrained fn authorizes_any_threshold_subset() {
    let fixture = setup(3, 2, 2);
    let mut approving = [false; MAX_SIGNERS];
    approving[0] = true;
    approving[2] = true;
    assert(fixture.authorize(OUTER_HASH, fixture.approved(0, approving)));
}

#[test]
unconstrained fn authorizes_with_every_signer() {
    let fixture = setup(MAX_SIGNERS, MAX_SIGNERS, 2);
    assert(fixture.authorize(OUTER_HASH, fixture.approved(0, first(MAX_SIGNERS))));
}

#[test]
unconstrained fn authorizes_when_deadline_equals_anchor_timestamp() {
    let fixture = setup(3, 2, 2);
    let expires_at = fixture.now();
    let witness =
        fixture.witness(fixture.approval_message(OUTER_HASH, 0, expires_at), first(2), expires_at);
    assert(fixture.authorize(OUTER_HASH, witness));
}

#[test]
unconstrained fn rejects_below_threshold() {
    let fixture = setup(3, 2, 2);
    assert(!fixture.authorize(OUTER_HASH, fixture.approved(0, first(1))));
}

#[test]
unconstrained fn rejects_without_approvals() {
    let fixture = setup(3, 1, 2);
    assert(!fixture.authorize(OUTER_HASH, fixture.approved(0, first(0))));
}

#[test(should_fail_with = "approval expired")]
unconstrained fn rejects_expired_approval() {
    let fixture = setup(3, 2, 2);
    let expires_at = fixture.now() - 1;
    let witness =
        fixture.witness(fixture.approval_message(OUTER_HASH, 0, expires_at), first(2), expires_at);
    let _ = fixture.authorize(OUTER_HASH, witness);
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
    let _ = fixture.authorize(OUTER_HASH, witness);
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
    let _ = fixture.authorize(OUTER_HASH, witness);
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
    let _ = fixture.authorize(OUTER_HASH, encode_witness(fixture.keys, signatures, expires_at));
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
    let _ = fixture.authorize(OUTER_HASH, encode_witness(fixture.keys, signatures, expires_at));
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
    let _ = fixture.authorize(OUTER_HASH, encode_witness(keys, signatures, expires_at));
}

#[test(should_fail_with = "invalid signature")]
unconstrained fn rejects_approval_for_another_vault() {
    let fixture = setup(3, 2, 2);
    let expires_at = fixture.now() + ONE_DAY;
    let message = approval_message(AztecAddress::from_field(0x0dd), OUTER_HASH, 0, expires_at);
    let _ = fixture.authorize(OUTER_HASH, fixture.witness(message, first(2), expires_at));
}

#[test(should_fail_with = "invalid signature")]
unconstrained fn rejects_approval_for_another_config_version() {
    let fixture = setup(3, 2, 2);
    let expires_at = fixture.now() + ONE_DAY;
    let witness =
        fixture.witness(fixture.approval_message(OUTER_HASH, 1, expires_at), first(2), expires_at);
    let _ = fixture.authorize(OUTER_HASH, witness);
}
