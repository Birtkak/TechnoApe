#!/usr/bin/env bash
# Built things survive their chunks unloading (ADR 0165): a walk away and back in one session, not a rejoin.
# Multiplayer: alice builds a 3x3x3 bricks box (by the spawn) and the /stations factory line (a chest of ore,
# machines) 120 m off in -z (its machines pin a bubble that keeps their ground loaded on purpose); bob watches from the spawn, near the monolith. Both fly 250 m away (far past the 24 m view and the
# bubble), check the box's cell is no longer loaded, fly back and must find the same bricks cells (in the bubble
# and drawn in the terrain), the monolith's cells and the same block entities as before they left.
# The run's final JSON must still list machines holding their ore and coal (the firepit is net_proof.sh's).
# Single-player: the same build and trip with persist=1 (the SQLite save path).
# Prints one JSON line per client, exit 1 when an assertion fails.
#   bash tools/reload_proof.sh [seed] [port]
set -u
cd "$(dirname "$0")/../game" || exit 1
G="$HOME/Tools/Godot/4.7.2-mono/Godot_v4.7.2-stable_mono_win64/Godot_v4.7.2-stable_mono_win64_console.exe"
SEED=${1:-4242}
PORT=${2:-$((20000 + RANDOM % 20000 * 2))}   # random (even: discovery takes port + 1): parallel sessions run proofs too
SP_SEED=$SEED   # its files are <what>_<seed>_g*, the server's net_<seed>_g*: both start fresh
export MSYS_NO_PATHCONV=1
COMMON="view_distance_m=24 volume_master=0 walk=1 speed=0 load_cap_s=40"
W="$APPDATA/Godot/app_userdata/TechnoApe/worlds"
mkdir -p out
rm -rf "$W/net_${SEED}_g"* "$W/"*"_${SP_SEED}_g"*
CHECK="/spawn;wait 4;/monolith;/count bricks;/blocks;/voxel ~4 ~4 ~4"
AWAY="/tp ~2000 ~ ~;wait 12;/voxel ~-1996 ~4 ~4;/count bricks;/monolith;/spawn;wait 16"
BUILD="/gamemode creative;wait 4;/spawn;/fill ~3 ~3 ~3 ~5 ~5 ~5 bricks;/tp ~ ~ ~-960;wait 8;/stations factory;wait 3;/gamemode spectator;/spawn;wait 6"

timeout 140 "$G" --headless --path . -- server=1 seed=$SEED port=$PORT duration=110 > out/reload_server.log 2>&1 &
S=$!
sleep 4
timeout 150 "$G" --headless --path . -- connect=127.0.0.1:$PORT name=alice autopilot=1 duration=75 seed=$SEED $COMMON \
  "later=$BUILD;$CHECK;$AWAY;$CHECK" out=res://out/reload_a > out/reload_a.log 2>&1 &
A=$!
sleep 2
timeout 150 "$G" --headless --path . -- connect=127.0.0.1:$PORT name=bob autopilot=1 duration=75 seed=$SEED $COMMON \
  "later=/gamemode spectator;wait 18;$CHECK;$AWAY;$CHECK" out=res://out/reload_b > out/reload_b.log 2>&1 &
B=$!
timeout 150 "$G" --headless --path . -- autopilot=1 duration=75 seed=$SP_SEED persist=1 $COMMON \
  "later=$BUILD;$CHECK;$AWAY;$CHECK" out=res://out/reload_sp > out/reload_sp.log 2>&1 &
P=$!
wait $A $B $P $S

fail=0
for who in a b sp; do
  python -I - "out/reload_$who.log" "$who" <<'EOF' || fail=1
import json, re, sys
log, who = sys.argv[1], sys.argv[2]
lines = [l.rstrip("\n") for l in open(log, encoding="utf-8", errors="replace") if l.startswith("later /")]
def results(cmd):
    return [l.split(" -> ", 1)[1] for l in lines if l.startswith("later " + cmd + " ->")]
def num(t):
    m = re.search(r"(\d+) cells", t); return int(m.group(1)) if m else -1
def mono_of(t):   # [in place, loaded, air at a box centre, in place in the bubble]
    m = re.search(r"(\d+) of (\d+) loaded boxes in place, (\d+) air .*; (\d+) in the bubble", t); return [int(g) for g in m.groups()] if m else [0, 0, 0, 0]
def nblocks(t):
    m = re.match(r"(\d+) block entities", t); return int(m.group(1)) if m else 0
mono, glass, blocks, vox = results("/monolith"), results("/count bricks"), results("/blocks"), [l.split(" -> ",1)[1] for l in lines if l.startswith("later /voxel ")]
r = {"who": who, "monolith": [mono_of(t) for t in mono], "bricks": [num(t) for t in glass], "blocks": [nblocks(t) for t in blocks], "voxel": vox}
ok = len(mono) == 3 and len(glass) == 3 and len(blocks) == 2 and len(vox) == 3
if ok:
    g0, g_away, g1 = r["bricks"]
    ok = (g0 == 27 and g_away == 0 and g1 == 27                        # the box, gone with the bubble, back
          and "terrain=bricks" in vox[0] and "bricks" not in vox[1] and "terrain=bricks" in vox[2]
          and r["monolith"][0][1] > 0 and r["monolith"][1][1] == 0           # loaded near the spawn, unloaded away
          and r["monolith"][2][1] == r["monolith"][0][1]                    # the same boxes loaded
          and r["monolith"][2][0] >= r["monolith"][0][0]                    # none lost (the raise may finish more)
          and r["monolith"][2][2] <= r["monolith"][0][2]
          and r["blocks"][0] >= 4 and r["blocks"][1] == r["blocks"][0])
st = {}
for l in open(log, encoding="utf-8", errors="replace"):
    if l.startswith("{") and '"blocks":{"count"' in l:
        st = json.loads(l)
ents = st.get("blocks", {}).get("entities", [])
r["filled_end"] = sum(1 for e in ents if any(isinstance(e.get(k), dict) and e[k] for k in ("input", "fuel", "output")))
ok = ok and r["filled_end"] >= 1   # the machines still hold their ore and coal at the end
r["ok"] = ok
print(json.dumps(r))
sys.exit(0 if ok else 1)
EOF
done
exit $fail
