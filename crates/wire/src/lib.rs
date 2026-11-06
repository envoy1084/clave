//! Canonical, versioned, signed wire format for messages exchanged between Tecpan engines, keepers and relays.
//!
//! Every message travels as a [`SignedMessage`]: a [`Header`] (scope, epoch, id, timestamp) and a [`Message`] body,
//! signed by the sending keeper's Ed25519 identity over its canonical `postcard` encoding. Decoding enforces size
//! limits, the wire version and per-message bounds; [`ReplayGuard`] rejects duplicates, other epochs and stale
//! timestamps. See `architecture/wire.md`.

mod codec;
mod error;
mod identity;
pub mod lobby;
pub mod mailbox;
pub mod messages;
#[cfg(feature = "p2p")]
pub mod p2p;
mod replay;
mod signed;
pub mod types;

pub use codec::{MAX_MESSAGE_BYTES, WIRE_VERSION, decode, decode_body, encode, encode_body};
pub use error::Error;
pub use identity::{IdentityPublic, KeeperIdentity};
pub use messages::Message;
pub use replay::ReplayGuard;
pub use signed::{Header, SignedMessage};
