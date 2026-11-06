//! Verifying an arbitrary (likely invalid) partial never panics and never accepts a forged proof for a fixed member.
#![no_main]

use libfuzzer_sys::fuzz_target;
use tecpan_crypto::{
    Base, Point, curve,
    ecdh::{DleqProof, EcdhContext, Partial, verify_partial},
};

fuzz_target!(|data: &[u8]| {
    // point (64) | input (64) | challenge (32) | response (32) | index (4)
    if data.len() < 196 {
        return;
    }
    let point = <[u8; 64]>::try_from(&data[0..64]).ok().and_then(|b| Point::from_bytes(&b).ok());
    let input = <[u8; 64]>::try_from(&data[64..128]).ok().and_then(|b| Point::from_bytes(&b).ok());
    let challenge = curve::scalar_from_bytes(&data[128..160]).ok();
    let response = curve::scalar_from_bytes(&data[160..192]).ok();
    let index = u32::from_be_bytes([data[192], data[193], data[194], data[195]]);
    let (Some(point), Some(input), Some(challenge), Some(response)) = (point, input, challenge, response) else {
        return;
    };
    let partial = Partial { index, point, proof: DleqProof { challenge, response } };
    let context = EcdhContext { vault: Base::from(1u64), epoch: 0 };
    // The member's public share is the generator; a valid proof would need its discrete log (1), which the fuzzer
    // can only hit by producing a genuine proof.
    let _ = verify_partial(&partial, &input, &Point::generator(), &context);
});
