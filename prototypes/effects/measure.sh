#!/usr/bin/env bash
# The effects spike (README.md): run=fx at 4,096 and 16,384 live particles, lit per pixel (fxlit=frag) or with
# one sun visibility per particle (fxlit=one), three quick runs each, release build; and stand without effects.
# Prints one line per run: the frame thread's fx stretch and the GPU's fx stage at p50 and p99.
# Hold out/.lock in the main checkout around it when another session shares the PC.
cd "$(dirname "$0")/../.."
exe=target/release/technoape.exe
show() {
  python -I -c "
import json, sys
d = json.loads(sys.stdin.read())
c, c9, g, g9 = d['cpu_p50'], d['cpu_p99'], d['gpu_p50'], d['gpu_p99']
print('$1', 'live', d.get('fx_live_avg'), 'cpu_fx', c['fx'], c9['fx'], 'gpu_fx', g['fx'], g9['fx'], 'gbuffer', g['gbuffer'], 'gpu_total', g['total'], 'frame_p50', d['frame_p50_ms'], 'p99', d['frame_p99_ms'])"
}
for rep in 1 2 3; do
  for n in 4096 16384; do
    for lit in frag one; do
      timeout 170 $exe run=fx quick=1 fx_n=$n fxlit=$lit 2>/dev/null | tail -1 | show "fx n=$n lit=$lit"
    done
  done
  timeout 170 $exe run=stand quick=1 2>/dev/null | tail -1 | show "stand"
done
