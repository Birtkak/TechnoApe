#!/usr/bin/env bash
# The see-through spike's measured series (README.md): every line of a series file is "name | arguments",
# run once with quick=1 off-screen, one JSON line each into out/see-through/<series>.jsonl. Holds the shared
# lock around the whole series. Usage: bash prototypes/see-through/run.sh <series file> [reps]
set -u
cd "$(dirname "$0")/../.."
series="$1"
reps="${2:-1}"
# the main checkout's lock, shared by every worktree
lock="$(git rev-parse --path-format=absolute --git-common-dir)/../out/.lock"
mkdir -p out/see-through
out="out/see-through/$(basename "$series" .txt).jsonl"
until mkdir "$lock" 2>/dev/null; do echo "waiting for the lock" >&2; sleep 20; done
trap 'rmdir "$lock"' EXIT
for r in $(seq "$reps"); do
  grep -v '^#' "$series" | grep -v '^\s*$' | while IFS='|' read -r name args; do
    name=$(echo "$name" | xargs)
    line=$(timeout 400 target/release/technoape.exe quick=1 settings=out/see-through/settings.txt $args 2>"out/see-through/$name.err" | tail -1)
    echo "{\"name\":\"$name\",\"rep\":$r,\"r\":$line}" >> "$out"
    echo "$name rep $r done" >&2
  done
done
