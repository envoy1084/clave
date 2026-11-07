//! Decoding untrusted points and field elements never panics, and accepted encodings round-trip.
#![no_main]

use libfuzzer_sys::fuzz_target;
use clave_crypto::{Point, curve};

fuzz_target!(|data: &[u8]| {
    if let Ok(bytes) = <[u8; 64]>::try_from(data) {
        if let Ok(point) = Point::from_bytes(&bytes) {
            assert_eq!(point.to_bytes(), bytes);
        }
    }
    if let Ok(value) = curve::base_from_bytes(data) {
        assert_eq!(curve::base_to_bytes(&value).as_slice(), data);
    }
    if let Ok(value) = curve::scalar_from_bytes(data) {
        assert_eq!(curve::scalar_to_bytes(&value).as_slice(), data);
    }
});
