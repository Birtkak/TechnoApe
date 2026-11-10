# 0011 – Test harness I: soak, state hash, flight recorder, one gate

- **Status:** accepted
- **Date:** 2026-11-10

## Context
`docs/design/fundamentals.md` section 4 asks for steps 1-3 of the testing ladder before digging, saves and
water land: invariants checked by a fast-forward soak of random bots, a state hash per tick, and a flight
recorder whose recordings replay headless. Plus one gate, `tools/check.sh`. Each tool is picked here.

## Options considered
1. `proptest` for random cases and shrinking – strong shrinking, but it shrinks inputs it generates itself,
   and our cases are whole multi-bot sessions; a dependency for a 15-line loop.
2. Own seeded bots and a greedy shrink – each bot draws from its own splitmix64 stream, so dropping one bot
   leaves the others' choices unchanged; shrinking drops bots while the run still fails.
3. Hash with `std::hash::DefaultHasher` – not promised stable across Rust releases, so stored hashes could rot.
4. FNV-1a, a few lines – stable forever, fast enough for a hash a second.

## Decision
Options 2 and 4.
- **Soak** (`crates/server/src/soak.rs`, `technoape-server soak=1 bots=50 minutes=10 seed=`): bots in the same
  process as the server on in-memory links, no sleeping; each tick each bot may change its walk, teleport
  (half the teleports within two columns of a face edge, an eighth at a corner, a tenth on the moon), dig or
  place within 3 m. After every tick: every player finite, none under the core, no walking player inside a
  block; every 100 ticks every bot's copy of the world equals the server's, chunk by chunk. A failure is
  shrunk to the fewest bots, recorded to `out/soak-fail.rec` and printed with its repro line. 50 bots x 10
  simulated minutes take about 45 s on the reference machine.
- **State hash:** `World::chunk_hashes` (one per edited chunk, so a mismatch names the chunk) and
  `Server::hash` (the world, every player's physics and input, the tick and the carried-over time).
- **Flight recorder:** `Server::record` writes every join, message and leave with its tick, and a hash every
  second, as length-prefixed postcard records, flushed every tick (the release build aborts on panic, so a
  crash loses at most its own tick). The dedicated server records by default to `out/rec/`, and so does the
  client's local server when it runs on screen; `record=0` turns it off. `technoape-server replay=file
  until=tick` replays headless, stops at the first checkpoint that disagrees and dumps the players.
- **The gate:** `bash tools/check.sh` runs `cargo test`, a 3-minute soak on a fresh seed with recording, the
  replay of that recording (determinism and the recorder at once) and the four quick benchmark runs (budget in
  all, no missing chunks in `fly`), and prints one JSON verdict.

## Consequences
- The soak found four bugs on its first runs: a block could be placed inside a player, and three ways the
  sampled collision let a player into a block (ADR 0007, revised). All fixed.
- The gate takes about 1.5 minutes, not the minute fundamentals.md asks: the quick benchmark is most of it.
- Not covered yet: saves (none exist; the hash is ready to compare a save against its load); comparing the
  quick benchmark with a stored baseline (only the budget is checked); a state dump on panic beyond the
  recording; recordings in `out/rec/` are never cleaned up.
- The soak's physics is most of its time: the speed-lever spike (roadmap) should measure it too.
