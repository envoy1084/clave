use aztec::macros::aztec;

/// Test-only helper that hands back Schnorr signatures produced by the TXE.
///
/// `TestEnvironment` can sign a message with a test account's key (`add_authwit`) but only exposes the result to
/// private execution. Calling `signature` privately returns those limbs so tests can assemble multi-signer witnesses.
/// Never deploy this contract outside tests.
#[aztec]
pub contract TestSigner {
    use aztec::{macros::functions::external, oracle::auth_witness::get_auth_witness};

    #[external("private")]
    fn signature(message: Field) -> [Field; 4] {
        // Safety: test-only helper; the returned limbs are verified by the contract under test.
        unsafe {
            get_auth_witness(message)
        }
    }
}
