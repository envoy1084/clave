/// Errors for malformed, oversized, unauthentic or replayed messages.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Encoded message is larger than [`crate::MAX_MESSAGE_BYTES`].
    #[error("message too large: {0} bytes")]
    TooLarge(usize),
    /// Bytes are not a valid encoding.
    #[error("malformed message")]
    Malformed,
    /// The message uses a wire version this build does not speak.
    #[error("unsupported wire version {0}")]
    UnsupportedVersion(u16),
    /// A field exceeds its bound (count or length).
    #[error("{field} out of bounds")]
    OutOfBounds {
        /// The offending field.
        field: &'static str,
    },
    /// The signature does not verify for the claimed signer.
    #[error("invalid signature")]
    InvalidSignature,
    /// The signer is not who the receiver expected.
    #[error("unexpected signer")]
    UnexpectedSigner,
    /// Message id already seen from this signer.
    #[error("replayed message")]
    Replayed,
    /// Message belongs to another epoch.
    #[error("wrong epoch: expected {expected}, got {actual}")]
    WrongEpoch {
        /// Receiver's current epoch.
        expected: u32,
        /// Message epoch.
        actual: u32,
    },
    /// Message timestamp is outside the accepted window.
    #[error("stale or future message")]
    Stale,
    /// A cryptographic value inside the message is invalid.
    #[error("invalid content: {0}")]
    Crypto(#[from] clave_crypto::Error),
}
