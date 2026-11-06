//! Tecpan keeper: the member daemon that holds one viewing share per vault and never lets it out. It answers threshold
//! ECDH requests with DLEQ-proven partials (for its owner's engine and for other members), runs the DKG and reshare
//! ceremonies, keeps the proposal store, and activates or destroys shares as the vault's on-chain committee changes.
//! See `architecture/keeper.md`.

mod clock;
pub mod custody;
mod error;
mod random;
pub mod store;

pub use clock::{Clock, SystemClock};
pub use error::{Error, Result};
