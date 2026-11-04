#!/bin/bash
# A/B of two technosim DLLs on tools/gen_bench.gd (ADR 0135), interleaved: bash tools/gen_ab.sh <a> <b> [reps] [bench args]
# (names in ~/Tools/godot_voxel-wt/dll/). Prints hash, wall and CPU us per block per run, and the column-context
# build (ground_range over the lane and its margin, so an upper bound) apart: compare "sum" when work moves into it.
cd "$(dirname "$0")/.."
G="$HOME/Tools/Godot/4.7.2-mono/Godot_v4.7.2-stable_mono_win64/Godot_v4.7.2-stable_mono_win64_console.exe"
A=$1 B=$2 R=${3:-2}; shift 3
for r in $(seq $R); do for d in $A $B; do
  cp ~/Tools/godot_voxel-wt/dll/$d.dll addons/technosim/bin/libtechnosim.windows.template_debug.x86_64.dll
  timeout 300 "$G" --headless --path . --script res://tools/gen_bench.gd -- "$@" 2>&1 | grep '^{' | python -I -c "
import json,sys; d=json.loads(sys.stdin.read()); c = d.get('ctx_cpu_us_per_block', 0); print('$d', d['gen_bench'][:8], d['us_per_block'], 'cpu', d['cpu_us_per_block'], 'ctx', c, 'sum', round(d['cpu_us_per_block'] + c, 1))"
done; done
