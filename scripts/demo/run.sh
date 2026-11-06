#!/usr/bin/env bash
# Scripted two-member demo of a 2-of-2 Tecpan vault, driven from this terminal (the narrator).
# It opens one Ghostty window for Alice and one for Bob, then cues each step in the right window and pauses so you can
# talk. Passphrases and [y/N] confirmations are typed by you, in the member's window.
#
#   pnpm demo                  pause for Enter before each step
#   DEMO_GAP=8 pnpm demo       advance by itself, 8 s between steps
#   DEMO_UNATTENDED=1 pnpm demo   no windows or prompts (passphrase file, --yes): a smoke test of the script
#
# Needs `aztec start --local-network` on http://localhost:8080 (or AZTEC_NODE_URL) and Ghostty.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
node_url="${AZTEC_NODE_URL:-http://localhost:8080}"
demo="${TMPDIR:-/tmp}/tecpan-demo"
tecpan="$root/crates/target/debug/tecpan"
unattended="${DEMO_UNATTENDED:-}"
gap="${DEMO_GAP:-}"

bold=$'\033[1m' dim=$'\033[2m' green=$'\033[32m' yellow=$'\033[33m' red=$'\033[31m' reset=$'\033[0m'
t() { "$tecpan" "$@"; }
fail() {
  printf '%s\n' "${red}✗ $*${reset}" >&2
  exit 1
}
# A field of a JSON document on stdin: `field .profile.account` (numbers index arrays).
field() { node -e "let s='';process.stdin.on('data',d=>s+=d).on('end',()=>{let v=JSON.parse(s);for(const k of process.argv[1].split('.').filter(Boolean))v=v?.[k];console.log(v===undefined?'':typeof v==='string'?v:JSON.stringify(v))})" "$1"; }
short() { printf '%s…%s' "${1:0:6}" "${1: -4}"; }

# ── setup ────────────────────────────────────────────────────────────────────────────────────────────────────────────
stop_daemons() { for m in alice bob; do t -P "$m" daemon stop >/dev/null 2>&1 || true; done; }

printf '%s\n' "${bold}Tecpan demo${reset} ${dim}· preparing${reset}"
cargo build -q --manifest-path "$root/crates/Cargo.toml" -p tecpan-cli -p tecpan-keeper
[ -f "$root/packages/engine/dist/tecpand.js" ] || pnpm --dir "$root" --filter @tecpan/engine... build >/dev/null
curl -sf -m 5 -X POST "$node_url" -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"node_getBlockNumber","params":[]}' >/dev/null ||
  fail "no Aztec node at $node_url: start one with  aztec start --local-network"

export TECPAN_HOME="$demo/home"
export TECPAN_ENGINE="$root/packages/engine/dist/tecpand.js"
export TECPAN_KEEPER_BIN="$root/crates/target/debug/tecpan-keeper"
stop_daemons
rm -rf "$demo"
mkdir -p "$demo/q/alice" "$demo/q/bob"
# Windows from an earlier run see a different id and close themselves instead of running cues twice.
run_id="$$-$(date +%s)"
printf '%s' "$run_id" >"$demo/run"
yes_flag=""
if [ -n "$unattended" ]; then
  printf 'correct horse battery staple' >"$demo/passphrase"
  chmod 600 "$demo/passphrase"
  export TECPAN_PASSPHRASE_FILE="$demo/passphrase" NO_COLOR=1
  yes_flag=" -y"
fi
# Everything a member window needs; Ghostty starts it with launchd's PATH, not this shell's.
{
  printf 'export PATH=%q\n' "$(dirname "$tecpan"):$PATH"
  for v in TECPAN_HOME TECPAN_ENGINE TECPAN_KEEPER_BIN AZTEC_NODE_URL TECPAN_PASSPHRASE_FILE NO_COLOR DEMO_UNATTENDED; do
    [ -n "${!v:-}" ] && printf 'export %s=%q\n' "$v" "${!v}"
  done
} >"$demo/env"

# ── cues ─────────────────────────────────────────────────────────────────────────────────────────────────────────────
# Cues sent so far, per member (bash 3.2 has no associative arrays).
n_alice=0 n_bob=0
sent() {
  local var="n_$1"
  echo "${!var}"
}
next_cue() { printf '%s/q/%s/%03d.cmd' "$demo" "$1" "$(($(sent "$1") + 1))"; }

pids=()
finish() {
  for m in alice bob; do
    printf '__end__\n__end__\n' >"$demo/q/$m/999.cmd.tmp" 2>/dev/null && mv "$demo/q/$m/999.cmd.tmp" "$(next_cue "$m")" 2>/dev/null || true
  done
  if [ -n "$unattended" ]; then
    stop_daemons
    for pid in "${pids[@]}"; do wait "$pid" 2>/dev/null || true; done
    return
  fi
  printf '\n%s ' "Stop Alice's and Bob's daemons now? [Y/n]"
  read -r answer || answer=y
  case "$answer" in n | N) printf '%s\n' "${dim}Left running. Stop later: TECPAN_HOME=$TECPAN_HOME tecpan -P alice daemon stop${reset}" ;; *) stop_daemons ;; esac
}
trap finish EXIT

# Screen size in points (Retina panels report twice that).
screen_width() {
  local line w
  line="$(system_profiler SPDisplaysDataType 2>/dev/null | grep -m1 'Resolution:' || true)"
  w="$(printf '%s' "$line" | awk '{ print $2 }')"
  [ -n "$w" ] || {
    echo 1440
    return
  }
  case "$line" in *Retina*) w=$((w / 2)) ;; esac
  echo "$w"
}

open_member() {
  local member="$1" x="$2"
  if [ -n "$unattended" ]; then
    bash "$root/scripts/demo/actor.sh" "$member" "$demo" "$run_id" >"$demo/$member.log" 2>&1 &
    pids+=($!)
    return
  fi
  open -na Ghostty.app --args \
    --title="$member" --window-position-x="$x" --window-position-y=40 \
    --window-width=100 --window-height=42 --confirm-close-surface=false \
    -e /bin/bash "$root/scripts/demo/actor.sh" "$member" "$demo" "$run_id"
}

half=$(($(screen_width) / 2))
open_member alice 0
open_member bob "$half"

step_no=0
say() {
  step_no=$((step_no + 1))
  printf '\n%s\n' "${bold}${yellow}[$step_no]${reset} ${bold}$1${reset}"
  shift
  for line in "$@"; do printf '    %s\n' "$line"; done
}

pause() {
  if [ -n "$unattended" ]; then return; fi
  if [ -n "$gap" ]; then
    sleep "$gap"
  else
    printf '%s' "${dim}    ⏎ run${reset}"
    read -r _
  fi
}

# cue <member> <shown> [<run>]: queue a command in the member's window; prints the cue number.
cue() {
  local member="$1" shown="$2" run="${3:-$2}" file
  file="$(next_cue "$member")"
  printf -v "n_$member" '%s' "$(($(sent "$member") + 1))"
  printf '%s\n%s\n' "$shown" "$run" >"$file.tmp"
  mv "$file.tmp" "$file"
}

# wait_done <member>: waits for the member's latest cue; returns its exit code.
wait_done() {
  local member="$1" done_file code
  done_file="$(printf '%s/q/%s/%03d.done' "$demo" "$member" "$(sent "$member")")"
  until [ -f "$done_file" ]; do sleep 0.3; done
  code="$(cat "$done_file")"
  if [ "$code" = "0" ]; then printf '%s\n' "    ${green}✓ done${reset}"; else printf '%s\n' "    ${red}✗ exit $code${reset}"; fi
  return "$code"
}

# run <member> <shown> [<run>]: cue, then wait.
run() {
  cue "$@"
  wait_done "$1"
}

wait_for() {
  local what="$1" deadline=$(($(date +%s) + ${3:-180}))
  until eval "$2"; do
    [ "$(date +%s)" -lt "$deadline" ] || fail "timed out waiting for $what"
    sleep 1
  done
}

# ── the story ────────────────────────────────────────────────────────────────────────────────────────────────────────
[ -n "${DEMO_UNATTENDED:-}" ] || clear
printf '%s\n' "${bold}Tecpan: a private 2-of-2 vault on Aztec${reset}"
printf '%s\n' "${dim}Alice's window is on the left, Bob's on the right. Every command runs for real against the local network.${reset}"

say "Alice creates her wallet" \
  "Her signing key is generated and sealed under a passphrase only she knows." \
  "Her daemon (PXE) and keeper start. → type a passphrase twice in Alice's window."
pause
run alice "tecpan init --name Alice$yes_flag"

say "Bob creates his wallet" \
  "Same on Bob's machine: his own keys, his own keeper. → type a passphrase in Bob's window."
pause
run bob "tecpan init --name Bob$yes_flag"

say "Alice opens a 2-of-2 vault" \
  "k = 2: both must approve every spend. t_v = 2: both keepers are needed to read it." \
  "She gets an invite to send to Bob."
pause
cue alice "tecpan lobby open --name Treasury -k 2 --tv 2 --members 2 --alias treasury"
wait_for "Alice's invite" '[ -n "$(t -P alice --json lobby list 2>/dev/null | field .0.invite 2>/dev/null)" ]'
invite="$(t -P alice --json lobby list | field .0.invite)"
printf '    %s\n' "${dim}invite: $(short "$invite")${reset}"

say "Bob joins with the invite" \
  "He checks the plan (name, host, thresholds, network) and confirms. → type y in Bob's window." \
  "Then both machines run the founding: key generation (DKG), deriving the vault address, deploying it."
pause
cue bob "tecpan lobby join $(short "$invite") --alias treasury$yes_flag" "tecpan lobby join '$invite' --alias treasury$yes_flag"
wait_done alice
wait_done bob
vault="$(t -P alice --json vault list | field .0.address)"
printf '    %s\n' "${dim}vault: $vault${reset}"

say "Both see the same vault" \
  "Same address, same members, same on-chain config, verified independently on each machine."
pause
run alice "tecpan vault show"
run bob "tecpan vault show"

say "Fund the vault" \
  "On the local network: deploy a test token (TTT) and mint 10,000 privately to the vault."
pause
run alice "tecpan token deploy-test"
run alice "tecpan balance"
token="$(t -P alice --json token list | field .0.address)"

say "Bob reads the balance too" \
  "The vault's notes decrypt only with both keepers (threshold ECDH). Neither member can read it alone."
pause
run bob "tecpan token add $(short "$token")" "tecpan token add $token"
run bob "tecpan balance"

say "Alice proposes paying Bob 100 TTT" \
  "She saves Bob's account as a contact, then proposes. The proposal is replicated to Bob's keeper."
pause
bob_account="$(t -P bob --json status | field .profile.account)"
run alice "tecpan contacts add $(short "$bob_account") bob" "tecpan contacts add $bob_account bob"
run alice "tecpan propose transfer 100 ttt --to bob --memo 'Invoice 42'"
id="$(t -P alice --json proposals | field .0.id)"
id="${id:2:6}"

say "Alice approves" \
  "Her daemon rebuilds the transaction itself and shows exactly what she signs." \
  "→ in Alice's window: y, then her passphrase."
pause
run alice "tecpan approve $id$yes_flag"

say "Bob sees it and approves" \
  "Bob's daemon rebuilds the same transaction independently: if anything differed, it would refuse." \
  "→ in Bob's window: y, then his passphrase."
pause
run bob "tecpan proposals"
run bob "tecpan approve $id$yes_flag"

say "Bob executes" \
  "2 of 2 approvals: Bob's daemon builds the witness, proves and sends the transaction."
pause
run bob "tecpan execute $id"

say "Both verify" \
  "Each machine reads the result on its own: proposal executed, balance down by 100."
pause
run alice "tecpan proposal show $id"
run alice "tecpan balance"
run bob "tecpan balance"
run bob "tecpan activity"

printf '\n%s\n' "${bold}${green}Done.${reset} ${dim}Both windows now have a shell for questions.${reset}"
