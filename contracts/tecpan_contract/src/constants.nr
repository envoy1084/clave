//! Protocol constants shared by the contract and mirrored by the TypeScript SDK.

/// Maximum number of signers a multisig can hold.
///
/// Every authorization check iterates over all slots, so proving cost grows linearly with this value.
pub global MAX_SIGNERS: u32 = 8;

/// Number of fields used by one signer key (Grumpkin `x`, `y`).
pub global SIGNER_KEY_LEN: u32 = 2;

/// Number of fields used by one Schnorr signature (`s.lo`, `s.hi`, `e.lo`, `e.hi`).
pub global SIGNATURE_LEN: u32 = 4;

/// Length of the aggregated auth witness: the ordered signer keys followed by one signature slot per signer.
pub global WITNESS_LEN: u32 = MAX_SIGNERS * (SIGNER_KEY_LEN + SIGNATURE_LEN);

/// Seconds after the anchor block within which a transaction checked against the current config must be included.
///
/// Bounds how long a superseded config can still authorize transactions. The resulting expiration timestamp is public.
pub global CONFIG_MAX_AGE: u64 = 3600;

/// Domain separator for the message each signer signs: ASCII `tecpan.approval.v1`.
pub global DOM_SEP__APPROVAL: Field = 0x74656370616e2e617070726f76616c2e7631;

/// Domain separator for the signer set commitment: ASCII `tecpan.signers.v1`.
pub global DOM_SEP__SIGNER_SET: Field = 0x74656370616e2e7369676e6572732e7631;
