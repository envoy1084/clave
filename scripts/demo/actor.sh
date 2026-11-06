#!/usr/bin/env bash
# One member's terminal in the scripted demo (scripts/demo/run.sh). It waits for the director's cues, "types" each
# command, runs it in this real terminal (so passphrase prompts and [y/N] confirmations are answered here), and
# reports the exit code. When the demo ends it leaves an interactive shell for questions.
#   actor.sh <member> <demo-dir>
set -uo pipefail

member="$1"
demo="$2"
queue="$demo/q/$member"
source "$demo/env"
export TECPAN_PROFILE="$member"
name="$(printf '%s' "$member" | awk '{ print toupper(substr($0, 1, 1)) substr($0, 2) }')"

bold=$'\033[1m' dim=$'\033[2m' red=$'\033[31m' cyan=$'\033[36m' reset=$'\033[0m'
printf '\033]0;%s\007' "$name"
[ -n "${DEMO_UNATTENDED:-}" ] || clear
printf '%s%s%s\n%s\n\n' "$bold" "$name" "$reset" "${dim}Member terminal · waiting for the narrator…${reset}"

# Types `text` at a readable pace, faster for long lines.
type_out() {
  local text="$1" delay=0.03 i
  [ "${#text}" -gt 60 ] && delay=0.012
  [ -n "${DEMO_UNATTENDED:-}" ] && delay=0
  for ((i = 0; i < ${#text}; i++)); do
    printf '%s' "${text:i:1}"
    sleep "$delay"
  done
  printf '\n'
}

n=1
while true; do
  cue="$(printf '%s/%03d.cmd' "$queue" "$n")"
  until [ -f "$cue" ]; do sleep 0.2; done
  { IFS= read -r shown; IFS= read -r run; } <"$cue"
  if [ "$run" = "__end__" ]; then
    printf '\n%s\n' "${dim}Demo finished. This is $name's shell: try tecpan status, tecpan proposals, tecpan balance.${reset}"
    [ -n "${DEMO_UNATTENDED:-}" ] && exit 0
    BASH_SILENCE_DEPRECATION_WARNING=1 PS1="${cyan}${member} \$${reset} " exec bash --norc -i
  fi
  printf '%s ' "${cyan}${member} \$${reset}"
  type_out "$shown"
  eval "$run"
  code=$?
  [ "$code" -ne 0 ] && printf '%s\n' "${red}(exit $code)${reset}"
  printf '\n'
  printf '%s' "$code" >"$(printf '%s/%03d.done' "$queue" "$n")"
  n=$((n + 1))
done
