//! Node-API bindings exposing `tecpan-crypto` and `tecpan-wire` to the vault wallet and desktop app.
//!
//! Only public-data operations are exposed: vault formats, address derivation, Schnorr verification, threshold ECDH
//! verification and combination, and the proposal / approval codecs. Field elements and scalars cross the boundary
//! as `0x`-prefixed 32-byte big-endian hex, points as `{ x, y }`. Helpers that handle secret shares exist only behind
//! the `testing` feature.
//!
//! Every binding validates its input (canonical encodings, points on the curve, bounds) and throws an `InvalidArg`
//! error on failure. See `architecture/wire.md`.

pub mod address;
mod convert;
pub mod ecdh;
#[cfg(feature = "testing")]
pub mod testing;
pub mod vault;
pub mod wire;

pub use convert::Point;
