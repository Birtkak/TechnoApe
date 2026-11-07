#!/usr/bin/env bash
# Puts the current checkout's server on the Unraid box (ADR 0145): copies game/ and deploy/ over Tailscale,
# builds the image there and (re)starts the container with host networking and /mnt/user/appdata/technoape/data.
# The world survives: it lives in that data folder, not in the image. Needs ssh root@<box> to work without a password.
#   bash tools/deploy_server.sh [box=100.124.84.125] [seed=1337] [name=Tower] [port=7777] [gamemode=survival]
set -eu
cd "$(dirname "$0")/.."
BOX=100.124.84.125; SEED=1337; NAME=Tower; PORT=7777; GAMEMODE=survival
for a in "$@"; do case "$a" in box=*) BOX=${a#box=};; seed=*) SEED=${a#seed=};; name=*) NAME=${a#name=};; port=*) PORT=${a#port=};; gamemode=*) GAMEMODE=${a#gamemode=};; esac; done
APP=/mnt/user/appdata/technoape
echo "copying game/ and deploy/ to root@$BOX:$APP/src"
tar czf - --exclude='game/.godot' --exclude='game/out' --exclude='game/reports' --exclude='game/addons/zylann.voxel/bin' \
  --exclude='game/addons/technosim/bin' --exclude='game/addons/gdUnit4' --exclude='game/test' --exclude='game/bin' --exclude='game/obj' \
  game deploy .dockerignore 2>/dev/null | ssh -o BatchMode=yes "root@$BOX" "rm -rf $APP/src && mkdir -p $APP/src $APP/data && tar xzf - --warning=no-timestamp -C $APP/src"
echo "building technoape-server on the box"
ssh -o BatchMode=yes "root@$BOX" "cd $APP/src && docker build -q -f deploy/Dockerfile -t technoape-server:latest . && chmod +x deploy/entry.sh \
  && docker rm -f technoape-server >/dev/null 2>&1; docker run -d --name technoape-server --network host --restart unless-stopped \
  -e SEED=$SEED -e PORT=$PORT -e GAMEMODE=$GAMEMODE -e SERVER_NAME=$NAME -v $APP/data:/data technoape-server:latest >/dev/null \
  && sleep 5 && docker logs technoape-server 2>&1 | grep net_server"
echo "the Multiplayer page lists it as $NAME on every machine of the tailnet; address $BOX:$PORT"
