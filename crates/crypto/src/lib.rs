//! Threshold viewing-key cryptography for Tecpan vaults.
//!
//! Grumpkin arithmetic and encodings byte-compatible with Aztec 6.0.0, Aztec's Poseidon2, the vault contract's wire
//! formats, Shamir sharing with Feldman commitments, distributed key generation, threshold ECDH with DLEQ proofs,
//! committee resharing and authenticated envelopes. See `architecture/viewing-keys.md`.
//!
//! Secret scalars are [`SecretScalar`]: zeroized on drop, never printed, and multiplied with a constant-time ladder.

pub mod aztec;
mod ct;
pub mod curve;
pub mod dkg;
pub mod ecdh;
pub mod envelope;
mod error;
pub mod mailbox;
pub mod poseidon2;
pub mod reshare;
pub mod schnorr;
mod secret;
pub mod shamir;
pub mod vault;

pub use curve::{Base, Point, Scalar};
pub use error::Error;
pub use secret::SecretScalar;
