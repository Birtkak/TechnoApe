#!/bin/sh
# The container's command: the server with its settings from the environment (ADR 0145).
#   SEED (1337) · PORT (7777; the Multiplayer list probes PORT+1, client updates on PORT+2/tcp from /data/client) · GAMEMODE (survival) · SERVER_NAME (the host's name)
# Saves land in /data (user://): net_<seed>_g<gen>.ops (the edit journal), .chunks (the edited chunks, ADR 0169), _blocks.json, _ents.json and _players.json.
exec /opt/godot/godot --headless --path /game -s res://scripts/net/server_main.gd -- server=1 \
  "seed=${SEED:-1337}" "port=${PORT:-7777}" "gamemode=${GAMEMODE:-survival}" "server_name=${SERVER_NAME:-$(hostname)}" client_dir=/data/client "$@"
