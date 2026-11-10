#!/usr/bin/env bash
# The shared benchmark (docs/plans/roadmap.md): 3 reps per run at half length, off-screen, every run with a
# hard timeout. The default (small strips) takes about 8 min; base and huge add about 6 min each.
# Usage: bash tools/bench.sh [base|small|huge|strips]...   then: python -I tools/summarize.py
# Lines go to out/raw.jsonl, the rep log to out/reps.log. Build first: cargo build --release
cd "$(dirname "$0")/.."
P=target/release/technoape.exe
RAW=out/raw.jsonl
mkdir -p out
# one series at a time: two runs on one GPU spoil each other's numbers
mkdir out/.lock 2>/dev/null || { echo "another bench.sh is running (out/.lock)" >&2; exit 1; }
trap 'rmdir out/.lock' EXIT
tasklist | grep -qi '^technoape' && { echo "a technoape.exe is already running" >&2; exit 1; }
# a rep only counts if no compiler ran before, during (sampled every 3 s) or after it (ADR 0004);
# overlapped reps go to out/overlapped.jsonl and are repeated
busy() { tasklist | grep -qiE '^(rustc|cargo|cc1plus|cc1|link|ld)[^ ]*\.exe'; }
go() { # label run extra-args...
  local label=$1 run=$2 try; shift 2
  for try in 1 2 3 4; do
    local waited=0
    while busy && [ $waited -lt 1800 ]; do sleep 10; waited=$((waited + 10)); done
    rm -f out/.busy_seen out/.rep.jsonl
    busy && touch out/.busy_seen # gave up waiting after 30 min
    ( while sleep 3; do busy && touch out/.busy_seen; done ) & local sampler=$!
    timeout 170 $P run=$run label=$label length=0.5 out=out/.rep.jsonl "$@" 2>>out/bench.log >/dev/null || echo "FAILED $label $run $* (exit $?)" | tee -a out/bench.log >&2
    kill $sampler 2>/dev/null
    busy && touch out/.busy_seen
    if [ -e out/.busy_seen ]; then
      echo "$(date +%T) $label $run $* try=$try waited=${waited}s OVERLAP" | tee -a out/reps.log >&2
      cat out/.rep.jsonl >>out/overlapped.jsonl 2>/dev/null
    else
      echo "$(date +%T) $label $run $* try=$try waited=${waited}s clean" | tee -a out/reps.log >&2
      cat out/.rep.jsonl >>$RAW; break
    fi
  done
  rm -f out/.busy_seen out/.rep.jsonl
}
steps=${*:-small strips}
for s in $steps; do case $s in
base) # the benchmark planet (500 m class), comparable over time
  for i in 1 2 3; do for r in stand fly orbit edit; do go base $r planet=1536; done; done ;;
small) # the standard 4 km planet (ADR 0021) must hold the budget
  for i in 1 2 3; do for r in stand fly orbit edit; do go small $r planet=12288; done; done ;;
huge) # the huge 8 km planet (ADR 0021)
  for i in 1 2 3; do for r in stand fly orbit edit; do go huge $r planet=24576; done; done ;;
strips)
  go strip fly capture_ms=2500 sheet=out/fly_strip.png
  go strip orbit capture_ms=2500 sheet=out/orbit_strip.png ;;
esac; done
