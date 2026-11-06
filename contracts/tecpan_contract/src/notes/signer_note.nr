use aztec::{macros::notes::note, protocol::traits::{Deserialize, Packable, Serialize}};

/// One signer key at a fixed slot of the signer set.
///
/// Signer notes let every co-signer's PXE rebuild the ordered signer set from chain data. Authorization never reads
/// them: it checks the witness against the commitment in the config note instead.
#[derive(Deserialize, Eq, Packable, Serialize)]
#[note]
pub struct SignerNote {
    pub x: Field,
    pub y: Field,
    pub slot: u32,
}
