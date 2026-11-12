#!/usr/bin/env bash
# Step 2's measured series (ADR 0069): waits until no compiler runs on the PC, then today's build and this one,
# 3 reps each, logging any compiler or other game process seen during the series to out/see-through/busy.log.
# Usage: TODAY=<path to the build before step 2> bash prototypes/see-through/step2.sh
set -u
cd "$(dirname "$0")/../.."
busy() { tasklist 2>/dev/null | grep -iE "rustc|cargo|technoape-server|technoape-studio" | awk '{print $1}' | sort -u | tr '\n' ' '; }
while [ -n "$(busy)" ]; do echo "waiting: $(busy)" >&2; sleep 30; done
( while true; do b=$(busy); [ -n "$b" ] && echo "$(date +%T) $b"; sleep 10; done ) > out/see-through/busy.log &
watch=$!
trap 'kill $watch' EXIT
rm -f out/see-through/today.jsonl out/see-through/step2.jsonl
BIN="$TODAY" bash prototypes/see-through/run.sh prototypes/see-through/today.txt 3
bash prototypes/see-through/run.sh prototypes/see-through/step2.txt 3
for f in fancy fast; do
  target/release/technoape.exe run=meshbench at=1125.34,3634.37,1915.81 leaves=$f 2>/dev/null | tail -1 > out/see-through/meshbench-$f.json
done
"$TODAY" run=meshbench at=1125.34,3634.37,1915.81 2>/dev/null | tail -1 > out/see-through/meshbench-today.json
echo done
