# Creatures in the world: peaceful animals

- **Status:** design and first build, 2026-11-12 (ADR 0083). Peaceful animals only: what the hostile creatures
  are is still the developer's open call (`open-questions.md`). The walk rig and the rows are ADR 0053.
- **Later, not tonight:** combat and hurting an animal, drops (meat, hide), breeding and herds that grow,
  taming, hostile creatures, birds and fish, animals on Luna, footfall sounds on live animals, a sleep pose.

## One idea: a herd is computed until someone meets it
The planet is cut into **sites** of 64 x 64 columns on each cube face. Whether a site has a herd, of which
kind, how many and where they stand at first is a **pure function of the seed and the site** (the biome at a
hashed spot in it, `data/herds.toml`). Nothing is stored or ticked for a site nobody has come near.

When a player comes within about 96 m of a site, the site **wakes**: its herd is made from the function the
first time, or taken from the **kept delta** if it was met before. A woken herd is simulated every tick while
a player is near. When nobody has been near for 10 s it **sleeps**: its animals are frozen where they stand
and the site's record is kept. The kept delta is that record: per met site, its animals (id, kind, physics
state, mind). It is small (a few dozen bytes an animal) and grows only with the ground players have walked.
A site that sleeps is not simulated, but it **drifts** (the developer, 2026-11-13): when it wakes again, each
animal's place is computed from the time gone, not ticked. It has grazed along towards a spot hashed from the
seed, its id and the clock, within its home range and on land of its home's biome, at an eighth of its walk
speed for the time nobody saw it. Nothing jumps while seen (the drift happens as the site wakes, beyond sight),
it costs nothing while away and is the same on a replay.

## The animal
- **State:** id (u32, given in the order sites are met, so deterministic), kind (row of `data/creatures.toml`),
  the player's physics state (`phys::Player`: feet, velocity, look, up, on ground), the mind (stand, graze,
  wander, flee, sleep) with the tick it ends, the herd's home (the site's spot), a call counter.
- **Body:** the player's own collision box and gravity (`phys::step`, ADR 0007, 0024). Shortcut: every kind
  has the player's box; a box per kind when animals are larger or smaller than an ape matters.
- **Mind, each tick, deterministic** (a hash of the seed, the id and the world's tick; no shared RNG):
  - **Flee** for 4 to 6 s at the row's `run_speed`, away from a player within 10 m who comes at it faster
    than 2.5 m/s. It wakes a sleeper.
  - **Sleep** while the sun is under the horizon where it stands (lies still, does not call).
  - Else, when its mind ends: **stand** (3 to 8 s), **graze** (head down, 4 to 10 s) or **wander** (a heading,
    3 to 8 s at `walk_speed`). Farther than 24 m from home the heading is towards home, so a herd stays a herd.
  - **Cliffs and deep water:** every half second a walking animal looks 1.5 m ahead: a drop of more than 2.5 m,
    a rise of more than one block, or water over 0.7 m deep there turns it (90 degrees, either way); a one
    block step is jumped (`phys::ledge_ahead`).
  - **A call** now and then (about every 30 s while awake and not asleep): the row's `voice`, played where it
    stands by every client that sees it.
- An animal at rest on the ground is not stepped (no physics) except once a second, so a grazing herd costs
  little; a walking one is stepped like a player.

## Spawning: `data/herds.toml`
One row per herd kind: the creature, the biomes it lives in, its share of a site (the chance a site of that
biome holds it) and its size. The noise land (no map, tests) counts as grassland. Never under the sea or in a
lake: an animal whose spot is water is not made.

## On the wire
- `ToClient::Kinds(names)` once on join: the creature names, by index.
- `ToClient::Animals { seen, gone }` each tick, per client, only the deltas: an animal within 80 m that is new
  to this client, moved more than 2 cm, or changed what it does or its call count; and the ids it no longer
  sees. A view is (id, kind, feet, look, act, call). The client draws each one lerped from the view before to
  the last over one tick, so they move smoothly at the server's 20 Hz.
- Bots keep the same map and `report` lists the animals near (count, and the nearest few with kind, distance
  and act), so tests need no eyes.

## Saves, the hash and replay
- The kept delta (every met site's record and the next id) is the server's `Herds`; it is written beside the
  save as `<save>.herds` (postcard), so the save's own layout (`save.rs`, version 12) is untouched, and read
  back when a server starts from that save. A missing file is a world where no herd was met yet.
- The state hash includes every met site's animals bit for bit; a recording logs the loaded herds after the
  loaded save, so a replay from a save starts from the same animals. Everything an animal does is a function
  of the world, the players and the tick, so a replay steps the same animals the same way.

## The tick's cost
The server holds itself to a budget: at most `MAX_AWAKE` (600) animals are stepped a tick (nearest sites to
players first); beyond it, sites wait asleep. The cost of a few hundred awake animals is measured against the
50 ms tick (`sim` run, ADR 0083).

## Commands
`/spawn <creature>[:speed]` keeps the client's preview walking in place; `/spawn <creature> live` asks the
server for a real one three metres ahead, which joins the site it stands in.
