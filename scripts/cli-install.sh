#!/usr/bin/env bash
# Installs the tecpan CLI from this repository into $TECPAN_HOME (default ~/.tecpan):
#   bin/tecpan, bin/tecpan-keeper     release builds
#   lib/engine/                       the tecpand daemon with its production dependencies (Node 24 runs it)
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
home="${TECPAN_HOME:-$HOME/.tecpan}"

node_major="$(node -p 'process.versions.node.split(".")[0]' 2>/dev/null || echo 0)"
if [ "$node_major" -lt 24 ]; then
  echo "tecpan needs Node 24 or newer on PATH (found: $(node --version 2>/dev/null || echo none))." >&2
  exit 1
fi

echo "Building the CLI and keeper (release)…"
cargo build --release --manifest-path "$root/crates/Cargo.toml" -p tecpan-cli -p tecpan-keeper

echo "Building the daemon…"
pnpm --dir "$root" --filter @tecpan/engine... build

echo "Installing into ${home}…"
mkdir -p "$home/bin" "$home/lib"
chmod 700 "$home"
install -m 755 "$root/crates/target/release/tecpan" "$home/bin/tecpan"
install -m 755 "$root/crates/target/release/tecpan-keeper" "$home/bin/tecpan-keeper"
rm -rf "$home/lib/engine"
pnpm --dir "$root" --filter @tecpan/engine deploy --prod "$home/lib/engine" >/dev/null
ln -sf "dist/tecpand.js" "$home/lib/engine/tecpand.js"

echo
echo "Installed. Add the CLI to your PATH:"
echo "  export PATH=\"$home/bin:\$PATH\""
echo "Then run: tecpan doctor"
