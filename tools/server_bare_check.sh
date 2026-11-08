#!/usr/bin/env bash
# The dedicated server runs the stock Godot binary with no voxel or sim extension (deploy/Dockerfile drops them), so
# every script it loads must parse without them. This starts the working tree's server that way for a few seconds
# and requires its "listening" line. Exit 1 when it fails; the offending parse errors are printed.
#   bash tools/server_bare_check.sh [port]
set -u
cd "$(dirname "$0")/.." || exit 1
G="$HOME/Tools/Godot/4.7.2-mono/Godot_v4.7.2-stable_mono_win64/Godot_v4.7.2-stable_mono_win64_console.exe"
PORT=${1:-$((20000 + RANDOM % 20000 * 2))}
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
tar cf - --exclude='game/.godot' --exclude='game/out' --exclude='game/reports' --exclude='game/addons/zylann.voxel' \
  --exclude='game/addons/technosim' --exclude='game/addons/gdUnit4' --exclude='game/test' game | tar xf - -C "$T"
cd "$T/game" || exit 1
timeout 300 "$G" --headless --path . --import > import.log 2>&1
timeout 60 "$G" --headless --path . -s res://scripts/net/server_main.gd -- server=1 seed=5 port=$PORT duration=6 > server.log 2>&1
if grep -q '"net_server": *"listening"\|"net_server":"listening"' server.log; then
  echo "server_bare_check: ok (listens without the extensions)"
  exit 0
fi
echo "server_bare_check: FAIL: the server did not listen without the extensions"
grep -B2 "Parse Error\|Compile Error" server.log | grep -v "^--$" | head -20
exit 1
