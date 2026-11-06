#!/usr/bin/env bash
# Network-namespace test: three keepers in isolated networks run a DKG, serve threshold ECDH and sync a proposal,
# talking only through a relay. Requires Docker with Compose. Exits non-zero on failure.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
crates="$(cd "$here/../../.." && pwd)"
compose=(docker compose -f "$here/compose.yml")

docker build --target test -f "$crates/deploy/Dockerfile" -t tecpan-node:test "$crates"

cleanup() { "${compose[@]}" --profile driver down -v --remove-orphans >/dev/null 2>&1 || true; }
trap cleanup EXIT
cleanup

RELAY_PEER="$("${compose[@]}" run --rm --no-deps relay tecpan-relay peer-id --data-dir /data | tr -d '"\r')"
export RELAY_PEER
echo "relay peer id: $RELAY_PEER"

"${compose[@]}" up -d relay keeper1 keeper2 keeper3
if ! "${compose[@]}" --profile driver run --rm driver; then
  "${compose[@]}" logs --no-color --tail 80 relay keeper1 keeper2 keeper3
  exit 1
fi
