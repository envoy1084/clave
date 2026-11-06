/// Errors returned for invalid or adversarial input. Library code never panics on untrusted data.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Bytes or coordinates do not describe a point on Grumpkin.
    #[error("point is not on the Grumpkin curve")]
    PointNotOnCurve,
    /// The identity is never a valid key, share commitment or ECDH input.
    #[error("point is the identity")]
    IdentityPoint,
    /// A field element encoding is not canonical (not below the modulus).
    #[error("non-canonical field element encoding")]
    NonCanonicalEncoding,
    /// More members than the vault supports.
    #[error("too many members: at most {max}, got {actual}")]
    TooManyMembers {
        /// Maximum number of members.
        max: usize,
        /// Actual number.
        actual: usize,
    },
    /// Member indices start at one.
    #[error("invalid member index")]
    InvalidIndex,
    /// The same member index appears twice.
    #[error("duplicate member index {0}")]
    DuplicateIndex(u32),
    /// A threshold must be at least one and at most the number of members.
    #[error("invalid threshold")]
    InvalidThreshold,
    /// Fewer shares or partials than the threshold.
    #[error("insufficient shares: need {needed}, got {actual}")]
    InsufficientShares {
        /// Threshold.
        needed: usize,
        /// Provided.
        actual: usize,
    },
    /// Input has the wrong length.
    #[error("invalid length: expected {expected}, got {actual}")]
    InvalidLength {
        /// Expected length.
        expected: usize,
        /// Actual length.
        actual: usize,
    },
}
