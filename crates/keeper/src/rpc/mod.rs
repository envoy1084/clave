//! Local JSON-RPC 2.0 for the member's own engine: newline-delimited JSON over a Unix domain socket (named pipe on
//! Windows), owner-only, with a per-launch token presented through `session.auth` before anything else.

mod json;
mod lobby;
mod methods;
mod server;

pub use server::{RpcToken, serve};
