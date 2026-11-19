# 0083 – Peaceful animals in the world

- **Status:** accepted
- **Date:** 2026-11-13

## Context
ADR 0027 gave Terra animals and hostile creatures, and ADR 0053 gave creatures their rows and walk rig. Until
now nothing lived in the world: `/spawn` was only one player's preview. Which creatures are hostile is still
the developer's open call (`open-questions.md`), so this ADR covers peaceful animals only. The design is
`docs/design/creatures.md`.

## Options considered
1. Every animal simulated and stored all the time. Pros: time passes everywhere. Cons: a tick and a save
   that grow with the planet.
2. Herds are a pure function of the seed per site until a player comes near, then simulated and kept as a
   delta, and not ticked when nobody is near (in the spirit of `clutter-and-pickups.md`). Pros: nothing is
   stored or ticked for unmet ground, and the cost follows the players. Cons: a herd does not live while
   nobody sees it (answered by the drift below).
3. The client simulates its own animals. Cons: players would disagree, which breaks principle 7.

## Decision
Option 2, server-authoritative.

- **`engine::herds`.** A site is 64 x 64 columns of a face. The biome at a hashed spot in the site picks a
  row of `data/herds.toml` (creature, biomes, share, size; the noise land counts as grassland). It makes
  1 to 12 animals around the spot, never on water.
- **Waking.** A site wakes when a player is within about 96 m (25 sample points) and sleeps 200 ticks after
  the last one left. Every met site is kept, with its animals' `phys::Player`, mind, heading and call
  count, plus the next id and the herds' own clock.
- **Drift while away** (the developer's call, 2026-11-13). A sleeping site is not ticked. When it wakes again,
  each animal is found where the time gone took it, computed in `drift`: it has walked at an eighth of its
  walk speed for the time unsimulated (the clock's ticks gone, less the `SLEEP_TICKS` it ran on) towards a
  spot within `LEASH` of home, hashed from the seed, its id and the clock. The spot must be land (`stand`,
  not water) of the home spot's biome; after four refused tries it stays. It wakes standing, its mind
  Stand. No cost while away, the same on replay, and a site woken within `SLEEP_TICKS` does not drift.
- **Body.** The player's box, gravity and collision (`phys::step`). An animal at rest is stepped once a
  second. One trapped by a placed block climbs out on top.
- **Mind.** Every choice comes from a hash of the seed, the id and the herds' clock:
  - It flees for 4 to 6 s at `run_speed` from a player within 10 m who comes at it faster than 2.5 m/s.
  - It sleeps while the sun is under its horizon.
  - Otherwise it stands (3 to 8 s), grazes (4 to 10 s) or wanders at `walk_speed`, turning for home when
    more than 24 m from its herd's spot.
  - Every 4 ticks it looks ahead 1.5 m or more. A drop over 2.5 m, a rise over a block, or water over
    0.7 m deep turns it 90 degrees (left, right or back, else it stands). A one-block step is jumped.
  - It calls about every 30 s.
- **The herds' clock.** Minds and the look-ahead count the herds' own ticks, never the world's age, which
  `/timewarp` runs faster.
- **Cost cap.** At most `MAX_AWAKE` (600) animals are stepped a tick. Over the cap, each tick starts at
  another awake site.
- **Wire.**
  - `ToClient::Kinds` (names) is sent on join.
  - `ToClient::Animals { seen, gone }` carries, per client, only the changes within 80 m: new to it, moved
    over 2 cm, or a change of act or call count.
  - The client draws each animal lerped over one tick with the clips `a.idle`, `a.eat`, `a.walk`, `a.run`
    (made from its row for every creature model) and plays the row's `voice` on a new call.
  - Bots keep the same map, and `report` gains `animals` (how many it sees, and the nearest five with
    kind, distance and act).
- **Saves, hash and replay.**
  - The herds are written beside the save as `<save>.herds` (postcard of the next id, the clock, the kind
    names and the sites), so the save's own layout (version 12) is untouched. A kind is matched back by
    name, and a file naming a kind no longer in `data/creatures.toml` is refused.
  - `Server::read_herds` reads the file where a save is loaded (the client and the dedicated server).
  - Removing a world in the menu moves its `.herds` along.
  - The state hash includes the herds, and a recording logs `Ev::Herds` after `Ev::Loaded`.
- **`/spawn`.** `/spawn <creature>` stays the client's preview. `/spawn <creature> live` asks the server for
  a real one three metres ahead, in the site it stands in.
- **Data.**
  - Deer (`data/models/deer.toml`) and hare (`data/models/hare.toml`) are new quadruped drafts from palette
    colours, with rows in `data/creatures.toml` and voices `deer.call` and `hare.squeak` in
    `data/sounds.toml`.
  - Herds by biome: grassland deer and hare; temperate forest deer and boar; cold forest deer and hare;
    savanna deer and boar; dry scrub hare and boar; tundra and alpine hare; rain forest and wetland boar.

## Evidence
- **Engine tests, seen red then green.**
  - `a_player_running_at_one_makes_it_flee_away` and `a_herd_by_a_pit_never_falls_in` failed with the flee
    and the look-ahead switched off (the mind stayed Stand; one animal fell 20 m).
  - The kind test in `nobody_near_nothing_is_made_or_stepped_and_a_met_herd_keeps` failed with the remap by
    name switched off.
  - Also: the herds come from the seed alone and stand on land, they graze and wander by day and sleep at
    night, nobody near means nothing is made or stepped, and the files are refused by name.
- **Server test** `animals_reach_the_player_near_them_and_keep_beside_the_save`. A deer made by command
  reaches the player as a delta and goes when the player leaves. It is kept beside the save, and a recording
  from that save replays with no divergence. Without the logged herds that replay diverged at tick 20. The
  herds' clock counts one a tick under `/timewarp 100`.
- **Soak.** `technoape-server soak=1 bots=50 minutes=2 seed=1` passed with the new invariant (no animal
  inside the ground or non-finite). 58 animals in 96 sites were awake throughout; hash 5e43f1bbdf2297de.
  The soak's replay test (`a_replay_meets_every_checkpoint_of_its_recording`) passes with the herds in the
  hash.
- **Drift** `a_herd_left_alone_is_found_moved_inside_its_range_the_same_each_run`, seen red (0 of 15
  moved) then green. Left five minutes, at least three in four animals are found over 1 m away, all within
  `LEASH` of home, in air and off water; a second run is byte for byte the same, and 20 s away moves them
  less than 5 min.
- **Tick cost** (`herds_tick_cost`, release, ignored test). 29 players on the 4 km planet woke 244 animals,
  62 of them walking: 1.11 ms mean, 1.74 ms p99, 4.0 ms max of the 50 ms tick, on one thread. That is about
  5 us an animal, including waking. A `run=stand` beside a deer herd: `tick_ms_p99` 0.46 ms; frame p99
  4.38 ms, `budget_ok`.
- **Filmstrip** `out/herd.png`, read: `run=stand at=0.08241,0.84893,0.52205 eye=9 pitch=-40 capture_ms=1500
  sheet=out/herd.png` (seed 1337, 4 km planet, 37 m from the spawn). A deer herd grazes and walks on a
  lake shore by day, upright on the ground, and never into the water; `deer.call` played 12 times.

## Review (a fresh reviewer of the whole diff)
Found and fixed:
1. The look-ahead `% 4`, the rest check `% 20` and the mind timers counted the world's age, which jumps by
   the warp: at warp 20 or more, 3 of 4 walkers never looked ahead. The herds now keep their own clock,
   saved with them.
2. The sidecar stored kinds by row index, so a moved row turned deer into boars. It now names the kinds and
   maps them back (test seen red).
3. Removing a world left its `.herds` behind. It moves with the save now.
4. Over the cap the same sites were always starved. Each tick now starts at another site.
5. The gone-list check was quadratic; it uses a set now.
6. `add` took an id before it could fail. Fixed.

Answered:
- Met sites only grow, empty ones too, and a teleporting client can wake 25 a tick. The planet bounds this
  (about 55k sites on the 4 km one); it is marked `shortcut:` in `Herds::to_bytes`.
- An animal trapped under water stays trapped. This is rare, and the soak's invariant would name it.
- A crash between writing `.sav` and `.herds` leaves them unpaired. The climb-out absorbs mismatched
  ground.

## Review of the drift (a fresh reviewer of its diff)
No panic, underflow or non-determinism found (the drift reads only the seed, the id, the herds' clock, the
animal's place and the world; `add` wakes on the same clock). Answered:
- The drift places an animal by the first air over the ground, with no path: a herd fenced in or roofed over
  can be found outside the fence or inside the room. Later, with pens (a path or "enclosed" rule).
- An animal far out of range when its site slept (one that fled) heads home at the drift's pace, so after a
  short absence it is still out of range; and the range is measured on home's tangent plane, so on a slope
  the straight distance can pass `LEASH` a little. The test's `LEASH + 3 m` holds for its seed.
- The test's second run is in one process, and the test world is the noise land, where every spot is
  grassland, so the biome rule is not exercised there; the server's replay test covers the hash.

## Consequences
- **Later:**
  - combat and hurting an animal, drops, breeding, taming and hostile creatures;
  - birds and fish, and animals on Luna;
  - footfall sounds on live animals, and a sleep pose (a sleeper plays `a.idle`);
  - a collision box per kind (every animal has the player's);
  - a herd's minds living on while nobody sees it (the drift moves it, but it wakes standing, by day or
    night);
  - culling met sites the seed makes again empty.
- **Shortcuts:**
  - The server takes `data/creatures.toml` and `data/herds.toml` compiled in, so a change needs a rebuild.
  - The client reads `data/creatures.toml` on join for the models, voices and scales.
  - `send_animals` scans every awake animal per client, which is fine at hundreds; filter by the client's
    sites when there are thousands.
- **CPU cost:** about 5 us an awake animal a tick on the server; on the client one model instance per
  animal within 128 m.

## Calls for the developer to confirm
- `/spawn <creature> live` for a real animal: the word `live`, and that anyone may run it.
- Herds drift while nobody is near (decided by the developer, 2026-11-13). Still to confirm: the pace (an
  eighth of the walk speed) and that the drift happens even across nights.
- The kinds per biome, the shares (0.3, 0.3, 0.25 a site) and the herd sizes.
- Deer and hare look (drafts), deer run at 7 m/s and hare at 6, and the voices.
- Flee distance 10 m and speed 2.5 m/s (walking straight at one scares it).
- No sleep pose: a sleeping animal stands idle.
