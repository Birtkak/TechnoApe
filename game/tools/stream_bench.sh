#!/bin/bash
# ADR 0135: one fast spectator flight over the land of bug report 2026-11-03_23-05-16 at the default render
# distance, with stream_probe=1 (holes in the box, queues, worker time per stage).
#   bash tools/stream_bench.sh <label> [godot user args, e.g. speed=40 voxel_threads=6]  -> out/stream_bench/<label>.json
#   python -I tools/stream_summ.py out/stream_bench/A_*.json   (medians; run A and B interleaved, 3 each)
# DLL=<name> / TS=<name> first copy ~/Tools/godot_voxel-wt/dll/<name>.dll over the godot_voxel / technosim DLL (A/B).
# Waits (10 min at most, WAIT_TRIES=N tries of 10 s) until no other Godot runs: another session's run skews every number here.
label=$1; shift
cd "$(dirname "$0")/.."
G="$HOME/Tools/Godot/4.7.2-mono/Godot_v4.7.2-stable_mono_win64/Godot_v4.7.2-stable_mono_win64_console.exe"
export MSYS_NO_PATHCONV=1
if [ -n "$DLL" ]; then cp ~/Tools/godot_voxel-wt/dll/$DLL.dll addons/zylann.voxel/bin/libvoxel.windows.template_debug.x86_64.dll; fi
if [ -n "$TS" ]; then cp ~/Tools/godot_voxel-wt/dll/$TS.dll addons/technosim/bin/libtechnosim.windows.template_debug.x86_64.dll; fi
for i in $(seq ${WAIT_TRIES:-60}); do
  n=$(powershell -NoProfile -Command "@(Get-Process -Name Godot* -ErrorAction SilentlyContinue | Where-Object { \$_.ProcessName -notlike '*_console' }).Count")
  [ "${n//[$'\r\n ']/}" = "0" ] && break
  sleep 10
done
mkdir -p out/stream_bench
# the flight starts once the box at the report's spot has settled (load_cap_s=40); it crosses cliffs, savanna, forest, beach
timeout 200 "$G" --path . --position -30000,-30000 --resolution 1280x720 -- autopilot=1 duration=20 vsync=0 seed=1935341982 \
  speed=20 alt=30 follow_ground=1 "commands=/gamemode spectator;/tp 1148 1877 5955" volume_master=0 mob_spawn=0 stream_probe=1 \
  load_cap_s=40 out=res://out/stream_bench/$label "$@" 2>&1 | grep '^{' | tail -1 > out/stream_bench/$label.json
python -I tools/stream_summ.py out/stream_bench/$label.json
