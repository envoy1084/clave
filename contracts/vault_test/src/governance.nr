use crate::utils::{first, OUTER_HASH, setup};
use vault_contract::TecpanVault;

#[test]
unconstrained fn set_threshold_updates_policy_and_version() {
    let fixture = setup(3, 2, 2);
    fixture.self_call(TecpanVault::at(fixture.vault).set_threshold(3));

    let config = fixture.view(TecpanVault::at(fixture.vault).get_config());
    assert_eq(config.threshold, 3);
    assert_eq(config.version, 1);
    assert_eq(config.signer_count, 3);

    assert(!fixture.authorize(OUTER_HASH, fixture.approved(1, first(2))));
    assert(fixture.authorize(OUTER_HASH, fixture.approved(1, first(3))));
}

#[test(should_fail_with = "invalid signature")]
unconstrained fn set_threshold_invalidates_earlier_approvals() {
    let fixture = setup(3, 2, 2);
    fixture.self_call(TecpanVault::at(fixture.vault).set_threshold(2));
    let _ = fixture.authorize(OUTER_HASH, fixture.approved(0, first(2)));
}

#[test(should_fail_with = "threshold out of range")]
unconstrained fn set_threshold_rejects_threshold_above_signer_count() {
    let fixture = setup(3, 2, 2);
    fixture.self_call(TecpanVault::at(fixture.vault).set_threshold(4));
}

#[test(should_fail_with = "threshold out of range")]
unconstrained fn set_threshold_rejects_zero() {
    let fixture = setup(3, 2, 2);
    fixture.self_call(TecpanVault::at(fixture.vault).set_threshold(0));
}

#[test(should_fail_with = "can only be called by the same contract")]
unconstrained fn set_threshold_rejects_outside_caller() {
    let fixture = setup(3, 2, 2);
    fixture.env.call_private(fixture.accounts[0], TecpanVault::at(fixture.vault).set_threshold(1));
}

#[test]
unconstrained fn invalidate_pending_bumps_version_only() {
    let fixture = setup(3, 2, 2);
    let before = fixture.view(TecpanVault::at(fixture.vault).get_config());
    fixture.self_call(TecpanVault::at(fixture.vault).invalidate_pending());

    let after = fixture.view(TecpanVault::at(fixture.vault).get_config());
    assert_eq(after.version, before.version + 1);
    assert_eq(after.threshold, before.threshold);
    assert_eq(after.signer_set_commitment, before.signer_set_commitment);
    assert_eq(after.committee_commitment, before.committee_commitment);
    assert(fixture.authorize(OUTER_HASH, fixture.approved(1, first(2))));
}

#[test(should_fail_with = "invalid signature")]
unconstrained fn invalidate_pending_rejects_collected_approvals() {
    let fixture = setup(3, 2, 2);
    let collected = fixture.approved(0, first(3));
    fixture.self_call(TecpanVault::at(fixture.vault).invalidate_pending());
    let _ = fixture.authorize(OUTER_HASH, collected);
}

#[test(should_fail_with = "can only be called by the same contract")]
unconstrained fn invalidate_pending_rejects_outside_caller() {
    let fixture = setup(3, 2, 2);
    fixture.env.call_private(
        fixture.accounts[1],
        TecpanVault::at(fixture.vault).invalidate_pending(),
    );
}
