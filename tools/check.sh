#!/usr/bin/env bash
# The one gate (docs/design/fundamentals.md section 4, ADR 0011): tests, a soak on a fresh seed, a replay of
# that soak (determinism and the flight recorder), the quick benchmark (budget in every run, no missing chunks
# in fly), the fixed viewpoints (tools/views.sh: cracks and pixels against the baselines) and a played run's
# feel report (ADR 0018). Prints one JSON verdict line, exit 1 on any failure; each failure carries its repro.
# Usage: bash tools/check.sh [soak_minutes]   (default 3; about 4 min in all after a build, most of it the soak
# and its replay, which run beside the tests and the viewpoints)
#        bash tools/check.sh fast            the same checks cut short for use after every change: a 1 minute
#        soak, the stand, fly and sim benchmarks only. The full gate stays what a handover needs.
#        bash tools/check.sh accept-perf     three quick runs of each benchmark run, their medians written to
#        tests/perf/baseline.json: for a change that is slower (or faster) on purpose; its commit says why.
# Every benchmark run is held to its baseline too (tools/perf.py, ADR 0025 K): a run worse than its tolerance
# is run twice more and judged on the median of the three.
# The verdict's "seconds" says where the time went.
cd "$(dirname "$0")/.."
export PATH="$HOME/Tools/rust/cargo/bin:$PATH" CARGO_HOME="${CARGO_HOME:-$HOME/Tools/rust/cargo}" RUSTUP_HOME="${RUSTUP_HOME:-$HOME/Tools/rust/rustup}"
mkdir -p out/check
# under `tools/slot.sh measure` the lock is this run's own
[ -z "${SLOT_HELD:-}" ] && [ -d out/.lock ] && { echo '{"ok":false,"error":"a benchmark is running (out/.lock)"}'; exit 1; }
FAST=; [ "$1" = fast ] && { FAST=1; shift; }
ALL="stand fly orbit edit sim fx"
if [ "$1" = accept-perf ]; then
  cargo build --profile play -p client -p server >out/check/build.log 2>&1 || { echo "play build failed: out/check/build.log" >&2; exit 1; }
  for i in 1 2 3; do for r in $ALL; do
    printf '%s\t%s\n' $r "$(timeout 170 target/play/technoape.exe run=$r quick=1 2>/dev/null | tail -1)"
  done; done | python -I tools/perf.py accept
  exit $?
fi
MIN=${1:-1}
RUNS=$([ $FAST ] && echo "stand fly sim" || echo "$ALL")
SECONDS=0
SEED=$(( $(date +%s) % 100000 ))
fails=()

# One optimised build, `play`: release without fat LTO, built in a fraction of the time (the developer's call,
# 2026-11-12: the fat-LTO link alone was 165 s of every gate). The gate's speed guard holds the play build to its
# own baseline; every performance number that is reported comes from the release build (tools/bench.sh).
# Everything that times nothing runs at once from the start:
# - the `play` build of the game and the server (the gate runs neither the Studio nor the bot's binary), then on
#   it the viewpoints, the soak and its replay. The soak runs its clock 32 times faster
#   (the bots' first says /timewarp, so the replay follows): a minute of ticks is 1920 s of the world, a whole
#   day and night and a full cycle of the tide, where three minutes unwarped were a tenth of a day (ADR 0067);
# - the tests, every test binary side by side (tools/partest.py).
# The benchmark and the played run measure frame times, so they wait until all of it is done.
rm -f out/check/soak.json out/check/replay.json out/check/views.json out/check/built
S=target/play/technoape-server.exe
( cargo build --profile play -p client -p server >out/check/build.log 2>&1 || exit 1
  touch out/check/built
  ( TECHNOAPE=target/play/technoape.exe bash tools/views.sh | tail -1 >out/check/views.json ) &
  timeout 600 $S soak=1 bots=50 minutes=$MIN warp=32 seed=$SEED record=out/check/soak.rec >out/check/soak.json
  timeout 600 $S replay=out/check/soak.rec >out/check/replay.json
  wait ) &
sim=$!

if python -I tools/partest.py out/check/test.log; then tests=ok; else
  tests=fail; fails+=("cargo test: $(grep -E '^test .* FAILED$' out/check/test.log | head -3 | sed 's/^test //;s/ \.\.\. FAILED//' | tr '\n' ' ')")
fi
t_tests=$SECONDS

wait $sim
views=$(cat out/check/views.json 2>/dev/null)
echo "$views" | grep -q '"ok": true' || fails+=("views: $(echo "$views" | python -I -c "import json,sys; print('; '.join(json.loads(sys.stdin.read())['failures']))" 2>/dev/null || echo "$views")")
t_views=$SECONDS

[ -f out/check/built ] || { echo '{"ok":false,"error":"play build failed: out/check/build.log"}'; exit 1; }
t_sim=$SECONDS
soak=$(tail -1 out/check/soak.json)
echo "$soak" | grep -q '"soak":"ok"' || fails+=("soak: $soak")
replay=$(tail -1 out/check/replay.json)
echo "$replay" | grep -q '"diverged":null' || fails+=("replay: $replay (repro: technoape-server replay=out/check/soak.rec)")

bench=()
for r in $RUNS; do
  line=$(timeout 170 target/play/technoape.exe run=$r quick=1 2>/dev/null | tail -1)
  bench+=("$line")
  echo "$line" | python -I -c "import json,sys; d=json.loads(sys.stdin.read()); sys.exit(0 if d['budget_ok'] and (d['run']!='fly' or d['missing_chunks_max']==0) and d.get('coarse_slow_s', 0) <= 1.0 else 1)" 2>/dev/null \
    || fails+=("bench $r: budget or streaming failed, or chunks coarse over 1 s with the camera slow (coarse_slow_s, ADR 0073) (repro: technoape.exe run=$r quick=1)")
  if ! echo "$line" | python -I tools/perf.py check $r >/dev/null; then
    more=$(for i in 1 2; do timeout 170 target/play/technoape.exe run=$r quick=1 2>/dev/null | tail -1; done)
    msg=$(printf '%s\n%s\n' "$line" "$more" | python -I tools/perf.py check $r) \
      || fails+=("perf $r: $msg, the median of three quick runs against tests/perf/baseline.json (repro: technoape.exe run=$r quick=1; slower on purpose: bash tools/check.sh accept-perf)")
  fi
done
t_bench=$SECONDS

# a played run: walk over hills, dig, place, build with the hammer. Clicks must show within a 60 Hz frame,
# and the server must never correct the prediction (ADR 0019)
# its own settings, a copy the game may write to: the developer's settings.txt must not decide the verdict.
# detail=8 loads for over a second, which is what showed the steps lost while loading (db30c50)
cp tests/gate-settings.txt out/check/settings.txt
ACTS="0.5:m:0:250,1:say:/playmode creative,1.2:say:/give stone,2:L,2.1:l,3:R,3.1:r,4:L,4.1:l,5:h,5.2:R,5.3:m:-100:0,5.6:r,7:L,7.1:l,8:R,8.1:r"
feel=$(MSYS_NO_PATHCONV=1 timeout 120 target/play/technoape.exe run=play save=0 settings=out/check/settings.txt seconds=10 autowalk=1 acts="$ACTS" 2>/dev/null | tail -1 \
  | python -I -c "import json,sys; print(json.dumps(json.loads(sys.stdin.read())['feel']))" 2>/dev/null)
echo "$feel" | python -I -c "import json,sys; f=json.loads(sys.stdin.read()); sys.exit(0 if f['corrections'] == 0 and f['clicks'] >= 4 and f['clicks_not_shown'] == 0 and f['click_to_change_ms_max'] <= 16.7 else 1)" 2>/dev/null \
  || fails+=("feel: $feel (repro: technoape.exe run=play save=0 settings=out/check/settings.txt seconds=10 autowalk=1 acts=\"$ACTS\")")

times="{\"tests\":$t_tests,\"soak_and_views_wait\":$((t_views-t_tests)),\"bench\":$((t_bench-t_sim)),\"feel\":$((SECONDS-t_bench)),\"total\":$SECONDS}"
printf '%s\n' "$tests" "$soak" "$replay" "$views" "$feel" "$times" "${bench[@]}" "--" "${fails[@]}" | python -I -c "
import json, sys
lines = sys.stdin.read().split('\n')
cut = lines.index('--')
tests, soak, replay, views, feel, times, bench, fails = lines[0], lines[1], lines[2], lines[3], lines[4], lines[5], lines[6:cut], [l for l in lines[cut + 1:] if l]
def js(s):
    try: return json.loads(s)
    except Exception: return s
b = {}
for l in bench:
    d = js(l)
    if isinstance(d, dict): b[d['run']] = {k: d[k] for k in ('budget_ok', 'fps_avg', 'frame_p50_ms', 'frame_p99_ms', 'frame_max_ms', 'missing_chunks_max')}
r = js(replay)
v, f = js(views), js(feel)
print(json.dumps({'ok': not fails, 'fast': '$FAST' == '1', 'seconds': js(times), 'tests': tests, 'soak': js(soak), 'replay': {k: r[k] for k in ('ticks', 'checks', 'diverged', 'hash')} if isinstance(r, dict) and 'checks' in r else r, 'bench': b,
    'views': v['views'] if isinstance(v, dict) else v,
    'feel': {k: f[k] for k in ('corrections', 'jerk_p99', 'jerk_max', 'clicks', 'click_to_change_frames_max', 'click_to_change_ms_max')} if isinstance(f, dict) else f,
    'failures': fails}))
sys.exit(1 if fails else 0)
"
