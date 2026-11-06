//! Helpers for loading the Aztec golden vectors (`tests/vectors/aztec.json`).
#![allow(
    dead_code,
    unreachable_pub,
    clippy::unwrap_used,
    clippy::missing_panics_doc
)]

use serde_json::Value;
use tecpan_crypto::{Base, Point, Scalar, curve};

pub fn vectors() -> Value {
    serde_json::from_str(include_str!("../vectors/aztec.json")).expect("valid vectors json")
}

pub fn bytes(hex_str: &str) -> Vec<u8> {
    hex::decode(hex_str.trim_start_matches("0x")).expect("hex")
}

pub fn base(value: &Value) -> Base {
    curve::base_from_bytes(&bytes(value.as_str().expect("string"))).expect("canonical base")
}

pub fn scalar(value: &Value) -> Scalar {
    curve::scalar_from_bytes(&bytes(value.as_str().expect("string"))).expect("canonical scalar")
}

pub fn point(value: &Value) -> Point {
    Point::from_xy(base(&value["x"]), base(&value["y"])).expect("on-curve point")
}

/// Asserts coordinates and both Aztec encodings.
pub fn assert_point(actual: &Point, expected: &Value) {
    assert_eq!(actual, &point(expected));
    assert_eq!(
        actual.to_bytes().to_vec(),
        bytes(expected["bytes"].as_str().unwrap())
    );
    assert_eq!(
        actual.to_compressed().to_vec(),
        bytes(expected["compressed"].as_str().unwrap())
    );
}
