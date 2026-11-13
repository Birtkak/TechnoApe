#!/usr/bin/env bash
# Sharing the PC between sessions (ADR 0096). Builds, tests and pictures may run two at a time; anything
# whose speed numbers are kept runs alone.
#   bash tools/slot.sh job <command...>      a build, a test run, a shot or film: one of two slots
#   bash tools/slot.sh measure <command...>  a speed run or soak whose numbers are kept: the PC to itself
#   bash tools/slot.sh quiet <command...>    waits for a quiet PC, then runs a tool that takes out/.lock
#                                            itself (tools/check.sh, tools/bench.sh)
# The slots are folders in the main checkout's out/, shared by every worktree. A slot older than its limit
# is taken over: its owner died.
set -u
mode=${1:?job, measure or quiet}; shift
out="$(cd "$(git rev-parse --git-common-dir)/.." && pwd)/out"
mkdir -p "$out"
who="$(basename "$(git rev-parse --show-toplevel)"): $*"

stale() { # folder, minutes
  [ -d "$1" ] && [ -n "$(find "$1" -maxdepth 0 -mmin +"$2" 2>/dev/null)" ]
}
take() { # folder, minutes -> 0 when taken
  stale "$1" "$2" && rm -rf "$1"
  mkdir "$1" 2>/dev/null && echo "$who" > "$1/who"
}
jobs_busy() {
  for s in "$out/.job1" "$out/.job2"; do
    stale "$s" 30 && rm -rf "$s"
    [ -d "$s" ] && return 0
  done
  return 1
}

case $mode in
job)
  held=
  until [ -n "$held" ]; do
    if [ ! -d "$out/.lock" ] || stale "$out/.lock" 45; then
      for s in "$out/.job1" "$out/.job2"; do
        if take "$s" 30; then held=$s; break; fi
      done
    fi
    [ -n "$held" ] || sleep 5
  done
  trap 'rm -rf "$held"' EXIT
  "$@"
  ;;
measure)
  until take "$out/.lock" 45; do sleep 5; done
  trap 'rm -rf "$out/.lock"' EXIT
  while jobs_busy; do sleep 5; done
  "$@"
  ;;
quiet)
  while { [ -d "$out/.lock" ] && ! stale "$out/.lock" 45; } || jobs_busy; do sleep 5; done
  stale "$out/.lock" 45 && rm -rf "$out/.lock"
  "$@"
  ;;
*) echo "slot.sh: job, measure or quiet" >&2; exit 2 ;;
esac
