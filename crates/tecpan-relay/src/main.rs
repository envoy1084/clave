//! `tecpan-relay`: untrusted libp2p circuit relay and ciphertext mailbox for Tecpan agents.
//!
//! Implemented in Phase 9; this binary is a placeholder until then.

#[expect(
    clippy::print_stdout,
    reason = "placeholder binary reports its version until Phase 9"
)]
fn main() {
    println!("tecpan-relay {}", env!("CARGO_PKG_VERSION"));
}
