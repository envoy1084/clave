use subtle::{ConstantTimeEq, CtOption};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::Scalar;

/// A secret Grumpkin scalar (share, polynomial coefficient, nonce).
///
/// Zeroized on drop, compared in constant time, never printed (`Debug` is redacted), and only multiplied through the
/// constant-time ladder.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecretScalar(Scalar);

impl SecretScalar {
    /// Wraps a scalar that must be kept secret.
    #[must_use]
    pub fn new(value: Scalar) -> Self {
        Self(value)
    }

    /// Access to the raw value for arithmetic. Keep results secret too.
    #[must_use]
    pub fn expose(&self) -> &Scalar {
        &self.0
    }

    /// Returns the scalar if it is non-zero, in constant time.
    #[must_use]
    pub fn non_zero(self) -> CtOption<Self> {
        let is_zero = self.ct_eq(&Self(Scalar::from(0u64)));
        CtOption::new(self, !is_zero)
    }
}

impl ConstantTimeEq for SecretScalar {
    fn ct_eq(&self, other: &Self) -> subtle::Choice {
        let a = crate::curve::scalar_to_bytes(&self.0);
        let b = crate::curve::scalar_to_bytes(&other.0);
        a.ct_eq(&b)
    }
}

impl core::fmt::Debug for SecretScalar {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("SecretScalar(<redacted>)")
    }
}
