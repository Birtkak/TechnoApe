#!/usr/bin/env bash
# The one gate (docs/design/fundamentals.md section 4, ADR 0011): tests, a soak on a fresh seed, a replay of
# that soak (determinism and the flight recorder), the quick benchmark (budget in every run, no missing chunks
# in fly). Prints one JSON verdict line, exit 1 on any failure; each failure carries its repro line.
# Usage: bash tools/check.sh [soak_minutes]   (default 3; about 1.5 min in all)
cd "$(dirname "$0")/.."
export PATH="$HOME/Tools/rust/cargo/bin:$PATH" CARGO_HOME="${CARGO_HOME:-$HOME/Tools/rust/cargo}" RUSTUP_HOME="${RUSTUP_HOME:-$HOME/Tools/rust/rustup}"
mkdir -p out/check
[ -d out/.lock ] && { echo '{"ok":false,"error":"a benchmark is running (out/.lock)"}'; exit 1; }
MIN=${1:-3}
SEED=$(( $(date +%s) % 100000 ))
fails=()

if cargo test --workspace >out/check/test.log 2>&1; then tests=ok; else
  tests=fail; fails+=("cargo test: $(grep -E '^test .* FAILED$' out/check/test.log | head -3 | sed 's/^test //;s/ \.\.\. FAILED//' | tr '\n' ' ')")
fi
cargo build --release >out/check/build.log 2>&1 || { echo '{"ok":false,"error":"release build failed: out/check/build.log"}'; exit 1; }

S=target/release/technoape-server.exe
soak=$(timeout 600 $S soak=1 bots=50 minutes=$MIN seed=$SEED record=out/check/soak.rec)
echo "$soak" | grep -q '"soak":"ok"' || fails+=("soak: $soak")
replay=$(timeout 600 $S replay=out/check/soak.rec)
echo "$replay" | grep -q '"diverged":null' || fails+=("replay: $replay (repro: technoape-server replay=out/check/soak.rec)")

bench=()
for r in stand fly orbit edit; do
  line=$(timeout 170 target/release/technoape.exe run=$r quick=1 2>/dev/null | tail -1)
  bench+=("$line")
  echo "$line" | python -I -c "import json,sys; d=json.loads(sys.stdin.read()); sys.exit(0 if d['budget_ok'] and (d['run']!='fly' or d['missing_chunks_max']==0) else 1)" 2>/dev/null \
    || fails+=("bench $r: budget or streaming failed (repro: technoape.exe run=$r quick=1)")
done

printf '%s\n' "$tests" "$soak" "$replay" "${bench[@]}" "--" "${fails[@]}" | python -I -c "
import json, sys
lines = sys.stdin.read().split('\n')
cut = lines.index('--')
tests, soak, replay, bench, fails = lines[0], lines[1], lines[2], lines[3:cut], [l for l in lines[cut + 1:] if l]
def js(s):
    try: return json.loads(s)
    except Exception: return s
b = {}
for l in bench:
    d = js(l)
    if isinstance(d, dict): b[d['run']] = {k: d[k] for k in ('budget_ok', 'fps_avg', 'frame_p99_ms', 'frame_max_ms', 'missing_chunks_max')}
r = js(replay)
print(json.dumps({'ok': not fails, 'tests': tests, 'soak': js(soak), 'replay': {k: r[k] for k in ('ticks', 'checks', 'diverged', 'hash')} if isinstance(r, dict) and 'checks' in r else r, 'bench': b, 'failures': fails}))
sys.exit(1 if fails else 0)
"
