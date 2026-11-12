#!/usr/bin/env bash
# Claude Code Stop hook (.claude/settings.json): a session that edited code under crates/ may not end its
# turn while `cargo test` fails for the crates changed since the last pass and those that depend on them.
# Exit 2 sends the failures back to the session; exit 0 lets it stop.
# It stays out of the way of sessions that only wrote docs, of a running benchmark, and of a tree that
# already passed. Self-test: bash tools/stop-check.sh selftest
cd "$(dirname "$0")/.." || exit 0
export PATH="$HOME/Tools/rust/cargo/bin:$PATH" CARGO_HOME="${CARGO_HOME:-$HOME/Tools/rust/cargo}" RUSTUP_HOME="${RUSTUP_HOME:-$HOME/Tools/rust/rustup}"

# 0 = run the tests, 1 = skip. $1 is the hook's JSON input.
wanted() {
  # already sent back once this turn: never loop
  echo "$1" | grep -Eq '"stop_hook_active": *true' && return 1
  # only a session whose own transcript shows an edit under crates/ (the checkout is shared)
  local t
  t=$(echo "$1" | sed -n 's/.*"transcript_path": *"\([^"]*\)".*/\1/p' | sed 's/\\\\/\//g')
  [ -f "$t" ] && grep -Eq '"name":"(Edit|Write|MultiEdit)","input":\{"file_path":"[^"]*crates[/\\]' "$t" || return 1
  return 0
}

# Changed paths on stdin -> cargo test's crate arguments: each changed crate and every crate that depends on
# it (a `path = "../<crate>"` in its Cargo.toml, transitively); the root manifests -> the whole workspace.
pick() {
  local files sel prev c
  files=$(cat)
  echo "$files" | grep -Eq '^Cargo\.(toml|lock)$' && { echo --workspace; return; }
  sel=$(echo "$files" | sed -n 's#^crates/\([^/]*\)/.*#\1#p' | sort -u)
  while [ "$sel" != "$prev" ]; do
    prev=$sel
    for c in $prev; do sel+=$'\n'$(grep -l "path *= *\"\.\./$c\"" crates/*/Cargo.toml | cut -d/ -f2); done
    sel=$(echo "$sel" | grep . | sort -u)
  done
  echo $(for c in $sel; do echo "-p $c"; done)
}

if [ "$1" = selftest ]; then
  d=$(mktemp -d); trap 'rm -rf "$d"' EXIT
  echo '{"type":"tool_use","name":"Edit","input":{"file_path":"C:\\\\x\\\\crates\\\\engine\\\\src\\\\mesh.rs"}}' >"$d/code.jsonl"
  echo '{"type":"tool_use","name":"Edit","input":{"file_path":"C:\\\\x\\\\docs\\\\a.md","content":"about crates/engine"}}' >"$d/docs.jsonl"
  wanted "{\"transcript_path\":\"$d/code.jsonl\",\"stop_hook_active\":false}" || { echo "FAIL: a code session was skipped"; exit 1; }
  wanted "{\"transcript_path\":\"$d/docs.jsonl\",\"stop_hook_active\":false}" && { echo "FAIL: a docs session was checked"; exit 1; }
  wanted "{\"transcript_path\":\"$d/code.jsonl\",\"stop_hook_active\":true}" && { echo "FAIL: it would loop"; exit 1; }
  # $1 the changed paths, $2 the cargo test arguments they must pick
  sel() { local got; got=$(printf '%s\n' "$1" | pick); [ "$got" = "$2" ] || { echo "FAIL: '$1' picked '$got', not '$2'"; exit 1; }; echo "picks '$2' for '$1'"; }
  sel "crates/studio/src/main.rs" "-p studio"
  sel "crates/engine/src/mesh.rs" "-p bot -p client -p engine -p server -p studio"
  sel "crates/server/src/lib.rs" "-p bot -p client -p server -p studio"
  sel "Cargo.lock" "--workspace"
  sel "" ""
  echo "selftest ok"; exit 0
fi

wanted "$(cat)" || exit 0
# compiling would spoil a running benchmark's numbers (tools/bench.sh, tools/check.sh hold out/.lock)
[ -d out/.lock ] && exit 0

# the same code as the last pass needs no second run
sum=$({ git rev-parse HEAD:crates; git diff HEAD -- crates Cargo.toml Cargo.lock; git ls-files -o --exclude-standard -z crates | xargs -0 -r cat; } 2>/dev/null | sha1sum | cut -d' ' -f1)
stamp=target/.stop-check-ok
[ "$(head -1 $stamp 2>/dev/null)" = "$sum" ] && exit 0

# only the crates changed since the last pass (its commit is the stamp's second line) and their dependents;
# without a pass to compare against, the whole workspace
base=$(sed -n 2p $stamp 2>/dev/null)
if [ -n "$base" ] && git cat-file -e "$base^{commit}" 2>/dev/null; then
  args=$({ git diff --name-only "$base" -- crates Cargo.toml Cargo.lock; git ls-files -o --exclude-standard crates; } | pick)
else
  args=--workspace
fi
pass() { printf '%s\n%s\n' "$sum" "$(git rev-parse HEAD)" >$stamp; exit 0; }
[ -z "$args" ] && pass

mkdir -p out
if timeout 280 cargo test $args >out/stop-check.log 2>&1; then pass; fi
{ echo "cargo test $args fails (out/stop-check.log). Fix it, or say plainly why it cannot pass, before ending the turn:"
  grep -E '^test .* FAILED$|^error|panicked at' out/stop-check.log | head -15; } >&2
exit 2
