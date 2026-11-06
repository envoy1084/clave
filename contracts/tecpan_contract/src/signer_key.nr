use aztec::protocol::traits::{Deserialize, Serialize};
use std::embedded_curve_ops::EmbeddedCurvePoint;

/// Grumpkin public key of a signer. The all-zero key marks an empty slot.
#[derive(Deserialize, Eq, Serialize)]
pub struct SignerKey {
    pub x: Field,
    pub y: Field,
}

impl SignerKey {
    pub fn empty() -> Self {
        Self { x: 0, y: 0 }
    }

    pub fn is_empty(self) -> bool {
        (self.x == 0) & (self.y == 0)
    }

    /// Whether the key lies on Grumpkin (`y^2 = x^3 - 17`).
    pub fn is_on_curve(self) -> bool {
        self.y * self.y == self.x * self.x * self.x - 17
    }

    pub fn to_point(self) -> EmbeddedCurvePoint {
        EmbeddedCurvePoint { x: self.x, y: self.y }
    }
}

mod test {
    use super::SignerKey;
    use std::embedded_curve_ops::EmbeddedCurvePoint;

    #[test]
    fn empty_key_is_empty_and_off_curve() {
        let key = SignerKey::empty();
        assert(key.is_empty());
        assert(!key.is_on_curve());
    }

    #[test]
    fn generator_is_on_curve() {
        let g = EmbeddedCurvePoint::generator();
        let key = SignerKey { x: g.x, y: g.y };
        assert(!key.is_empty());
        assert(key.is_on_curve());
    }
}
