//! Boundary conversions: hex strings and `{ x, y }` objects to and from `tecpan-crypto` values.

use std::fmt::Display;

use napi::{Error, Result, Status};
use napi_derive::napi;
use tecpan_crypto::{Base, Scalar, curve};

/// A Grumpkin point in affine coordinates, each a `0x`-prefixed 32-byte hex field element.
#[napi(object)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Point {
    /// `x`.
    pub x: String,
    /// `y`.
    pub y: String,
}

pub(crate) fn invalid(error: impl Display) -> Error {
    Error::new(Status::InvalidArg, error.to_string())
}

pub(crate) fn bytes32(value: &str) -> Result<[u8; 32]> {
    let digits = value.strip_prefix("0x").unwrap_or(value);
    let mut out = [0u8; 32];
    hex::decode_to_slice(digits, &mut out)
        .map_err(|_| invalid(format!("expected 32-byte hex, got {value:?}")))?;
    Ok(out)
}

pub(crate) fn hex32(bytes: &[u8; 32]) -> String {
    format!("0x{}", hex::encode(bytes))
}

pub(crate) fn base(value: &str) -> Result<Base> {
    curve::base_from_bytes(&bytes32(value)?).map_err(invalid)
}

pub(crate) fn base_hex(value: &Base) -> String {
    hex32(&curve::base_to_bytes(value))
}

pub(crate) fn scalar(value: &str) -> Result<Scalar> {
    curve::scalar_from_bytes(&bytes32(value)?).map_err(invalid)
}

#[cfg(feature = "testing")]
pub(crate) fn scalar_hex(value: &Scalar) -> String {
    hex32(&curve::scalar_to_bytes(value))
}

pub(crate) fn point(value: &Point) -> Result<tecpan_crypto::Point> {
    tecpan_crypto::Point::from_xy(base(&value.x)?, base(&value.y)?).map_err(invalid)
}

pub(crate) fn points(values: &[Point]) -> Result<Vec<tecpan_crypto::Point>> {
    values.iter().map(point).collect()
}

pub(crate) fn point_out(value: &tecpan_crypto::Point) -> Point {
    Point {
        x: base_hex(&value.x()),
        y: base_hex(&value.y()),
    }
}

pub(crate) fn limbs(values: &[String]) -> Result<[Base; 4]> {
    match values {
        [a, b, c, d] => Ok([base(a)?, base(b)?, base(c)?, base(d)?]),
        _ => Err(invalid(format!(
            "expected 4 signature limbs, got {}",
            values.len()
        ))),
    }
}

pub(crate) fn unix_seconds(value: i64) -> Result<u64> {
    u64::try_from(value)
        .map_err(|_| invalid(format!("expected a non-negative timestamp, got {value}")))
}

pub(crate) fn timestamp_out(value: u64) -> Result<i64> {
    i64::try_from(value)
        .map_err(|_| invalid(format!("timestamp {value} does not fit in a JS integer")))
}
