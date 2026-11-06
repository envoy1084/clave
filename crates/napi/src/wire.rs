//! Proposal and approval bodies in the canonical wire encoding (signed and sent by the member's keeper).

use napi::{Result, bindgen_prelude::Buffer};
use napi_derive::napi;
use tecpan_crypto::schnorr;
use tecpan_wire::{Message, WIRE_VERSION, decode_body, encode_body, messages};

use crate::convert::{
    Point, base, base_hex, bytes32, hex32, invalid, limbs, point, timestamp_out, unix_seconds,
};

/// A spending proposal.
#[napi(object)]
pub struct Proposal {
    /// Config version the approvals are collected under.
    pub config_version: u32,
    /// The entrypoint payload hash members approve.
    pub outer_hash: String,
    /// Approval deadline (unix seconds).
    pub expires_at: i64,
    /// Entrypoint `tx_nonce`.
    pub tx_nonce: String,
    /// Fee payment method (`0..=255`).
    pub fee_method: u32,
    /// Opaque serialised execution payload.
    pub payload: Buffer,
    /// Human-readable summary.
    pub summary: String,
    /// Proposing member index.
    pub proposer: u32,
}

/// A member's approval of a proposal.
#[napi(object)]
pub struct Approval {
    /// Proposal id (32-byte hex).
    pub proposal: String,
    /// Signer slot.
    pub slot: u32,
    /// Signature limbs `[s.lo, s.hi, e.lo, e.hi]`.
    pub signature: Vec<String>,
}

/// The wire version this build speaks.
#[napi]
#[must_use]
pub fn wire_version() -> u32 {
    u32::from(WIRE_VERSION)
}

/// `SHA-256("tecpan.proposal.v0" ‖ canonical(proposal))`.
#[napi]
pub fn proposal_id(proposal: Proposal) -> Result<String> {
    Ok(hex32(&to_wire(&proposal)?.id().map_err(invalid)?))
}

/// The approval message of `proposal` on `vault`.
#[napi]
pub fn proposal_approval_message(proposal: Proposal, vault: String) -> Result<String> {
    Ok(base_hex(
        &to_wire(&proposal)?
            .approval_message(base(&vault)?)
            .map_err(invalid)?,
    ))
}

/// Canonical bytes of a proposal body.
#[napi]
pub fn encode_proposal(proposal: Proposal) -> Result<Buffer> {
    Ok(encode_body(&Message::Proposal(to_wire(&proposal)?))
        .map_err(invalid)?
        .into())
}

/// Decodes a proposal body.
#[napi]
pub fn decode_proposal(bytes: Buffer) -> Result<Proposal> {
    match decode_body(&bytes).map_err(invalid)? {
        Message::Proposal(proposal) => from_wire(proposal),
        _ => Err(invalid("not a proposal")),
    }
}

/// Canonical bytes of an approval body.
#[napi]
pub fn encode_approval(approval: Approval) -> Result<Buffer> {
    Ok(
        encode_body(&Message::Approval(approval_to_wire(&approval)?))
            .map_err(invalid)?
            .into(),
    )
}

/// Decodes an approval body.
#[napi]
pub fn decode_approval(bytes: Buffer) -> Result<Approval> {
    match decode_body(&bytes).map_err(invalid)? {
        Message::Approval(approval) => Ok(Approval {
            proposal: hex32(&approval.proposal),
            slot: approval.slot,
            signature: approval.signature.iter().map(hex32).collect(),
        }),
        _ => Err(invalid("not an approval")),
    }
}

/// Whether `approval` names `proposal` and carries a valid signature by `signerKey` over its approval message on
/// `vault`.
#[napi]
pub fn verify_proposal_approval(
    proposal: Proposal,
    approval: Approval,
    vault: String,
    signer_key: Point,
) -> Result<bool> {
    let approval = approval_to_wire(&approval)?;
    if schnorr::Signature::from_limbs(&limbs(&approval.signature.map(|l| hex32(&l)))?).is_err() {
        return Ok(false);
    }
    approval
        .verify(&to_wire(&proposal)?, base(&vault)?, &point(&signer_key)?)
        .map_err(invalid)
}

fn to_wire(proposal: &Proposal) -> Result<messages::Proposal> {
    let fee_method =
        u8::try_from(proposal.fee_method).map_err(|_| invalid("feeMethod must fit in a byte"))?;
    base(&proposal.outer_hash)?;
    base(&proposal.tx_nonce)?;
    Ok(messages::Proposal {
        config_version: proposal.config_version,
        outer_hash: bytes32(&proposal.outer_hash)?,
        expires_at: unix_seconds(proposal.expires_at)?,
        tx_nonce: bytes32(&proposal.tx_nonce)?,
        fee_method,
        payload: proposal.payload.to_vec(),
        summary: proposal.summary.clone(),
        proposer: proposal.proposer,
    })
}

fn from_wire(proposal: messages::Proposal) -> Result<Proposal> {
    Ok(Proposal {
        config_version: proposal.config_version,
        outer_hash: hex32(&proposal.outer_hash),
        expires_at: timestamp_out(proposal.expires_at)?,
        tx_nonce: hex32(&proposal.tx_nonce),
        fee_method: u32::from(proposal.fee_method),
        payload: proposal.payload.into(),
        summary: proposal.summary,
        proposer: proposal.proposer,
    })
}

fn approval_to_wire(approval: &Approval) -> Result<messages::Approval> {
    let parsed = limbs(&approval.signature)?;
    Ok(messages::Approval {
        proposal: bytes32(&approval.proposal)?,
        slot: approval.slot,
        signature: parsed.map(|limb| tecpan_wire::types::field(&limb)),
    })
}
