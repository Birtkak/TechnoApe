#!/usr/bin/env bash
# Puts the current checkout on the Unraid box (ADR 0145, 0166) in one command: stamps a build version, exports
# the Windows client, copies game/ and deploy/ over Tailscale, builds the image there, puts the client files in
# /mnt/user/appdata/technoape/data/client (served to players on port + 2, so their game updates itself on the
# next start or join) and (re)starts the container with host networking. The world survives: it lives in that
# data folder, not in the image. Needs ssh root@<box> to work without a password.
#   bash tools/deploy_server.sh [box=100.124.84.125] [seed=1337] [name=Tower] [port=7777] [gamemode=survival] [client=0]
#   client=0 deploys the server alone (the players' build stays as it is, and so does the version they need).
set -eu
cd "$(dirname "$0")/.."
BOX=100.124.84.125; SEED=1337; NAME=Tower; PORT=7777; GAMEMODE=survival; CLIENT=1
for a in "$@"; do case "$a" in box=*) BOX=${a#box=};; seed=*) SEED=${a#seed=};; name=*) NAME=${a#name=};; port=*) PORT=${a#port=};; gamemode=*) GAMEMODE=${a#gamemode=};; client=*) CLIENT=${a#client=};; esac; done
APP=/mnt/user/appdata/technoape
G="$HOME/Tools/Godot/4.7.2-mono/Godot_v4.7.2-stable_mono_win64/Godot_v4.7.2-stable_mono_win64_console.exe"
if [ "$CLIENT" = 1 ]; then
  VER="$(date -u +%Y%m%d-%H%M)-$(git rev-parse --short HEAD)$(git diff --quiet HEAD -- game || echo -dirty)"
  echo "$VER" > game/build_version.txt
  echo "exporting the client, build $VER"
  rm -rf build/client && mkdir -p build/client
  timeout 600 "$G" --headless --path game --export-debug "Windows Desktop" "$(pwd -W 2>/dev/null || pwd)/build/client/TechnoApe.exe" > build/client_export.log 2>&1
  [ -f build/client/TechnoApe.pck ] || { echo "export failed: see build/client_export.log"; exit 1; }
  echo "$VER" > build/client/version.txt
fi
echo "copying game/ and deploy/ to root@$BOX:$APP/src"
tar czf - --exclude='game/.godot' --exclude='game/out' --exclude='game/reports' --exclude='game/addons/zylann.voxel/bin' \
  --exclude='game/addons/technosim/bin' --exclude='game/addons/gdUnit4' --exclude='game/test' --exclude='game/bin' --exclude='game/obj' \
  game deploy .dockerignore 2>/dev/null | ssh -o BatchMode=yes "root@$BOX" "rm -rf $APP/src && mkdir -p $APP/src $APP/data && tar xzf - --warning=no-timestamp -C $APP/src"
if [ "$CLIENT" = 1 ]; then
  echo "copying the client ($(du -sh build/client | cut -f1)) to root@$BOX:$APP/data/client"
  tar cf - -C build/client . | ssh -o BatchMode=yes "root@$BOX" "rm -rf $APP/data/client.new && mkdir -p $APP/data/client.new && tar xf - -C $APP/data/client.new \
    && rm -rf $APP/data/client.old && { [ ! -d $APP/data/client ] || mv $APP/data/client $APP/data/client.old; } && mv $APP/data/client.new $APP/data/client"
fi
echo "building technoape-server on the box"
ssh -o BatchMode=yes "root@$BOX" "cd $APP/src && docker build -q -f deploy/Dockerfile -t technoape-server:latest . && chmod +x deploy/entry.sh \
  && docker rm -f technoape-server >/dev/null 2>&1; docker run -d --name technoape-server --network host --restart unless-stopped \
  -e SEED=$SEED -e PORT=$PORT -e GAMEMODE=$GAMEMODE -e SERVER_NAME=$NAME -v $APP/data:/data technoape-server:latest >/dev/null \
  && sleep 5 && docker logs technoape-server 2>&1 | grep net_server"
if [ "$CLIENT" = 1 ]; then echo "shipped client build $VER ($(ls build/client | grep -vc version.txt) files, $(du -sh build/client | cut -f1)): players update on their next start or join"
else echo "client build unchanged: $(ssh -o BatchMode=yes "root@$BOX" "cat $APP/data/client/version.txt 2>/dev/null || echo none")"; fi
echo "the Multiplayer page lists it as $NAME on every machine of the tailnet; address $BOX:$PORT; client updates on $((PORT + 2))/tcp"
