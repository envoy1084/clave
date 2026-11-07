//! Vault formats shared with the Noir contract: commitments, the approval message, the auth witness and Schnorr
//! signature limbs.

use clave_crypto::{schnorr, vault};
use napi::{Result, bindgen_prelude::Buffer};
use napi_derive::napi;

use crate::convert::{Point, base, base_hex, invalid, limbs, point, points, unix_seconds};

/// `Poseidon2("clave.approval.v0", [vault, outerHash, configVersion, expiresAt])`: the message every signer signs.
#[napi]
pub fn approval_message(
    vault: String,
    outer_hash: String,
    config_version: u32,
    expires_at: i64,
) -> Result<String> {
    Ok(base_hex(&vault::approval_message(
        base(&vault)?,
        base(&outer_hash)?,
        config_version,
        unix_seconds(expires_at)?,
    )))
}

/// The contract's `signer_set_commitment` over ordered signer keys (at most 8).
#[napi]
pub fn signer_set_commitment(signer_keys: Vec<Point>) -> Result<String> {
    Ok(base_hex(
        &vault::signer_set_commitment(&points(&signer_keys)?).map_err(invalid)?,
    ))
}

/// The contract's `committee_commitment` over the viewing threshold and ordered verification shares.
#[napi]
pub fn committee_commitment(
    viewing_threshold: u32,
    verification_shares: Vec<Point>,
) -> Result<String> {
    Ok(base_hex(
        &vault::committee_commitment(viewing_threshold, &points(&verification_shares)?)
            .map_err(invalid)?,
    ))
}

/// The 49-field auth witness: one entry of `signatures` per signer slot, `null` where the slot did not approve.
#[napi]
pub fn encode_witness(
    signer_keys: Vec<Point>,
    signatures: Vec<Option<Vec<String>>>,
    expires_at: i64,
) -> Result<Vec<String>> {
    let signatures = signatures
        .iter()
        .map(|s| s.as_deref().map(limbs).transpose())
        .collect::<Result<Vec<_>>>()?;
    let witness = vault::encode_witness(
        &points(&signer_keys)?,
        &signatures,
        unix_seconds(expires_at)?,
    )
    .map_err(invalid)?;
    Ok(witness.iter().map(base_hex).collect())
}

/// Splits a 64-byte Aztec Schnorr signature (`s ‖ e`) into the contract's limbs `[s.lo, s.hi, e.lo, e.hi]`.
#[napi]
pub fn signature_limbs(signature: Buffer) -> Result<Vec<String>> {
    let bytes: &[u8; 64] = signature
        .as_ref()
        .try_into()
        .map_err(|_| invalid(format!("expected 64 bytes, got {}", signature.len())))?;
    let signature = schnorr::Signature::from_bytes(bytes).map_err(invalid)?;
    Ok(signature.to_limbs().iter().map(base_hex).collect())
}

/// Whether `limbs` are a valid Schnorr signature by `signerKey` over `message`, exactly as the contract checks it.
#[napi]
pub fn verify_signature(
    signer_key: Point,
    message: String,
    signature: Vec<String>,
) -> Result<bool> {
    let Ok(signature) = schnorr::Signature::from_limbs(&limbs(&signature)?) else {
        return Ok(false);
    };
    Ok(schnorr::verify(
        &point(&signer_key)?,
        base(&message)?,
        &signature,
    ))
}
