//! k-of-n Schnorr authorization shared by `entrypoint`, `verify_private_authwit` and `lookup_validity`.

use crate::{
    constants::{CONFIG_MAX_AGE, DOM_SEP__APPROVAL, MAX_SIGNERS, WITNESS_LEN},
    notes::config_note::ConfigNote,
    signer_set::SignerSet,
    witness::{ApprovalWitness, Signature},
};
use aztec::{
    context::PrivateContext,
    history::note::assert_local_note_was_not_nullified_by,
    note::note_getter::get_note,
    oracle::auth_witness::get_auth_witness,
    protocol::{address::AztecAddress, hash::poseidon2_hash_with_separator, traits::ToField},
};
use std::embedded_curve_ops::EmbeddedCurvePoint;

/// The message every signer signs to approve `outer_hash` for `account` under config `config_version`.
///
/// Binding the account address stops an approval being replayed on another multisig with the same signers; binding the
/// config version invalidates approvals collected before a config change.
pub fn approval_message(account: AztecAddress, outer_hash: Field, config_version: u32) -> Field {
    poseidon2_hash_with_separator(
        [account.to_field(), outer_hash, config_version as Field],
        DOM_SEP__APPROVAL,
    )
}

/// Counts valid approvals of `message`. Empty signature slots are skipped; any non-empty signature must be valid and
/// belong to an occupied slot.
pub fn count_approvals(
    signers: SignerSet,
    signatures: [Signature; MAX_SIGNERS],
    message: Field,
) -> u32 {
    let mut approvals: u32 = 0;
    for i in 0..MAX_SIGNERS {
        let signature = signatures[i];
        let has_signature = !signature.is_empty();
        if has_signature {
            assert(i < signers.count, "signature for empty signer slot");
        }

        // Empty slots hold the zero key, which is not a curve point; verify against the generator instead so the
        // (unused) verification stays well defined.
        let key = signers.keys[i];
        let point = if key.is_empty() {
            EmbeddedCurvePoint::generator()
        } else {
            key.to_point()
        };
        let valid = schnorr::verify_signature(point, (signature.s, signature.e), message);
        assert(!has_signature | valid, "invalid signature");

        approvals += has_signature as u32;
    }
    approvals
}

/// Whether `witness` carries at least `config.threshold` valid approvals of `outer_hash` for `account`.
pub fn check_witness(
    config: ConfigNote,
    witness: ApprovalWitness,
    account: AztecAddress,
    outer_hash: Field,
) -> bool {
    assert(witness.signers.commitment() == config.signer_set_commitment, "signer set mismatch");
    let message = approval_message(account, outer_hash, config.version);
    count_approvals(witness.signers, witness.signatures, message) >= config.threshold
}

/// Constrained authorization check against the config note stored at `config_slot`.
///
/// Reads the config without nullifying it, so it is valid inside static calls, and proves the note was current at the
/// anchor block. The transaction's expiration is capped to `CONFIG_MAX_AGE` after the anchor block, bounding how long
/// a superseded config can still authorize.
pub fn is_authorized(context: &mut PrivateContext, config_slot: Field, outer_hash: Field) -> bool {
    let anchor = context.get_anchor_block_header();

    let config = get_note::<ConfigNote>(context, Option::none(), config_slot);
    assert_local_note_was_not_nullified_by(anchor, config, context);
    context.set_expiration_timestamp(anchor.timestamp() + CONFIG_MAX_AGE);

    // Safety: the witness is only a hint. Its signer keys are checked against the stored commitment and every
    // counted signature is verified, so a malicious oracle can only make authorization fail.
    let fields: [Field; WITNESS_LEN] = unsafe { get_auth_witness(outer_hash) };
    let witness = ApprovalWitness::from_fields(fields, config.note.signer_count);

    check_witness(config.note, witness, context.this_address(), outer_hash)
}

mod test {
    use super::approval_message;
    use aztec::protocol::{address::AztecAddress, traits::FromField};

    #[test]
    fn approval_message_binds_account_hash_and_version() {
        let account = AztecAddress::from_field(1);
        let base = approval_message(account, 2, 3);

        assert(base != approval_message(AztecAddress::from_field(9), 2, 3));
        assert(base != approval_message(account, 9, 3));
        assert(base != approval_message(account, 2, 9));
    }
}
