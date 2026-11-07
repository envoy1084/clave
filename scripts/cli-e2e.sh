#!/usr/bin/env bash
# Phase 13 end to end: three members (ana, bea, cai) on one machine operate a 2-of-3 vault only through `clave`,
# against `aztec start --local-network` on http://localhost:8080 (or AZTEC_NODE_URL):
#   init → lobby and founding → test token → propose → reject → approve → offline catch-up → execute
#   → blocked by rejections → cancel, and a check that --yes never skips authentication.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
node_url="${AZTEC_NODE_URL:-http://localhost:8080}"
work="$(mktemp -d "${TMPDIR:-/tmp}/clave-e2e.XXXXXX")"
export CLAVE_HOME="$work/home" NO_COLOR=1
export CLAVE_ENGINE="$root/packages/engine/dist/claved.js"
export CLAVE_KEEPER_BIN="$root/crates/target/debug/clave-keeper"
passphrase="$work/passphrase"
printf 'correct horse battery staple' >"$passphrase"
chmod 600 "$passphrase"
export CLAVE_PASSPHRASE_FILE="$passphrase"
clave="$root/crates/target/debug/clave"
started=$(date +%s)

step() { printf '\n[%4ss] %s\n' "$(($(date +%s) - started))" "$*"; }
fail() {
  echo "FAILED: $*" >&2
  exit 1
}
t() { "$clave" "$@"; }
# A field of a JSON document on stdin: `field .profile.account` (numbers index arrays).
field() { node -e "let s='';process.stdin.on('data',d=>s+=d).on('end',()=>{let v=JSON.parse(s);for(const k of process.argv[1].split('.').filter(Boolean))v=v?.[k];console.log(typeof v==='string'?v:JSON.stringify(v))})" "$1"; }
# Status of proposal $2 as profile $1 sees it.
status_of() { t -P "$1" --json proposal show "$2" 2>/dev/null | field .status || true; }
wait_for() {
  local what="$1" deadline=$(($(date +%s) + ${3:-300}))
  until eval "$2"; do
    [ "$(date +%s)" -lt "$deadline" ] || fail "timed out waiting for $what"
    sleep 1
  done
}
cleanup() {
  for p in ana bea cai; do t -P "$p" daemon stop >/dev/null 2>&1 || true; done
  if [ "${KEEP:-}" = "1" ]; then echo "kept $work"; else rm -rf "$work"; fi
}
trap cleanup EXIT

step "build"
cargo build -q --manifest-path "$root/crates/Cargo.toml" -p clave-cli -p clave-keeper
pnpm --dir "$root" --filter @clave/engine... build >/dev/null
curl -sf -m 5 -X POST "$node_url" -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"node_getBlockNumber","params":[]}' >/dev/null ||
  fail "no Aztec node at $node_url (aztec start --local-network)"

step "init three members"
for p in ana bea cai; do t -P "$p" -n "$node_url" -y init --name "$p" >/dev/null; done
t -P ana doctor
t -P ana status

step "found a 2-of-3 vault (t_v 2)"
t -P ana lobby open --name Ops -k 2 --tv 2 --members 3 --alias ops >"$work/ana-lobby.log" 2>&1 &
host=$!
wait_for "the invite" "grep -qo 'clave-invite-v0:[A-Za-z0-9_-]*' '$work/ana-lobby.log'" 120
invite="$(grep -o 'clave-invite-v0:[A-Za-z0-9_-]*' "$work/ana-lobby.log" | head -1)"
t -P bea lobby inspect "$invite"
t -P bea -y lobby join "$invite" --alias ops >"$work/bea-lobby.log" 2>&1 &
bea_join=$!
t -P cai -y lobby join "$invite" --alias ops >"$work/cai-lobby.log" 2>&1 &
cai_join=$!
wait "$host" || {
  cat "$work/ana-lobby.log"
  fail "founding (ana)"
}
wait "$bea_join" || {
  cat "$work/bea-lobby.log"
  fail "founding (bea)"
}
wait "$cai_join" || {
  cat "$work/cai-lobby.log"
  fail "founding (cai)"
}
cat "$work/ana-lobby.log"
vault="$(t -P ana --json vault list | field .0.address)"
for p in bea cai; do
  [ "$(t -P "$p" --json vault list | field .0.address)" = "$vault" ] || fail "$p founded a different vault"
done
t -P cai vault show

step "test token and balances"
token="$(t -P ana --json token deploy-test | field .address)"
t -P ana balance
bea_account="$(t -P bea --json status | field .profile.account)"
t -P ana contacts add "$bea_account" bea
t -P bea token add "$token" >/dev/null
t -P cai token add "$token" >/dev/null

step "propose 25 TTT to bea"
t -P ana simulate transfer 25 ttt --to bea
id="$(t -P ana --json propose transfer 25 ttt --to bea --memo "Q3 retainer" | field .id)"
short="${id:2:8}"
echo "proposal $short"
wait_for "replication" "[ -n \"\$(status_of cai $short)\" ] && [ -n \"\$(status_of bea $short)\" ]" 120
t -P bea proposals

step "cai rejects; bea and ana approve (clear-signing screen)"
t -P cai -y reject "$short" --reason "check the amount"
t -P cai daemon stop
t -P bea -y approve "$short"
t -P ana -y approve "$short"
wait_for "ready" "[ \"\$(status_of bea $short)\" = ready ]" 60
t -P bea proposal show "$short"

step "--yes never skips authentication"
second="$(t -P ana --json propose transfer 1 ttt --to bea | field .id)"
wait_for "replication" "[ -n \"\$(status_of bea ${second:2:8})\" ]" 120
set +e
CLAVE_PASSPHRASE_FILE='' t -P bea --json -y approve "${second:2:8}" </dev/null >/dev/null 2>"$work/locked.err"
code=$?
set -e
[ "$code" -ne 0 ] || fail "approve without a passphrase succeeded"
grep -q locked "$work/locked.err" || fail "expected a locked error, got: $(cat "$work/locked.err")"
[ "$(t -P bea --json proposal show "${second:2:8}" | field .approvedByMe)" = "false" ] || fail "bea approved without authenticating"
echo "refused: $(cat "$work/locked.err")"

step "execute while cai is offline"
t -P bea execute "$short"
wait_for "executed (ana)" "[ \"\$(status_of ana $short)\" = executed ]" 120

step "cai comes back and catches up"
t -P cai daemon start
wait_for "executed (cai)" "[ \"\$(status_of cai $short)\" = executed ]" 300
t -P cai activity
t -P cai balance

step "blocked by rejections, then cancelled"
wait_for "replication" "[ -n \"\$(status_of cai ${second:2:8})\" ]" 120
t -P bea -y reject "${second:2:8}" --reason "not this month"
t -P cai -y reject "${second:2:8}"
wait_for "blocked" "[ \"\$(status_of ana ${second:2:8})\" = blocked ]" 60
cancel="$(t -P ana --json -y cancel "${second:2:8}" | field .id)"
wait_for "replication" "[ -n \"\$(status_of bea ${cancel:2:8})\" ]" 120
t -P ana -y approve "${cancel:2:8}"
t -P bea -y approve "${cancel:2:8}"
t -P ana execute "${cancel:2:8}"
wait_for "cancelled" "[ \"\$(status_of cai ${second:2:8})\" = cancelled ]" 120
t -P ana proposals --status all

step "passed in $(($(date +%s) - started)) s"
