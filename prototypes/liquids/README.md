# Water spike: which liquid model carries the whole game

**Question.** Which water model gives real speeds and waves, stays cheap when much of it moves, counts every
drop exactly (the water cycle, ADR 0010), and costs nothing when it is still? Earlier attempts (the dropped
game) lagged with lots of water, looked jumpy while it updated, and flowed unrealistically slowly.
Answer taken in ADR 0020.

**Answer.** Use **shallow water on the columns' air runs** (`crates/engine/src/liquid.rs`):
- **One node per air run.** A column of 0.5 m blocks is split by solid blocks into air runs (gaps). Each gap
  holding water is one node, however deep, with its water in integer units (4096 per block).
- **A velocity per link.** Neighbouring gaps that overlap are linked, and each link carries a velocity (the
  virtual pipe model). A height difference speeds it up, friction slows it down, and water carries its speed
  on into the next link.
- **Rules on top:**
  - A gap that water falls into passes at most Torricelli's flow.
  - Calm bodies are laid exactly flat and put to sleep.
  - Untouched sea is not stored at all.

The alternative measured here is a **voxel cellular automaton**: every 0.5 m cell holds a fill level, and each
step water falls, shares sideways and is pushed up by pressure (the "compressible" CA, Minecraft- and
Terraria-like).

## Measured (`cargo run --release` in this folder, Ryzen 5 5600X, one thread)

| scene | columns (chosen) | voxel automaton | real water |
|---|---|---|---|
| dam break, 2 m deep: front after 0.5 / 1 / 2 s | 3.0 / 5.5 / 10.0 m | 4.0 / 6.0 / 8.0 m | Ritter: 8.9 m/s at the very tip |
| after the front | the bore hits the far wall, returns, dies down; asleep after 54 s, exactly level | spreads and levels, no bore, no slosh | as the columns |
| 4 m tank over a 0.5 m floor hole: first second | 17.3 blocks | - | Torricelli 17.7 |
| the same tank: time to empty | 24.6 s | - | Torricelli 22.6 s |
| water created or lost | 0 units, every scene | float drift | 0 |
| cost | 115 ns per moving column-step | 9.4 ns per wet cell-step, 4 cells per column here | |
| 64 m x 64 m basin, 2 m deep, a 3 m surge dropped in | 16384 columns moving: 1.9 ms a step; asleep after 32 s; level to the unit | 16 m of water would be 32 cells a column, every one visited until all are still | |
| 4 blocks poured into a 16 m deep generated sea | 4127 columns stored, asleep after 15 s, 77 ms of work in all | the whole sea is cells | |

What decided it:
- **The cost follows area, not volume.**
  - **Deep water is cheap.** A deep sea column is one node, but 32 cells in the automaton. Still water costs
    nothing.
  - **Untouched sea costs nothing at all.** It is generated from the seed, like terrain, and only the
    columns a disturbance reaches get stored.
- **Momentum.** Real surges, bores, sloshing and Torricelli's drain rate come from the velocities. The
  automaton has none, which makes it look slow and sticky.
- **Exact counting.** Volume moves in whole integer units, so the total is exact by construction. That is
  what the water cycle needs.
- **Settling is solved, not hoped for.** Friction alone left lakes tilted after a slow fill and let the last
  millimetres creep or ping-pong one unit for minutes. Every 15 steps, a calm body (all flow under 6 cm/s,
  level within 1.5 cm) gets its one exact level, every unit kept, and sleeps.

What the columns cost us (in ADR 0020 as known limits):
- **Water in a gap always lies at its bottom.** Falling water inside one column lands at once. Waterfalls
  and pouring look right only with the client's splash particles.
- **Hole flow needed its own rule.** A hole in a floor first drained 9x too fast, because the whole side of
  the shaft was open to the neighbours. The fall cap (Torricelli, one block wide) fixed it: 17.3 against 17.7.
- **Pressure under a ceiling.** A full gap under a ceiling holds a little extra water under pressure (1/16
  block per block of head), so that closed tunnels pass pressure on (the U-tube test). A tunnel under a deep
  lake hides about 10% extra water. It is counted exactly, but it is not visible.
- **Deep-water waves are slow.** They run at most at the speed of 8 m deep water (ACAP), which keeps a
  1/60 s step stable.
- **One thread for now.** 16k moving columns is 1.9 ms a step, 5.7 ms a 20 Hz tick. The server caps
  liquid work per tick (water slows down instead of the game lagging), and parallel passes are the next lever.

## Files
- `src/main.rs`: the scenes and the voxel automaton baseline; one JSON line per scene.
- The model and its tests: `crates/engine/src/liquid.rs` (`cargo test -p engine liquid`).
