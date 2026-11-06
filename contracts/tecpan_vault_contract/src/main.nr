use aztec::macros::aztec;

/// Tecpan vault: private k-of-n account contract.
///
/// Placeholder until Phase 3 adds the signer set, viewing committee commitment and k-of-n authorization.
#[aztec]
pub contract TecpanVault {
    use aztec::macros::functions::{external, initializer};

    #[external("private")]
    #[initializer]
    fn constructor() {}
}
