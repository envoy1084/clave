//! `tecpan-agent` binary.

#[expect(
    clippy::print_stdout,
    reason = "placeholder binary reports its version until Phase 8"
)]
fn main() {
    println!("tecpan-agent {}", env!("CARGO_PKG_VERSION"));
}
