use aztec::{macros::notes::note, protocol::traits::{Deserialize, Packable, Serialize}};

/// Current authorization policy of the multisig.
///
/// The signer keys themselves live in the witness; this note commits to them via `signer_set_commitment`. `version`
/// increases on every config change and is bound into every approval, so approvals collected under an older config
/// stop verifying once it changes.
#[derive(Deserialize, Eq, Packable, Serialize)]
#[note]
pub struct ConfigNote {
    pub threshold: u32,
    pub signer_count: u32,
    pub version: u32,
    pub signer_set_commitment: Field,
}
