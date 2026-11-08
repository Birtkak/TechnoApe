#!/usr/bin/env bash
# Built things survive their chunks unloading (ADR 0165): a walk away and back in one session, not a rejoin.
# Multiplayer: alice builds a 3x3x3 bricks box (by the spawn) and the /stations factory line (a chest of ore,
# machines) 120 m off in -z (its machines pin a bubble that keeps their ground loaded on purpose); bob watches from the spawn, near the monolith. Both fly 250 m away (far past the 24 m view and the
# bubble), check the box's cell is no longer loaded, fly back and must find the same bricks cells (in the bubble
# and drawn in the terrain), the monolith's cells and the same block entities as before they left.
# The run's final JSON must still list machines holding their ore and coal (the firepit is net_proof.sh's).
# Single-player: the same build and trip with persist=1 (the SQLite save path).
# Prints one JSON line per client, exit 1 when an assertion fails.
#   [VIEW=72] bash tools/reload_proof.sh [seed] [port]
set -u
cd "$(dirname "$0")/../game" || exit 1
G="$HOME/Tools/Godot/4.7.2-mono/Godot_v4.7.2-stable_mono_win64/Godot_v4.7.2-stable_mono_win64_console.exe"
SEED=${1:-4242}
PORT=${2:-$((20000 + RANDOM % 20000 * 2))}   # random (even: discovery takes port + 1): parallel sessions run proofs too
SP_SEED=$SEED   # its files are <what>_<seed>_g*, the server's net_<seed>_g*: both start fresh
export MSYS_NO_PATHCONV=1
VIEW=${VIEW:-24}   # VIEW=72 (the default view): terrain outside the 64 m bubble is drawn too, the ADR 0169 case
COMMON="view_distance_m=$VIEW volume_master=0 walk=1 speed=0 load_cap_s=40"
W="$APPDATA/Godot/app_userdata/TechnoApe/worlds"
mkdir -p out
rm -rf "$W/net_${SEED}_g"* "$W/"*"_${SP_SEED}_g"*
CHECK="/spawn;wait 4;/monolith;/count bricks;/blocks;/voxel ~4 ~4 ~4"
AWAY_S=$(( VIEW > 24 ? 30 : 12 ))   # a 72 m box of blocks takes longer than 12 s to stream out
AWAY="/tp ~2000 ~ ~;wait $AWAY_S;/voxel ~-1996 ~4 ~4;/count bricks;/monolith;/spawn;wait 16"
BUILD="/gamemode creative;wait 4;/spawn;/fill ~3 ~3 ~3 ~5 ~5 ~5 bricks;/tp ~ ~ ~-960;wait 8;/stations factory;wait 3;/gamemode spectator;/spawn;wait 6"

# the server outlives the clients by a wide margin: their clock pauses in every loading cover (four far teleports at 72 m)
timeout $((180 + AWAY_S * 2)) "$G" --headless --path . -- server=1 seed=$SEED port=$PORT duration=$((150 + AWAY_S * 2)) > out/reload_server.log 2>&1 &
S=$!
sleep 4
timeout $((170 + AWAY_S * 2)) "$G" --headless --path . -- connect=127.0.0.1:$PORT name=alice autopilot=1 duration=$((75 + AWAY_S)) seed=$SEED $COMMON \
  "later=$BUILD;$CHECK;$AWAY;$CHECK" out=res://out/reload_a > out/reload_a.log 2>&1 &
A=$!
sleep 2
timeout $((170 + AWAY_S * 2)) "$G" --headless --path . -- connect=127.0.0.1:$PORT name=bob autopilot=1 duration=$((75 + AWAY_S)) seed=$SEED $COMMON \
  "later=/gamemode spectator;wait 18;$CHECK;$AWAY;$CHECK" out=res://out/reload_b > out/reload_b.log 2>&1 &
B=$!
timeout $((170 + AWAY_S * 2)) "$G" --headless --path . -- autopilot=1 duration=$((75 + AWAY_S)) seed=$SP_SEED persist=1 $COMMON \
  "later=$BUILD;$CHECK;$AWAY;$CHECK" out=res://out/reload_sp > out/reload_sp.log 2>&1 &
P=$!
wait $A $B $P $S

fail=0
for who in a b sp; do
  python -I - "out/reload_$who.log" "$who" "$VIEW" <<'EOF' || fail=1
import json, re, sys
log, who, far = sys.argv[1], sys.argv[2], int(sys.argv[3]) > 24   # far: the ground shell (ADR 0124) keeps far ground loaded, so only the bubble unloads
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
          and "terrain=bricks" in vox[0] and (far or "bricks" not in vox[1]) and "terrain=bricks" in vox[2]
          and r["monolith"][0][1] > 0 and (far or r["monolith"][1][1] == 0)  # loaded near the spawn, unloaded away
          and r["monolith"][2][1] == r["monolith"][0][1]                    # the same boxes loaded
          and r["monolith"][2][0] >= r["monolith"][0][0]                    # none lost (the raise may finish more)
          and r["monolith"][2][2] <= r["monolith"][0][2]
          and r["blocks"][0] >= 4 and r["blocks"][1] == r["blocks"][0])
st = {}
for l in open(log, encoding="utf-8", errors="replace"):
    if l.startswith("{") and '"blocks":{"count"' in l:
        st = json.loads(l)
ents = st.get("blocks", {}).get("entities", [])
net = st.get("net", {})
r["chunks"] = {k: net.get(k, 0) for k in ("chunks_sent", "chunks_recv", "chunks_pasted", "ops_skipped", "chunks")}
if who != "sp":
    ok = ok and net.get("chunks_sent", 0) > 0 and net.get("chunks_recv", 0) > 0   # the server holds the edited chunks (ADR 0169), both ways
r["filled_end"] = sum(1 for e in ents if any(isinstance(e.get(k), dict) and e[k] for k in ("input", "fuel", "output")))
ok = ok and r["filled_end"] >= 1   # the machines still hold their ore and coal at the end
r["ok"] = ok
print(json.dumps(r))
sys.exit(0 if ok else 1)
EOF
done
exit $fail
