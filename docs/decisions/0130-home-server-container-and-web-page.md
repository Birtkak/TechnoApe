# 0130 – The home server's container and its web page

- **Status:** accepted
- **Date:** 2026-11-14

## Context
The container on the home server still ran the dropped Godot game. The developer wants the Rust server there,
a page that shows its terminal output, and a download of the newest build for friends to play-test, all on
Tailscale addresses so it works from anywhere (ADR 0005).

## Options considered
1. **The page inside `crates/server`** – one process, but HTTP in the game server, and every page change is an
   engine-tier change.
2. **A second container** (nginx and a log file) – two things to deploy, and a log file that grows.
3. **A small wrapper as the container's command** – `deploy/web.py` (Python's standard library) starts the
   server, passes its output on and serves the page. The server's code does not change.

## Decision
Option 3. `deploy/Dockerfile` builds `technoape-server` from the committed source on the box itself (release,
cargo caches kept between builds) and runs it under `deploy/web.py`, host network, world and bake in the
`/data` volume. The page on port 7780 shows the last 5000 lines of output, the 5 s stats line apart as a
status, and hands out `/data/client/TechnoApe.zip`: the client, `data/` and a `Join Tower.bat` that
connects to the server's Tailscale address. `bash tools/deploy.sh [all|server|client]` does it; `all` keeps
client and server on one commit. `python -I deploy/web_test.py` checks the page.

**Second round (the same day):**
- **The friends' client** is its own build (`target/friends`): `target-cpu=x86-64-v3` (any CPU with AVX2, 2013 on)
  and the C runtime linked in, so nothing is installed first. `build/TechnoApe.exe` stays this PC's own.
- **The client updates itself in its launcher**, not in the game: `Join Tower.bat` compares the page's
  `version.txt` with its own and, when they differ, takes the zip and unpacks it over its folder (Windows' own
  curl and tar) before it joins. An updater inside the game would need HTTP in the client and a title-screen
  step; the game has no other way to join a server than the launcher's `connect=` yet.
- **A clean stop:** a line `stop` on the server's stdin saves and ends it; the end of stdin does not. The
  wrapper writes it on `docker stop` (30 s allowed). No signal crate, and the same word works typed in a terminal.
- The container carries Unraid's WebUI label, so its menu opens the page.

## Consequences
- The client needs a GPU with hardware ray queries on Vulkan (ADR 0022): no build setting widens that.
- No sign-in: whoever reaches the box on the tailnet or the LAN reads the log and downloads the build.
- The log is in memory: a restart empties the page (`docker logs` has the same lines).
- The launcher updates; a game started by its exe alone does not, and nothing yet refuses a client older than
  the server.
