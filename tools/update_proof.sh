#!/usr/bin/env bash
# Client self-update proof without eyes (ADR 0166): exports the client twice (build "old" and build "new"),
# serves "new" from a local headless server, starts the "old" exported client with connect=, and checks that
# it is refused, downloads the changed files, keeps *.old, restarts itself and joins on the new build.
# Prints one JSON line, exit 1 when an assertion fails. Evidence in game/out/upd/.
#   [STAGE=1] bash tools/update_proof.sh [port=7791]
set -u
cd "$(dirname "$0")/../game" || exit 1
G="$HOME/Tools/Godot/4.7.2-mono/Godot_v4.7.2-stable_mono_win64/Godot_v4.7.2-stable_mono_win64_console.exe"
PORT=7791; for a in "$@"; do case "$a" in port=*) PORT=${a#port=};; esac; done
export MSYS_NO_PATHCONV=1
U="$(pwd -W 2>/dev/null || pwd)/out/upd"
rm -rf out/upd && mkdir -p out/upd/old out/upd/new
had=""; [ -f build_version.txt ] && had=$(cat build_version.txt)
for v in old new; do
  mkdir -p out/upd/$v && echo "proof-$v" > build_version.txt
  timeout 400 "$G" --headless --path . --export-debug "Windows Desktop" "$U/$v/TechnoApe.exe" > out/upd/export_$v.log 2>&1
  [ -f out/upd/$v/TechnoApe.pck ] || { echo "export $v failed: see game/out/upd/export_$v.log"; exit 1; }
done
if [ -n "$had" ]; then echo "$had" > build_version.txt; else rm -f build_version.txt; fi
echo "proof-new" > out/upd/new/version.txt
cp -r out/upd/old out/upd/client
# STAGE=1: an earlier interrupted try left the new .pck in update/: it must be used, not fetched again
[ "${STAGE:-0}" = 1 ] && mkdir -p out/upd/client/update && cp out/upd/new/TechnoApe.pck out/upd/client/update/

timeout 200 "$G" --headless --path . -- server=1 seed=4243 port=$PORT client_dir="$U/new" duration=180 > out/upd/server.log 2>&1 &
SERVER=$!
sleep 5
timeout 120 out/upd/client/TechnoApe.console.exe --headless -- connect=127.0.0.1:$PORT name=upd \
  autopilot=1 duration=8 seed=4243 view_distance_m=24 volume_master=0 walk=1 speed=0 load_cap_s=40 out="$U/run" > out/upd/client_first.log 2>&1
# the first process quits after starting the updated one (detached, its stdout is gone): the server's join
# and leave lines are its evidence (it leaves when its duration= is up); only the new build passes the version gate
for i in $(seq 1 150); do grep -q "upd left" out/upd/server.log && break; sleep 1; done
kill $SERVER 2>/dev/null; wait $SERVER 2>/dev/null

STAGE=${STAGE:-0} python -I - "$U" <<'EOF'
import json, sys, hashlib, os
u = sys.argv[1]
sha = lambda p: hashlib.sha256(open(p, "rb").read()).hexdigest() if os.path.exists(p) else None
first = open(os.path.join(u, "client_first.log"), encoding="utf-8", errors="replace").read()
server = open(os.path.join(u, "server.log"), encoding="utf-8", errors="replace").read()
r = {
  "refused_old": "refused, build proof-old" in server,
  "installed": '"update":"installed"' in first.replace(" ", ""),
  "pck_now_new": sha(os.path.join(u, "client/TechnoApe.pck")) == sha(os.path.join(u, "new/TechnoApe.pck")) != sha(os.path.join(u, "old/TechnoApe.pck")),
  "old_cleaned": not os.path.exists(os.path.join(u, "client/TechnoApe.pck.old")) and not os.path.exists(os.path.join(u, "client/update")),
  "staged_reused": '"downloaded_bytes":0,' in first.replace(" ", "") if os.environ.get("STAGE") == "1" else True,
  "joined_after": server.find("joined as peer") > server.find("refused, build proof-old") >= 0,
  "played": server.rfind("upd left") > server.find("joined as peer") >= 0,
  "restart_flag": "updated_from=proof-old" in first,
}
r["ok"] = all(r.values())
print(json.dumps({"update_proof": r}))
sys.exit(0 if r["ok"] else 1)
EOF
