#!/usr/bin/env bash
# The one gate (docs/design/fundamentals.md section 4, ADR 0011): tests, a soak on a fresh seed, a replay of
# that soak (determinism and the flight recorder), the quick benchmark (budget in every run, no missing chunks
# in fly), the fixed viewpoints (tools/views.sh: cracks and pixels against the baselines) and a played run's
# feel report (ADR 0018). Prints one JSON verdict line, exit 1 on any failure; each failure carries its repro.
# Usage: bash tools/check.sh [soak_minutes]   (default 3; about 4 min in all after a build, most of it the soak
# and its replay, which run beside the tests and the viewpoints)
#        bash tools/check.sh fast            the same checks cut short for use after every change: a 1 minute
#        soak, the stand and fly benchmarks only. The full gate stays what a handover needs.
# The verdict's "seconds" says where the time went.
cd "$(dirname "$0")/.."
export PATH="$HOME/Tools/rust/cargo/bin:$PATH" CARGO_HOME="${CARGO_HOME:-$HOME/Tools/rust/cargo}" RUSTUP_HOME="${RUSTUP_HOME:-$HOME/Tools/rust/rustup}"
mkdir -p out/check
[ -d out/.lock ] && { echo '{"ok":false,"error":"a benchmark is running (out/.lock)"}'; exit 1; }
FAST=; [ "$1" = fast ] && { FAST=1; shift; }
MIN=${1:-$([ $FAST ] && echo 1 || echo 3)}
RUNS=$([ $FAST ] && echo "stand fly" || echo "stand fly orbit edit")
SECONDS=0
SEED=$(( $(date +%s) % 100000 ))
fails=()

cargo build --release >out/check/build.log 2>&1 || { echo '{"ok":false,"error":"release build failed: out/check/build.log"}'; exit 1; }
t_build=$SECONDS

# the soak and its replay (CPU, timing nothing) run beside the tests and the viewpoints (no timing either);
# the benchmark and the played run measure frame times, so they wait until both are done
S=target/release/technoape-server.exe
( timeout 600 $S soak=1 bots=50 minutes=$MIN seed=$SEED record=out/check/soak.rec >out/check/soak.json
  timeout 600 $S replay=out/check/soak.rec >out/check/replay.json ) &
sim=$!

if cargo test --workspace >out/check/test.log 2>&1; then tests=ok; else
  tests=fail; fails+=("cargo test: $(grep -E '^test .* FAILED$' out/check/test.log | head -3 | sed 's/^test //;s/ \.\.\. FAILED//' | tr '\n' ' ')")
fi
t_tests=$SECONDS

views=$(bash tools/views.sh | tail -1)
echo "$views" | grep -q '"ok": true' || fails+=("views: $(echo "$views" | python -I -c "import json,sys; print('; '.join(json.loads(sys.stdin.read())['failures']))" 2>/dev/null || echo "$views")")
t_views=$SECONDS

wait $sim
t_sim=$SECONDS
soak=$(tail -1 out/check/soak.json)
echo "$soak" | grep -q '"soak":"ok"' || fails+=("soak: $soak")
replay=$(tail -1 out/check/replay.json)
echo "$replay" | grep -q '"diverged":null' || fails+=("replay: $replay (repro: technoape-server replay=out/check/soak.rec)")

bench=()
for r in $RUNS; do
  line=$(timeout 170 target/release/technoape.exe run=$r quick=1 2>/dev/null | tail -1)
  bench+=("$line")
  echo "$line" | python -I -c "import json,sys; d=json.loads(sys.stdin.read()); sys.exit(0 if d['budget_ok'] and (d['run']!='fly' or d['missing_chunks_max']==0) else 1)" 2>/dev/null \
    || fails+=("bench $r: budget or streaming failed (repro: technoape.exe run=$r quick=1)")
done
t_bench=$SECONDS

# a played run: walk over hills, dig, place, build with the hammer. Clicks must show within a 60 Hz frame,
# and the server must never correct the prediction (ADR 0019)
ACTS="0.5:m:0:250,1:say:/playmode creative,2:L,2.1:l,3:R,3.1:r,4:L,4.1:l,5:h,5.2:R,5.3:m:-100:0,5.6:r,7:L,7.1:l,8:R,8.1:r"
feel=$(MSYS_NO_PATHCONV=1 timeout 120 target/release/technoape.exe run=play save=0 seconds=10 autowalk=1 acts="$ACTS" 2>/dev/null | tail -1 \
  | python -I -c "import json,sys; print(json.dumps(json.loads(sys.stdin.read())['feel']))" 2>/dev/null)
echo "$feel" | python -I -c "import json,sys; f=json.loads(sys.stdin.read()); sys.exit(0 if f['corrections'] == 0 and f['clicks'] >= 4 and f['clicks_not_shown'] == 0 and f['click_to_change_ms_max'] <= 16.7 else 1)" 2>/dev/null \
  || fails+=("feel: $feel (repro: technoape.exe run=play save=0 seconds=10 autowalk=1 acts=\"$ACTS\")")

times="{\"build\":$t_build,\"tests\":$((t_tests-t_build)),\"views\":$((t_views-t_tests)),\"soak_wait\":$((t_sim-t_views)),\"bench\":$((t_bench-t_sim)),\"feel\":$((SECONDS-t_bench)),\"total\":$SECONDS}"
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
    if isinstance(d, dict): b[d['run']] = {k: d[k] for k in ('budget_ok', 'fps_avg', 'frame_p99_ms', 'frame_max_ms', 'missing_chunks_max')}
r = js(replay)
v, f = js(views), js(feel)
print(json.dumps({'ok': not fails, 'fast': '$FAST' == '1', 'seconds': js(times), 'tests': tests, 'soak': js(soak), 'replay': {k: r[k] for k in ('ticks', 'checks', 'diverged', 'hash')} if isinstance(r, dict) and 'checks' in r else r, 'bench': b,
    'views': v['views'] if isinstance(v, dict) else v,
    'feel': {k: f[k] for k in ('corrections', 'jerk_p99', 'jerk_max', 'clicks', 'click_to_change_frames_max', 'click_to_change_ms_max')} if isinstance(f, dict) else f,
    'failures': fails}))
sys.exit(1 if fails else 0)
"
