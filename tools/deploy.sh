#!/usr/bin/env bash
# Puts the game on the home server (ADR 0130): `bash tools/deploy.sh` does both halves, so they match;
# `server` builds the committed source in Docker there and restarts the container, `client` uploads
# build/TechnoApe.exe with data/ as the zip the web page (http://$HOST:7780) hands out.
# HOST is the server's Tailscale address: it works from anywhere the tailnet does.
set -euo pipefail
cd "$(dirname "$0")/.."
HOST="${HOST:-100.124.84.125}"
DIR=/mnt/user/appdata/technoape
what="${1:-all}"
[[ "$what" == all || "$what" == server || "$what" == client ]] || { echo "usage: tools/deploy.sh [all|server|client]" >&2; exit 2; }
version="$(git rev-parse --short HEAD) $(date '+%Y-%m-%d %H:%M')"

if [[ "$what" != client ]]; then
  # the committed source only: no saves, bakes or build output
  git archive HEAD Cargo.toml Cargo.lock .cargo crates data deploy | ssh "root@$HOST" "rm -rf $DIR/src && mkdir -p $DIR/src && tar -xm -C $DIR/src"
  ssh "root@$HOST" "cd $DIR/src && docker build -f deploy/Dockerfile -t technoape-server . > $DIR/build.log 2>&1 || { tail -40 $DIR/build.log; exit 1; }
    docker rm -f technoape-server >/dev/null 2>&1
    docker run -d --name technoape-server --network host --restart unless-stopped \
      -v $DIR/data:/data -v /etc/localtime:/etc/localtime:ro technoape-server"
fi

if [[ "$what" != server ]]; then
  stage=out/deploy/TechnoApe
  rm -rf out/deploy && mkdir -p "$stage"
  cp build/TechnoApe.exe "$stage/"
  cp -r data "$stage/data"
  printf '@echo off\r\ncd /d "%%~dp0"\r\nstart "" TechnoApe.exe run=play onscreen=1 connect=%s:7777 name=%%USERNAME%%\r\n' "$HOST" > "$stage/Join Tower.bat"
  python -I -c "import shutil; shutil.make_archive('out/deploy/TechnoApe', 'zip', 'out/deploy', 'TechnoApe')"
  echo "$version" > out/deploy/version.txt
  ssh "root@$HOST" "mkdir -p $DIR/data/client"
  # the zip under another name first: a download in flight never gets half a file
  scp -q out/deploy/TechnoApe.zip "root@$HOST:$DIR/data/client/TechnoApe.zip.new"
  scp -q out/deploy/version.txt "root@$HOST:$DIR/data/client/version.txt"
  ssh "root@$HOST" "mv $DIR/data/client/TechnoApe.zip.new $DIR/data/client/TechnoApe.zip"
fi
echo "deployed $what ($version): http://$HOST:7780"
