pub mod committee;
pub mod constants;
pub mod point;
pub mod signer_set;
pub mod witness;

use aztec::macros::aztec;

/// Tecpan vault: private k-of-n account contract.
///
/// Placeholder entry points until the vault storage and authorization land later in Phase 3.
#[aztec]
pub contract TecpanVault {
    use aztec::macros::functions::{external, initializer};

    #[external("private")]
    #[initializer]
    fn constructor() {}
}
