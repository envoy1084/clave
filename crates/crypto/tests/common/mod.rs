//! Helpers for loading the Aztec golden vectors (`tests/vectors/aztec.json`).
#![allow(
    dead_code,
    unreachable_pub,
    clippy::unwrap_used,
    clippy::missing_panics_doc
)]

use clave_crypto::{Base, Point, Scalar, curve};
use serde_json::Value;

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

/// Runs a full DKG for members `1..=n` with threshold `t`.
pub fn run_dkg(
    t: usize,
    n: u32,
    rng: &mut rand_chacha::ChaCha20Rng,
) -> Vec<clave_crypto::dkg::KeyShare> {
    use clave_crypto::dkg;
    let params = dkg::Parameters {
        session: [7u8; 32],
        threshold: t,
        participants: (1..=n).collect(),
    };
    let (secrets, round1): (Vec<_>, Vec<_>) = params
        .participants
        .iter()
        .map(|&i| dkg::round1(&params, i, rng).unwrap())
        .unzip();
    let round2: Vec<_> = secrets
        .iter()
        .flat_map(|s| dkg::round2(&params, s, &round1).unwrap())
        .collect();
    secrets
        .into_iter()
        .map(|s| dkg::finalize(&params, s, &round1, &round2).unwrap())
        .collect()
}
