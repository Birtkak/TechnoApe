# 0020 – Water: shallow water on column gaps, a counted sea, tiles to the clients

- **Status:** accepted
- **Date:** 2026-11-10

## Context
Water must flow at real speed, with waves, and look smooth while it updates. It must cost nothing when it
is still, and stay affordable when much of it moves. It must count every drop exactly, because the water
cycle (ADR 0010) moves water around the planet and must neither lose nor make any. The dropped Godot game
lagged with lots of water, looked jumpy, and flowed too slowly.

Taste calls by the developer:
- momentum and waves;
- a generated sea now, basin lakes later with the biomes;
- splashes as client particles now, ADR 0002's 12.5 cm cells later with the fine-cell step;
- a bucket plus simple swimming. Creative pours freely but is counted. No endless sources.

## Options considered
Measured in `prototypes/liquids/`:
1. **A voxel automaton.** Every 0.5 m cell holds a fill level; water falls, shares sideways, and is pushed up
   by pressure.
   - Pros: simple; each cell is cheap (9.4 ns per wet cell step).
   - Cons: no momentum, so it flows slow and sticky (the old game's complaint). Deep water is many cells.
     Floats drift, so nothing is counted exactly.
2. **Shallow water on the columns' air runs (gaps).** One node per gap holding water, a velocity per link.
   - Pros: real speeds (a dam break, Torricelli's drain) and waves. Cost follows area, not depth. Integer
     units make the count exact.
   - Cons: water in a gap lies at its bottom, so a fall inside a column lands at once. Hole flow and closed
     tunnels need their own rules.
3. **SPH or other particles.** Too costly for a planet, and not exact.

## Decision
**Option 2, in `crates/engine/src/liquid.rs`. The server simulates; clients show it.**
- **Model.**
  - Water in integer units: 4096 units are one block (`U`).
  - Links carry velocities (virtual pipes). Speed carries on from link to link. Friction, a thin-sheet
    drag, and a stop speed slow it down.
  - A fall into a gap passes at most Torricelli's flow.
  - A full gap under a ceiling holds a little extra under pressure (1/16 block per block of head), so closed
    tunnels pass pressure on.
  - A step is 1/60 s; the server runs as many steps as the physics does.
- **Activity.**
  - Only awake nodes are stepped. A node sleeps after 30 quiet steps.
  - Every 15 steps, a calm body is laid exactly flat (a binary search for its level, every unit kept) and
    put to sleep. This fixes tilted lakes and one-unit ping-pong.
- **The sea.**
  - Generated from the seed: every column whose ground is under the sea level (4 m under the planet's
    radius) holds water up to it. The moon has none.
  - A column is stored only once water near it moves.
  - The edge of the stored region is open sea. A stored column next to unstored sea, within 1.5 cm of sea
    level, is held at sea level, and the difference goes to a counted reservoir (`sea_extra`).
  - A calm body touching the open sea is laid at sea level. Columns that are as generation gives them are
    forgotten again.
  - Measured: a bucket in the sea stores at most 93 columns, and none after 18 s.
  - shortcut: the sea's level does not rise with what it takes in; a whole sea holds millions of blocks.
    Revisit when the water cycle moves sea water.
- **Conservation.**
  - `counted() = stored − generated + sea_extra` changes only when water is carried in or out.
  - The server's ledger adds the buckets and what creative made or removed. It must be 0, and the soak
    checks it every tick.
  - The save keeps stored columns, `sea_extra` and the ledger (save version 2; version 1 still loads).
- **Budget.**
  - At most 60 000 node-steps per tick (about 7 ms). Past that, water slows down instead of the server
    lagging. The count of skipped steps goes on the stats line.
- **Network.**
  - Water goes out in tiles of 16×16 columns, each sent whole. A tile is sent when it changed by more than
    a dead band (1/64 block), or settled, since this client last had it.
  - Tiles go to a client within 128 m, nearest first, about 16 KB per tick.
  - Settled tiles reach a client exactly; the soak compares them with every bot.
  - shortcut: far floods are not sent, so they are not seen from orbit. A coarse far copy comes when views
    from orbit need it.
- **Rendering (client).**
  - The generated sea is the terrain chunks' trailing quads at the exact sea level, so it gets the terrain's
    LOD and streaming for free.
  - Tiles holding stored water are left out of those quads (down to level 4) and drawn by the water mesh
    (`crates/client/src/water.rs`): a top per column with corners shared with wet neighbours (never below
    its floor), and sides over lower water or floors (fronts, falls).
  - Heights move from the last state to the new one over one tick on the GPU.
  - Droplets fly off sides that drop over half a block and splash where they land.
  - Under water the screen gets a blue veil.
- **Play.**
  - B takes the bucket: left click scoops up to a block, right click pours it.
  - Creative pours and scoops a full block every time, counted.
  - Swimming in `phys::step`: buoyancy holds a body 80% under water, drag, jump held swims up at 2 m/s, and
    walking is slower by how deep you are.

## Consequences
- Water that is still costs nothing, and the sea costs nothing until someone touches it.
- Rain, steam, pumps and pipes can add and remove water through the same ledger.
- The soak is a strong check: 50 bots digging 40 m holes on the coast flood up to 69k columns. The water
  slows down under the budget, the ledger stays at 0, a save and a load change nothing, and every bot's
  settled tiles equal the server's. It costs 24 s per simulated minute (was 150 s before tiles).
- Known limits:
  - falls inside a column land at once (the particles cover it);
  - waves in deep water run at most at the speed of 8 m deep water;
  - a cube edge has a seam in the water mesh;
  - one thread for the simulation (parallel passes are the next lever);
  - the dead band means a client's moving water can be up to 1/64 block off until it settles.
- Later: basin lakes with the biomes, the 12.5 cm splash cells, a far water copy, fog under water.
