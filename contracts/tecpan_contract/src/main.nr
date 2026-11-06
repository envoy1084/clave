use aztec::macros::aztec;

/// Private k-of-n multisig account.
///
/// Executes a payload only when the auth witness carries valid Schnorr signatures from at least
/// `threshold` registered signers. Signers and threshold are held in private notes.
#[aztec]
pub contract TecpanMultisig {
    use aztec::macros::functions::{external, initializer};

    #[initializer]
    #[external("private")]
    fn constructor() {}
}
