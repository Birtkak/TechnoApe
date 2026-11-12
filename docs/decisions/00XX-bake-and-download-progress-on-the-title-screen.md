# 00XX – Bake and download progress on the title screen

- **Status:** accepted
- **Date:** 2026-11-12

## Context
W2's open point (worldgen-plan.md): making a new world bakes its planet (ADR 0043, about 30 s for the 4 km
planet) and a client joining a server that lacks the map downloads it in pieces; neither showed anything.
Worse, both ran in `main()` before the game's window existed, so the title window sat frozen on the hand-over
wait (ADR 0038) while a new world baked. The bake's code is being changed by another session (rivers), so
the bake may only gain thin calls.

## Options considered
1. **An `Arc<Progress>` passed down as an argument** – explicit, but every bake function's signature
   changes, a hard merge with the rivers work.
2. **A global progress static** – one line per call, but two bakes at once (tests run in parallel; a server
   and a title) would write over each other.
3. **A thread-local sink set around the work** (`bake::reporting(p, || ...)`), written by the existing
   `lap` closures, the erosion's and the air model's step loops, and `fetch_map`'s piece loop – a line at
   each, no signature changes, no cost when nobody listens.
- For the window: (a) open the window first and load behind it – the whole `main()` (server, link, world,
  streamer, App) would move; (b) **do the long work in the title process** that already has a window, on a
  worker thread, and start the game again once the map is kept on disk.

## Decision
Option 3 with (b).
- `engine::bake::Progress`: a `Mutex<(&'static str, f32)>`; the fraction never falls. `STAGES` names the
  bake's laps in order with each one's seconds in a 4 km bake, so the bar moves at the bake's pace; the two
  erosions and the air model also report their step within the stage. A lap of a name not in the table is
  passed over (the rivers session may add laps: the bar stays monotonic, only less even). `reporting` ends at 1.
- `net::fetch_map` reports `DOWNLOAD` with pieces received / total.
- Client: `menu::Loading` (a worker thread below normal priority, its `Progress`, the arguments to start
  again with). The new world page's Create bakes there when `bake::file(seed, planet)` is missing, else starts
  at once. A `connect=` play run whose map is not kept starts the title screen with `join=host:port` (and every
  other argument it had), which fetches the map under the bar and then starts the game with `connect=`
  and `joined=1`. The frame polls `is_finished` and
  draws the stage and a bar at the foot of the screen; while it runs the menu answers only Quit.

## Consequences
- Tests: `bake::tests::a_bake_reports_progress_and_makes_the_same_map` (monotonic, ends at 1, at least five
  stages, the bar moves inside the erosion, the same hash as a bake that reports nothing),
  `net::tests::a_map_download_reports_its_pieces` (each piece's received / total, ends at 1).
- Seed 7, 4 km planet, release, `Map::bake`: 29.9 s before, 27.6 s after (run-to-run noise); the hashes of
  seed 7 / 4 km (6b835875f1871d10) and seed 3 / 256 (1bf96be37cccebe6) are unchanged.
- A filmstrip of the bar mid-bake (a fresh clock seed, 4 km, from a worktree with its own `saves/`, ended by
  `seconds=14` before the bake finished, so nothing was kept): `technoape.exe run=play onscreen=0 title=1
  save=0 record=0 planet=768 seed=1 settings=out/bar-settings.txt acts=1:next,1.3:go,1.6:next,1.9:go,
  2.2:next,...(nine nexts)...,3.3:go capture_ms=1500 seconds=14 sheet=out/bake-bar.png`: the bar grows
  frame by frame while the menu draws on.
- Review (principle 12): a fresh reviewer read 041ba10. The bake side was sound; the `connect=` path had
  six findings, each fixed in the follow-up commit:
  1. `join=` got the raw `connect` value and `Loading::join` connected without the default port, so the
     documented `connect=host` failed. `menu::server_addr` now adds `:7777` once, for both uses.
  2. A map that could not be written sent the joining run back to the title, round and round. The title now
     checks `bake::file` after the download and shows an error note when the file is missing, and the run it
     starts carries `joined=1`, which never starts a title again (it fetches in place if it must).
  3. Only `TITLE_ARGS`, `join=` and `name=` survived the hand-overs (`mode`, `seconds`, `settings`,
     `threads`, ... were lost). `menu::join_args` / `joined_args` pass every argument through, `connect`
     as `join` and back (`menu::a_join_keeps_every_argument_and_the_port`).
  4. A `join=` title built the 4 km seed-1337 world before the download started. The join now starts first,
     and the title's own world is the 768 planet with no save.
  5. The bar read "Making the world: grid 0%" while connecting. It now says "Joining the server: connecting",
     then `downloading the map`.
  6. A failed assert in the download test's server thread left the client waiting. The server thread now
     only records, the test asserts after it, and one 5 s deadline covers the whole download (a wrong
     report fails in 5.4 s, checked by breaking the report on purpose).
  Also: the join now goes through the title from any play run, not only on screen.
  The join was run for real (a `technoape-server` on 127.0.0.1, planet 768, its map file removed from the
  client's bakes folder; the client `run=play onscreen=0 connect=127.0.0.1 seconds=6 settings=out/x.txt`):
  probe, title, download, kept, joined (`tester joined`), the run ended by `seconds`, no `settings.txt` written.
  The bar itself was not filmed there: a 276 KB map over loopback arrives before the title's first frame.
- A saved world whose map is missing (deleted, or a new map version) still bakes in `main()` with no bar;
  so does the title's own background world on a first start. A `connect=` join that needs the map connects
  three times (the probe, the title's download, then the game). The stage weights are one machine's 4 km bake; other sizes move a little
  unevenly.
