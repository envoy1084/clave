//! Opening arbitrary envelopes never panics.
#![no_main]

use libfuzzer_sys::fuzz_target;
use clave_crypto::envelope::{self, Context, Envelope, TransportPublic, TransportSecret};

fuzz_target!(|data: &[u8]| {
    if data.len() < 32 {
        return;
    }
    let recipient = TransportSecret::from_bytes([7u8; 32]);
    let sender = TransportPublic([9u8; 32]);
    let mut encapsulated_key = [0u8; 32];
    encapsulated_key.copy_from_slice(&data[..32]);
    let envelope = Envelope { encapsulated_key, ciphertext: data[32..].to_vec() };
    let context = Context { session: [0u8; 32], epoch: 0, kind: 0, sender: 1, recipient: 2 };
    let _ = envelope::open(&context, &recipient, &sender, &envelope);
    let _ = envelope::open_scalar(&context, &recipient, &sender, &envelope);
});
