#!/usr/bin/env bash
# Installs the clave CLI from this repository into $CLAVE_HOME (default ~/.clave):
#   bin/clave, bin/clave-keeper     release builds
#   lib/engine/                       the claved daemon with its production dependencies (Node 24 runs it)
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
home="${CLAVE_HOME:-$HOME/.clave}"

node_major="$(node -p 'process.versions.node.split(".")[0]' 2>/dev/null || echo 0)"
if [ "$node_major" -lt 24 ]; then
  echo "clave needs Node 24 or newer on PATH (found: $(node --version 2>/dev/null || echo none))." >&2
  exit 1
fi

echo "Building the CLI and keeper (release)…"
cargo build --release --manifest-path "$root/crates/Cargo.toml" -p clave-cli -p clave-keeper

echo "Building the daemon…"
pnpm --dir "$root" --filter @clave/engine... build

echo "Installing into ${home}…"
mkdir -p "$home/bin" "$home/lib"
chmod 700 "$home"
install -m 755 "$root/crates/target/release/clave" "$home/bin/clave"
install -m 755 "$root/crates/target/release/clave-keeper" "$home/bin/clave-keeper"
rm -rf "$home/lib/engine"
pnpm --dir "$root" --filter @clave/engine deploy --prod "$home/lib/engine" >/dev/null
ln -sf "dist/claved.js" "$home/lib/engine/claved.js"

echo
echo "Installed. Add the CLI to your PATH:"
echo "  export PATH=\"$home/bin:\$PATH\""
echo "Then run: clave doctor"
