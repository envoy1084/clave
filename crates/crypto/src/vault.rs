//! The vault contract's wire formats (`architecture/contract.md`), identical to `contracts/vault_contract`.

use ark_ff::{BigInteger, PrimeField, Zero};

use crate::{Base, Error, Point, poseidon2};

/// Member slots.
pub const MAX_SIGNERS: usize = 8;
/// Fields per Schnorr signature (`s.lo`, `s.hi`, `e.lo`, `e.hi`).
pub const SIGNATURE_LEN: usize = 4;
/// Auth witness length: signer keys, signature slots, `expires_at`.
pub const WITNESS_LEN: usize = MAX_SIGNERS * (2 + SIGNATURE_LEN) + 1;

/// ASCII `tecpan.approval.v0`.
pub const DOM_SEP_APPROVAL: &[u8] = b"tecpan.approval.v0";
/// ASCII `tecpan.signers.v0`.
pub const DOM_SEP_SIGNER_SET: &[u8] = b"tecpan.signers.v0";
/// ASCII `tecpan.committee.v0`.
pub const DOM_SEP_COMMITTEE: &[u8] = b"tecpan.committee.v0";

/// A domain tag as a field element: its ASCII bytes read big-endian.
#[must_use]
pub fn domain_separator(tag: &[u8]) -> Base {
    Base::from_be_bytes_mod_order(tag)
}

/// `[x_0, y_0, ..., x_7, y_7]` with empty slots zero.
fn padded_points(points: &[Point]) -> Result<[Base; 2 * MAX_SIGNERS], Error> {
    if points.len() > MAX_SIGNERS {
        return Err(Error::TooManyMembers {
            max: MAX_SIGNERS,
            actual: points.len(),
        });
    }
    let mut fields = [Base::zero(); 2 * MAX_SIGNERS];
    for (i, point) in points.iter().enumerate() {
        fields[2 * i] = point.x();
        fields[2 * i + 1] = point.y();
    }
    Ok(fields)
}

/// `Poseidon2(tecpan.signers.v0, [x_0, y_0, ..., x_7, y_7])` over the ordered signer keys.
///
/// # Errors
/// [`Error::TooManyMembers`] for more than [`MAX_SIGNERS`] keys.
pub fn signer_set_commitment(signer_keys: &[Point]) -> Result<Base, Error> {
    Ok(poseidon2::hash_with_separator(
        domain_separator(DOM_SEP_SIGNER_SET),
        &padded_points(signer_keys)?,
    ))
}

/// `Poseidon2(tecpan.committee.v0, [t_v, Y_0.x, Y_0.y, ..., Y_7.y])` over the ordered verification shares.
///
/// # Errors
/// [`Error::TooManyMembers`] for more than [`MAX_SIGNERS`] shares.
pub fn committee_commitment(
    viewing_threshold: u32,
    verification_shares: &[Point],
) -> Result<Base, Error> {
    let shares = padded_points(verification_shares)?;
    let mut inputs = Vec::with_capacity(1 + shares.len());
    inputs.push(Base::from(viewing_threshold));
    inputs.extend_from_slice(&shares);
    Ok(poseidon2::hash_with_separator(
        domain_separator(DOM_SEP_COMMITTEE),
        &inputs,
    ))
}

/// The message each signer signs: `Poseidon2(tecpan.approval.v0, [vault, outer_hash, version, expires_at])`.
#[must_use]
pub fn approval_message(
    vault: Base,
    outer_hash: Base,
    config_version: u32,
    expires_at: u64,
) -> Base {
    poseidon2::hash_with_separator(
        domain_separator(DOM_SEP_APPROVAL),
        &[
            vault,
            outer_hash,
            Base::from(config_version),
            Base::from(expires_at),
        ],
    )
}

/// Encodes the 49-field auth witness: ordered signer keys, one 4-limb signature per slot (`None` = no approval),
/// then `expires_at`.
///
/// # Errors
/// [`Error::TooManyMembers`] for more than [`MAX_SIGNERS`] keys, [`Error::InvalidLength`] if `signatures` does not have
/// one entry per key.
pub fn encode_witness(
    signer_keys: &[Point],
    signatures: &[Option<[Base; SIGNATURE_LEN]>],
    expires_at: u64,
) -> Result<[Base; WITNESS_LEN], Error> {
    if signatures.len() != signer_keys.len() {
        return Err(Error::InvalidLength {
            expected: signer_keys.len(),
            actual: signatures.len(),
        });
    }
    let keys = padded_points(signer_keys)?;
    let mut witness = [Base::zero(); WITNESS_LEN];
    witness[..keys.len()].copy_from_slice(&keys);
    let offset = 2 * MAX_SIGNERS;
    for (slot, signature) in signatures.iter().enumerate() {
        if let Some(limbs) = signature {
            let start = offset + slot * SIGNATURE_LEN;
            witness[start..start + SIGNATURE_LEN].copy_from_slice(limbs);
        }
    }
    witness[WITNESS_LEN - 1] = Base::from(expires_at);
    Ok(witness)
}

/// Hex of a field element, `0x`-prefixed, 32 bytes (for logs and test vectors).
#[must_use]
pub fn field_hex(value: &Base) -> String {
    use core::fmt::Write as _;

    let bytes = value.into_bigint().to_bytes_be();
    let mut out = String::with_capacity(2 + 2 * bytes.len());
    out.push_str("0x");
    for byte in bytes {
        // Writing to a String cannot fail.
        let _ = write!(out, "{byte:02x}");
    }
    out
}
