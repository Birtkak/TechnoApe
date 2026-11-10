# TechnoApe – Fundamentals: feel, expandability, performance, testing

- **Status:** living document, written 2026-11-09 from a design session. `vision.md` says what the game is;
  this says what the code must be able to do so the game can feel right, keep growing and stay fast, and how
  we find bugs before the developer does. Developer calls are in ADR 0008. Everything marked *proposal* is a
  default to measure or play before it is fixed; each system still gets its own ADR when it is built.

## 0. The numbers the planet gives us

| | standard 4 km planet (N = 12288) | huge 8 km planet (N = 24576) | moon (N = 512) |
|---|---|---|---|
| radius | 4244 m | 8488 m | 177 m |
| walk around at 4.3 m/s | 103 min | 207 min | 4 min |
| horizon at eye height | 117 m | 165 m | 24 m |
| sunset moves over the ground (20 min day) | 22 m/s | 44 m/s | – |
| speed of a surface orbit / one lap | 204 m/s / 131 s | 289 m/s / 185 s | 17 m/s at lunar gravity |
| surface | 226 km² | 905 km² | 0.4 km² |

- Nobody on foot keeps up with the sunset.
- **The planet's size stays a parameter** (ADR 0008): nothing may assume R = 4244 m. The standard planet is
  4 km and the huge one 8 km (ADR 0021); the engine must hold its budget on both.
- A player is 1.2 blocks wide and 3.6 blocks tall. A walkable tunnel is 2 x 4 blocks: 16 blocks per metre,
  against Minecraft's 2. Any feel that copies Minecraft's one-block actions is 8x the clicks.

## 1. Game feel

Feel is a list of numbers a headless run can check, not an impression. Each item below ends up as a field
in a `feel` report (section 4) next to the frame times.

**Hands (latency)**
- Look: the mouse turns the camera the same frame, never through the physics step or the server.
- Move: predicted on the client with the shared physics (done in `phys.rs`). Target: 0 server corrections in
  a clean local run; a correction is eased over about 100 ms, not snapped (done, ADR 0019: 0 in the gate).
- Dig and place: **predicted on the client the frame of the click**, the server confirms or reverts. Today an
  edit waits for the next 20 Hz tick, up to 50 ms: too slow for a building game.
- Sound starts within 20 ms of the action that caused it; the remesh after an edit shows the next frame;
  light after breaking into a cave settles within 50 ms.

**Feet (movement polish, all *proposals* to tune in the first playtest)**
- Coyote time and a jump buffer of about 100 ms each; sprint about 6.5 m/s with a slight FOV widening;
  crouch never walks off an edge (building on a sphere needs it).
- Step-up eases the camera over about 80 ms so 0.5 m stairs do not jitter; a small landing dip.
- Per-body gravity is data: the moon is floaty. Changing bodies rolls "up" over at least a second.
- Measured by camera jerk (the third derivative of the camera path) in a walk-over-hills run: steps, seams
  and corrections show up as spikes with a tick number.

**Tools (ADR 0008: brush + drag-build)**
- A dig or place action covers a 1 m group (2 x 2 x 2 blocks) by default; a held key gives single 0.5 m
  blocks; better tools carry bigger brushes. A tunnel is then 2 actions per metre, as in Minecraft.
- Building drags out lines, walls and floors in one action; the ghost shows the footprint and any seam it
  crosses before the player commits (ADR 0003).
- Protocol consequence: an edit message carries a shape (box of blocks), not one block. The server checks
  reach and inventory for the whole shape and applies it in one tick.

**Body (ADR 0008: harsh survival)**
- Needs: health, food, water, temperature, air. Survival is a main activity next to automation.
- The rule that joins the two: **every need starts as a chore and ends as a factory.** Water is carried, then
  piped; food is gathered, then grown and processed; warmth is a fire, then a heated base; air is a tank,
  then a sealed room with a machine. Each new place (night side, deep, moon) brings a need back at full
  strength.
- System need: **rooms.** Air and heat need to know enclosed volumes: a flood fill at block level, updated on
  edits, on worker threads.

**Planet (ADR 0008: the rules content is designed around)**
- **Sun and time zones:** the night side is dark and cold, the far side has another time of day. Solar power
  follows the sun. Megaproject: a power ring around the planet.
- **Finite planet:** deposits run out; the seed fixes the total. Mining scars show from orbit, so far LOD must
  draw edits and buildings, not just generated terrain. Late game needs recycling. Megaproject: a survey
  satellite that maps what is left.
- **Real orbits:** gravity falls off with distance above the surface; thrown things, players and cargo can
  orbit. The moon's rail is a real orbit for the planet's gravity, so transfers behave. A stable orbit with
  no thrust is switched to its ellipse and costs nothing per tick. Megaproject: a mass driver to the moon.
- **Megaprojects pull the ages:** each tech age ends in one planet-scale build that changes the world.
- **The planet has air** (ADR 0010): a water cycle (vented steam and evaporation return as rain, so ponds
  refill), weather, pollution and radiation, on one coarse grid for the whole planet. Detail in
  `progression.md`.
- Not a pillar: line of sight over the horizon (relays for radio or beams). Parked, not designed against.

## 2. Expandability

What must stay cheap to add: a block, an item, a recipe, a machine, a biome, a body, a simulated system, a
message, a player. The fundamentals that make that true, in the order they will bite:

1. **A registry instead of constants.** `MATERIALS: u8 = 4` in `net.rs` becomes data: blocks and items are
   named in data files, get runtime ids at load, and every save and every `Welcome` carries the name-to-id
   table. Adding content then never breaks a save or the protocol.
2. **Block = id + small state; machines are block entities.** Rotation, fill level and "this block is fine
   cells" live in the block's state; anything bigger (inventory, progress) is a sparse entity keyed by block
   position.
3. **One deterministic tick.** The simulation is a fixed list of systems run in a fixed order over the world:
   no wall clock, no iteration over unordered maps, random numbers seeded by (seed, tick, place). The same
   inputs give the same world, on the server, in a test and in a replay. This is the base of section 4.
4. **Activity, not area.** A system only visits what is awake: water that moved, a machine that has input.
   Sleeping chunks cost nothing. "The whole loaded area keeps running" then means "everything awake runs".
5. **Networks are graphs, not blocks.** Conveyors, pipes and cables are simulated as runs and nodes (a belt
   run moves all its items in one step; Factorio's transport lines), independent of which chunks are loaded.
   The planet is small enough that every factory on it runs all the time.
6. **Interest-scoped replication.** The server sends each client only the regions and entities it subscribes
   to, as generic entity deltas (not a special `Players` message), with a protocol version in `Hello`.
7. **Saves:** edits and entities per region, versioned, with the registry table, written by a worker to a
   temp file and renamed.
8. **Bodies are data:** radius (N), gravity, generator, atmosphere, orbit. More bodies is more rows.
9. **No abstraction ahead of need:** a system is a function over the world called from the tick. A trait or
   scheduler appears when the third system needs it, not before.

## 3. Performance

ADR 0004 budgets the frame. The rest of the game needs budgets too (*proposals*, to fix when measured):

| What | Budget | Checked by |
|---|---|---|
| Server tick (20 Hz) with 50 players | p99 <= 10 ms on the home server | bots, server JSON line |
| Wire, per player, walking and building | <= 20 kB/s | byte counters in the server line |
| Start to first playable frame | <= 3 s | client JSON line |
| Teleport: time until no chunk is missing | <= 500 ms | `run=stand at=` |
| Frame-thread allocations in steady state | 0 | counter, asserted |
| Huge planet (N = 24576) | ADR 0004 holds | a `huge` benchmark row |

**Where Rust's speed comes from, and what we still leave on the table**

Rust is not fast by itself; the design is (ADR 0002: binary meshing, 8-byte quads, culling). What the
language adds is that the fast design is safe to write:
- **No garbage collector**, so no pauses: the p99 and max-frame budgets are reachable at all.
- **Control of memory layout:** flat arrays of small values, no hidden boxes, no pointer chasing.
- **Data races are compile errors**, so everything can move to worker threads without months of heisenbugs.
  This is the big one for "never stall the frame thread".
- **Abstractions compile away** (generics are specialised, iterators become loops), so tidy code costs nothing.

Have already: fat LTO, one codegen unit, `panic = "abort"`, `target-cpu=native`, rayon. Levers not pulled
yet, each a one-day spike measured on `tools/bench.sh` before it is adopted:

| Lever | Why it may pay here | Cost |
|---|---|---|
| mimalloc as the global allocator | the Windows heap is slow under many threads | one line |
| a fast hasher (FxHash) for chunk-id maps | the default hasher is built against attackers, not for speed | one type alias |
| worker threads below normal priority, count = cores - 2 | workers can never preempt the frame thread on 6 cores | a few lines, Windows API |
| profile-guided optimisation with the benchmark as workload | typically 5-15% for free on release builds | a build script |
| checkerboard parallel simulation (neighbours never run together) | parallel and still deterministic (section 2.3) | part of the first sim system |
| Tracy or a built-in per-phase profiler | "why was this frame slow" as data | a dependency or 100 lines |

Rules that follow:
- **Spikes carry their cause.** A frame over budget reports its phases and what was swapped in that frame,
  so a spike in a JSON line is already half a diagnosis.
- **Count work, not just time.** Deterministic counters (cells visited, quads made, bytes sent, allocations)
  are exact and never flake; tests assert them. Time is measured by the benchmark, work by the tests.
- **Worst cases are named runs**, like `edit` today: a flood, 50 players in one spot, a fully built planet
  seen from the moon.
- **Shipped builds are not `native`.** `target-cpu=native` is right for the dev PC's numbers and wrong for
  the Linux server and other players' PCs: release artefacts use `x86-64-v3` (ADR 0005, when updates ship).

## 4. Testing: find it before the developer does

Today: `cargo test`, scripted bots, JSON runs, filmstrips read by eye. The ladder below is ordered by what
it buys per line of code. Steps 1-3 are small and belong right after movement lands, because digging, saves,
water and light are where the bugs will be.

1. **Invariants and a fast-forward soak.** The server ticks without sleeping, so 50 bots times 10 simulated
   minutes run in seconds. Seeded random bots walk, jump, dig and place while every tick checks: no player
   inside a block, nothing NaN, nothing under the core, the client's world equals the server's. Half of all
   random cases are placed on seams and corners, the 0.5% of cells where the bugs live. A failure prints its
   seed and the shortest script that still fails.
2. **A state hash per tick.** The server hashes its state (per chunk, so a mismatch says where). Three checks
   fall out: run twice and compare (determinism), compare a bot's copy with the server's (the delta
   protocol), save, load and compare (saves).
3. **A flight recorder.** Every session, the developer's included, records its inputs per tick to a small
   file. A bug seen in play is a file Claude replays headless to the exact tick, dumps and bisects; a replay
   plus its final hash is a regression test. A panic writes the last minute of inputs and a state dump.
   `git bisect run` on a replay finds the guilty commit unattended.
4. **Pictures as numbers.** Fixed viewpoints (a seam, a corner, an LOD border, a cave, orbit) are rendered
   and compared with stored images by pixel count; Claude reads an image only when a number moves. Checks
   that need no stored image: a **crack detector** (clear to a colour that exists nowhere, count its pixels
   inside the planet's outline: must be 0), a **pop detector** (frame-to-frame difference spikes in `fly`).
5. **The feel report** (section 1): corrections, camera jerk, click-to-change frames, as JSON fields.
6. **A bad wire.** The in-memory link gets delay, jitter and disconnects; prediction is tested at 200 ms.
7. **Two answers must agree.** The fast path against a slow obvious one: collision against brute force, an
   incremental remesh against a full one, an LOD chunk against the downsampled chunk, relight against full
   light.
8. **Conservation laws.** Every network has one: water volume, items on belts, power in = out + stored. A
   paranoid build checks them every tick in the soak.
9. **The work counters** of section 3 as exact performance tests.

**One gate.** `tools/check.sh` runs tests, the determinism check, a 30 s soak, the viewpoints and the quick
benchmark against the stored baseline, and prints one JSON verdict in under a minute. It is what "done"
means. **Every failure prints its own repro line** (seed, replay file, `at=` pose), so the next session
starts from evidence, not from a description.

Tools to pick when built (own ADR each): `proptest` for shrinking, a crate or 30 lines for the hash,
`cargo-mutants` now and then to see what the tests do not catch.

## Sources
- The Rust Performance Book, build configuration: https://nnethercote.github.io/perf-book/build-configuration.html
- Factorio Friday Facts #176, belts as transport lines: https://factorio.com/blog/post/fff-176
- Per-tick hashes and replay comparison (tickwise): https://docs.rs/crate/tickwise/0.2.0
- Replay determinism across frame rates: https://bugnet.io/blog/how-to-test-network-replay-determinism-across-frame-rates
