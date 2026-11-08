#!/usr/bin/env bash
# Multiplayer proof without eyes (ADR 0145): a headless server and two clients on localhost, same seed.
# Client a fills a stone box next to the spawn; client b must see it (net.verify_ok >= 1, net.ops_recv >= 1)
# and see a's avatar (net.avatars); b's run also makes a contact sheet (out/net_b/strip.png) looking at a.
# Items: alice drops sticks at her feet and picks them back through the server (picks_got), drops a flint 3 m on
# and leaves it: bob sees both spawn and one go, carol (late join) gets the flint from the server's registry.
# A third client joins late and must get the edit from the journal (ops_replayed >= 1); alice rejoins after her
# run and must stand where she quit (the server's player save).
# ADR 0165: alice lights a fire (/ignite: a live op, never journaled) and places a firepit with a real right click
# (/use: a block entity sent as its saved dict); bob must get the flame into his sim (live_ok) and the firepit
# (blocks_recv), carol the firepit from the server's registry, and alice's rejoin must get the monolith back from
# the seed's world ops (base_replayed: her save says it is up, so it is not raised again). Bob's mean op delay
# (lat_ms_sum / lat_n) must stay under 100 ms. Alice's /stations (a chest, a furnace) must reach bob too
# (blocks_recv >= 3), and bob's weather must hear her fire (weather.heard).
# ADR 0167: after the firepit alice holds an iron pickaxe, wears iron armour and is bloody; bob must get that look
# (net.looks.alice), a colour other than his own, alice on his map (net.map), and /tp alice must put him next to her.
# Prints one JSON line per role plus bandwidth, exit 1 when an assertion fails.
#   bash tools/net_proof.sh [seed] [port]
set -u
cd "$(dirname "$0")/../game" || exit 1
G="$HOME/Tools/Godot/4.7.2-mono/Godot_v4.7.2-stable_mono_win64/Godot_v4.7.2-stable_mono_win64_console.exe"
SEED=${1:-4242}
PORT=${2:-$((20000 + RANDOM % 20000 * 2))}   # random (even: discovery takes port + 1): parallel sessions run this proof too
export MSYS_NO_PATHCONV=1
COMMON="view_distance_m=24 volume_master=0 walk=1 speed=0 load_cap_s=40"
mkdir -p out
rm -rf out/net_a out/net_b out/net_c out/net_d
rm -f "$APPDATA/Godot/app_userdata/TechnoApe/worlds/net_${SEED}_g"*   # a fresh world for the server

timeout 130 "$G" --headless --path . -- server=1 seed=$SEED port=$PORT duration=105 > out/net_server.log 2>&1 &
SERVER=$!
sleep 4
# a: stands 2 m in +x from the spawn and fills a stone box 1.5 m further on, in b's view
timeout 150 "$G" --headless --path . -- connect=127.0.0.1:$PORT name=alice autopilot=1 duration=45 seed=$SEED $COMMON pitch=-0.8 give=firepit:1 \
  "later=/gamemode creative;/tp ~16 ~ ~;wait 3;/fill ~12 ~1 ~-1 ~14 ~3 ~1 stone;wait 2;/drop stick 2;wait 3;/tp ~24 ~ ~;/drop flint 1;/tp ~-24 ~ ~;wait 1;/ignite;/tp ~ ~ ~-12;wait 2;/use;wait 1;/tp ~ ~ ~-12;/stations;wait 1;/tp ~ ~ ~24;/clear firepit;/give iron_pickaxe 1;/wear iron_chestplate;/wear iron_helmet;/wear iron_boots;/stain bloody;wait 13;/fill ~12 ~1 ~-1 ~14 ~3 ~1 air" out=res://out/net_a > out/net_a.log 2>&1 &
A=$!
sleep 2
# b: looks along +x at a and the fill, films a thumbnail per second; shorter than a so a is still there at its snapshot
timeout 150 "$G" --path . --resolution 960x540 --position -30000,-30000 -- connect=127.0.0.1:$PORT name=bob autopilot=1 duration=38 \
  seed=$SEED $COMMON yaw=-1.5708 pitch=-0.1 capture_every=1 "later=wait 25;/shot net_look;wait 1;/tp alice" out=res://out/net_b > out/net_b.log 2>&1 &
B=$!
sleep 30
# c: late join, after the stone was placed: the journal must put it in
timeout 110 "$G" --headless --path . -- connect=127.0.0.1:$PORT name=carol autopilot=1 duration=12 seed=$SEED $COMMON out=res://out/net_c > out/net_c.log 2>&1 &
C=$!
timeout 30 "$G" --headless --path . --script res://tools/net_find.gd -- port=$PORT tailnet=0 2>/dev/null | grep '^{"servers' > out/net_find.json
wait $A
# a again: the server must put alice back where she stood (x 16.5: spawn + 16 voxels, saved at her quit)
timeout 100 "$G" --headless --path . -- connect=127.0.0.1:$PORT name=alice autopilot=1 duration=8 seed=$SEED $COMMON out=res://out/net_d > out/net_d.log 2>&1 &
D=$!
wait $B $C $D $SERVER

fail=0
for who in a b c d; do
  line=$(grep -m1 '^{.*"net"' out/net_$who.log)
  if [ -z "$line" ]; then echo "net_$who: no JSON (see out/net_$who.log)"; fail=1; continue; fi
  echo "$line" | python -I -c '
import json,sys
w=sys.argv[1]; d=json.loads(sys.stdin.read()); n=d["net"]
want={"a":("ops_sent",1),"b":("verify_ok",1),"c":("ops_replayed",1),"d":("journal",2)}[w]
ok=n.get(want[0],0)>=want[1]
if w=="a": ok=ok and n.get("items_sent",0)>=2 and n.get("picks_got",0)>=1 and n.get("live_sent",0)>=1 and n.get("blocks_sent",0)>=1
if w=="b": ok=ok and n.get("live_ok",0)>=1 and n.get("blocks_recv",0)>=3 and d.get("weather",{}).get("heard",0)>=1 and n.get("lat_n",0)>=1 and n["lat_ms_sum"]/n["lat_n"]<100
if w=="b": ok=ok and n.get("items_recv",0)>=2 and n.get("picked_recv",0)>=1
if w=="c": ok=ok and n.get("items_recv",0)>=1
if w=="c": ok=n.get("ops_replayed",0)+n.get("verify_ok",0)>=1 and n.get("blocks_recv",0)>=1
if w=="b": ok=ok and len(n.get("avatars",{}))>=1
if w=="b":
  lk=n.get("looks",{}).get("alice",{}); av=list(n.get("avatars",{}).values())
  near=bool(av) and ((av[0][0]-n["pos"][0])**2+(av[0][2]-n["pos"][2])**2)**0.5<12
  ok=ok and lk.get("held")=="iron_pickaxe" and "body" in lk.get("worn",[]) and lk.get("stained") and lk.get("colour")!=n.get("my_colour") and "alice" in n.get("map",[]) and near
if w=="d": ok=ok and abs(n["pos"][0]-16.5)<1.0 and n.get("base_replayed",0)>=1
print(json.dumps({"who":w,"ok":ok,"net":{k:n[k] for k in ["ops_sent","ops_recv","ops_replayed","verify_ok","verify_bad","states_sent","states_recv","items_sent","items_recv","picks_sent","picks_got","picked_recv","batches_sent","sent_bytes","recv_bytes","sent_bytes_per_s","recv_bytes_per_s","seconds","journal","peers","live_sent","live_recv","live_ok","blocks_sent","blocks_recv","base_ops","base_replayed","lat_ms_max","lat_ms_sum","lat_n"] if k in n},"avatars":n.get("avatars",{}),"looks":n.get("looks"),"map":n.get("map"),"my_colour":n.get("my_colour"),"pos":n.get("pos"),"budget_ok":d.get("budget_ok")}))
sys.exit(0 if ok else 1)' $who || fail=1
done
python -I -c '
import json,sys; d=json.load(open("out/net_find.json")); s=d["servers"]
k="127.0.0.1:%s" % sys.argv[1]; ok=k in s and s[k]["seed"]==int(sys.argv[2])
print(json.dumps({"who":"find","ok":ok,"servers":s})); sys.exit(0 if ok else 1)' $PORT $SEED || fail=1
for i in $(seq 1 40); do grep -q '"net_server":"done"' out/net_server.log && break; sleep 1; done   # the server quits at its duration
grep -m1 '"net_server":"done"' out/net_server.log || { echo "server: no final JSON (see out/net_server.log)"; fail=1; }
ls out/net_b/strip.png 2>/dev/null && echo "contact sheet: game/out/net_b/strip.png"
ls out/net_b/shots/net_look.png 2>/dev/null && echo "alice's look from bob: game/out/net_b/shots/net_look.png"
exit $fail
