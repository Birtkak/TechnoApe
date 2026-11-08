# 0166 – The client updates itself from the server

- **Status:** accepted
- **Date:** 2026-11-08 (the developer: "the new client to be updated through the server so I do not need to
  send a new file every time we update"; "make this seamless and automatic and good looking")

## Context
Friends play the exported Windows build (exe, .pck, two extension DLLs) that the developer sends by hand
after every change. ADR 0002 wanted a manifest served from the Unraid box and an in-client check; ADR 0145
put the dedicated server there and left updates for later. An old client joining a newer server replays a
journal it may not understand: that must never happen.

## Decision
- **The build version** is a string `tools/deploy_server.sh` stamps (`UTC date-time + git hash [-dirty]`) into
  `game/build_version.txt` (gitignored, exported into the .pck, `ClientUpdate.version()`; "dev" when absent)
  and next to the exported files as `version.txt`.
- **The server hands out the client:** `client_dir=` (`/data/client` in the container; deploy copies the
  export there) is served over plain HTTP/1.1 GET on **game port + 2 (TCP 7779)** by `ClientUpdate.Host`, a
  ~70-line TCPServer polled from `Net._process`: `/manifest.json` (`version` + name, size, sha256 per file,
  hashed once at start) and the files it names, nothing else. The server's wanted version is `version.txt`
  of that folder (its own build version when there is none).
- **The gate:** hello carries the build version (`PROTO` 4, a defaulted third argument). Any mismatch is
  refused with `update <version>`; the old client never joins. Pre-4 clients get the protocol refusal.
  The probe answer carries `ver`, so the server list says "updates on join".
- **The client, automatically:** on the title screen, an exported build quietly asks the last server it
  joined (`Settings.net_address`) for its manifest (4 s timeout, nothing shown if current or away); a join
  refused with `update` does the same visibly. Only files whose sha256 differs download (usually the .pck
  and DLLs; the exe is the stock template and stays) into `update/` beside the exe, each hash checked.
  Only then: every current file is renamed to `*.old` (the rollback, kept until the next update), the new
  ones move in (a failed rename undoes the ones before it; Windows allows renaming a running exe and loaded
  DLLs), and the game restarts with its command line plus `connect=` (after a join) and `updated_from=<old>`
  (no loop when a restart lands on the same build). A styled panel shows build old → new, MB, speed and time
  left. An editor run never updates itself (it would overwrite Godot): it gets the message instead.
- **Deploy:** one command (`bash tools/deploy_server.sh`) exports, copies server and client, rebuilds and
  restarts; `client=0` deploys the server alone.

## Options considered
- Over the ENet connection: no extra port, but chunked RPCs and progress in `net.gd`, which other work owns.
- A web server in the container (busybox/python): another process to supervise for a 70-line job.
- A launcher exe: a second program to ship and keep in sync; the game can swap its own files.

## Consequences
- **The build sent by hand after this change is the last one** (the bootstrap): every later deploy reaches
  players on their next start or join.
- Trust: whoever controls the server (or the tailnet path to it) can ship code to players. Accepted, the
  server is the developer's box on a private tailnet; manifest names are checked to be plain file names.
- A client that has no route to the server keeps its build and plays single-player as before.
- An exe change (a Godot upgrade) downloads ~100 MB once; nothing else changes.
- Proof: `bash tools/update_proof.sh` (two exports, a local server serving the new one, the old exported
  client refused, updated, restarted and joined; evidence in `game/out/upd/`).
