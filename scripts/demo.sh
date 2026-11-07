#!/usr/bin/env bash
# pnpm demo: builds what the demo needs and opens it (crates/demo): Alice and Bob side by side in this terminal,
# narrated step by step. Flags pass through: `pnpm demo --auto 6`, `pnpm demo --headless`.
# Needs `aztec start --local-network` on http://localhost:8080 (or AZTEC_NODE_URL).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cargo build -q --manifest-path "$root/crates/Cargo.toml" -p clave-cli -p clave-keeper -p clave-demo
[ -f "$root/packages/engine/dist/claved.js" ] || pnpm --dir "$root" --filter @clave/engine... build >/dev/null
exec "$root/crates/target/debug/clave-demo" "$@"
