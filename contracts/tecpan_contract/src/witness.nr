use crate::{
    constants::{MAX_SIGNERS, SIGNATURE_LEN, SIGNER_KEY_LEN, WITNESS_LEN},
    signer_key::SignerKey,
    signer_set::SignerSet,
};
use std::embedded_curve_ops::EmbeddedCurveScalar;

/// Schnorr signature over Grumpkin, as produced by `Schnorr.constructSignature(...).toLimbFields()`.
pub struct Signature {
    pub s: EmbeddedCurveScalar,
    pub e: EmbeddedCurveScalar,
}

impl Signature {
    pub fn from_limbs(limbs: [Field; SIGNATURE_LEN]) -> Self {
        Self {
            s: EmbeddedCurveScalar::new(limbs[0], limbs[1]),
            e: EmbeddedCurveScalar::new(limbs[2], limbs[3]),
        }
    }

    /// An all-zero signature marks a signer that did not approve.
    pub fn is_empty(self) -> bool {
        (self.s.lo == 0) & (self.s.hi == 0) & (self.e.lo == 0) & (self.e.hi == 0)
    }
}

/// Aggregated auth witness for one message.
///
/// Layout (`WITNESS_LEN` fields):
/// - `[0, MAX_SIGNERS * 2)`: signer keys in slot order, `x` then `y`, empty slots zero.
/// - `[MAX_SIGNERS * 2, WITNESS_LEN)`: one 4-limb signature per slot, zero where the signer did not approve.
pub struct ApprovalWitness {
    pub signers: SignerSet,
    pub signatures: [Signature; MAX_SIGNERS],
}

impl ApprovalWitness {
    /// Decodes the witness, taking the signer count from the config it is checked against.
    pub fn from_fields(fields: [Field; WITNESS_LEN], signer_count: u32) -> Self {
        let signatures_offset = MAX_SIGNERS * SIGNER_KEY_LEN;

        let mut keys = [SignerKey::empty(); MAX_SIGNERS];
        let mut signatures = [Signature::from_limbs([0; SIGNATURE_LEN]); MAX_SIGNERS];
        for i in 0..MAX_SIGNERS {
            keys[i] =
                SignerKey { x: fields[i * SIGNER_KEY_LEN], y: fields[i * SIGNER_KEY_LEN + 1] };

            let base = signatures_offset + i * SIGNATURE_LEN;
            signatures[i] = Signature::from_limbs([
                fields[base],
                fields[base + 1],
                fields[base + 2],
                fields[base + 3],
            ]);
        }

        Self { signers: SignerSet { keys, count: signer_count }, signatures }
    }
}

mod test {
    use crate::constants::{MAX_SIGNERS, WITNESS_LEN};
    use super::ApprovalWitness;

    #[test]
    fn decodes_keys_and_signatures_by_slot() {
        let mut fields = [0; WITNESS_LEN];
        for i in 0..WITNESS_LEN {
            fields[i] = (i + 1) as Field;
        }

        let witness = ApprovalWitness::from_fields(fields, 3);

        assert_eq(witness.signers.count, 3);
        assert_eq(witness.signers.keys[0].x, 1);
        assert_eq(witness.signers.keys[0].y, 2);
        assert_eq(witness.signers.keys[MAX_SIGNERS - 1].y, (MAX_SIGNERS * 2) as Field);

        let first = witness.signatures[0];
        assert_eq(first.s.lo, (MAX_SIGNERS * 2 + 1) as Field);
        assert_eq(first.e.hi, (MAX_SIGNERS * 2 + 4) as Field);
        assert_eq(witness.signatures[MAX_SIGNERS - 1].e.hi, WITNESS_LEN as Field);
    }

    #[test]
    fn zero_signature_is_empty() {
        let witness = ApprovalWitness::from_fields([0; WITNESS_LEN], 1);
        assert(witness.signatures[0].is_empty());
    }
}
