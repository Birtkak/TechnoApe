# ADR 0050: A taller live zone, and pins for what still moves

Status: accepted (2026-10-25)

## Context
Bug report 2026-10-25 02-22-37: "since the water is so high up its frozen and not simulated. maybe we should have a
smaller area around the player that is always simulated, and then everything in vision". The bubble (ADR 0033) was a
wrapped 64 x 32 x 64 m window, the player at 40 % of its height: anything more than ~19 m above or ~13 m below the
player froze, and whatever still moved when the window walked away froze where it was (water hanging in a shaft).

"Everything in vision" at 12.5 cm is out of reach: the view goes to 2 km, the window costs 6 bytes a voxel. RSS
measured (headless, `autopilot=1 target=river water_test=1`; frame times of these runs were not usable, other sessions
were compiling):

| window (chunks) | metres | RSS |
|---|---|---|
| 32 x 16 x 32 (old) | 64 x 32 x 64 | 1996 MB |
| 32 x 32 x 32 | 64 x 64 x 64 | 2264 MB |
| 64 x 16 x 64 | 128 x 32 x 128 | 3203 MB |

The tick's cost follows the active chunks, not the window; a bigger window costs memory and the columns to load
when it moves (ADR 0048 flight).

## Decision
The developer chose "taller + keep movers":
1. **The window is 64 m tall** (`Settings.sim_window_chunks` = 32 x 32 x 32).
2. **Pins** (`SimBubble`, `pinned`): when the window moves and chunks that the simulation changed in the last 15 ticks
   and that are still active leave it, a pinned window of 8 x 8 x 8 chunks (16 m, ~25 MB, 2 threads) opens around
   them, at most two at once, in a frame of its own (3 ms). A pin is a SimBubble that never moves: it loads its chunks
   from the terrain (the main window has just pasted them there), ticks them, pastes what changes back (on 30 % of
   the paste budget), and lets go once nothing in it moved for 60 ticks (2 s) or after 90 s. Chunks the main window
   covers are walls in a pin: when the main window moves over a pin, the pin pastes those chunks first and walls
   them off; when it moves away again, the pin loads them back. So no voxel is simulated twice.
3. **Each pin carries a data-only `VoxelViewer`** (no meshes, no collisions): the terrain keeps full-detail voxel data
   only within `render_distance_m` (24 m) of a viewer, and a pin can neither load nor paste anywhere else.
4. **The loader takes what the terrain has**: per column, the waiting chunk nearest the player's height whose terrain
   is in, grown up and down through waiting chunks that are in too; a column with nothing in waits 250 ms before it
   is tried again, so columns that will never load (above or below the data) no longer hold the loader up.
5. Native support: `SimWorld.set_chunk_loaded`, `get_active_chunks`.

## Found while measuring
- The first 64 m window loaded **nothing**: the loader copied a column's whole run of waiting chunks at once, and a
  64 m run always reaches past the terrain's 24 m of full-detail data. Point 4 fixed it.
- So the taller window gains what lies within the data: about 11 m more below the player and 5 m more above (the
  player sits at 40 % of the window), not the full 32 m. Rows beyond stay walls. Making it truly taller needs the
  terrain's full-detail data taller too (`lod_vertical_ratio` > 1 for LOD 0, or a second data viewer above the player).

## Measured (windowed, 1280 x 720, vsync off; one other Godot process was running on the machine)
- Budget run (`autopilot=1 duration=30`): p99 4.62 ms, max 5.38 ms, RSS 2294 MB, `budget_ok`; a second run had one
  18 ms frame on the render thread (bubble 0 ms) while another session's game shared the GPU.
- With a 4 m water bomb and a 3 m sand bomb, then flying on: 2 pins, pin set-up 3.2 ms, a pin's worst frame 7.2 ms
  (four pastes, before the paste share), p99 7.05 ms, max 15.34 ms, `budget_ok`.

## Consequences
- Water poured down a shaft keeps running while the player flies 30-40 m away, and finishes in its pin.
- Pins lose a cell's speed and burning state at the hand-over (the terrain paste keeps materials and fullness only).
- What leaves the window while two pins are busy still freezes, and so does anything outside the pins.

## Alternatives
- Wider window (128 m): 3.2 GB of the 4 GB budget, 4x the columns to load in flight, and still capped by the 24 m
  of full-detail data.
- Settling what leaves the window at once (drop falling cells to where they would land): cheaper, but water in
  mid-air still would not flow out across a floor.
- A sparse simulation of every chunk in view: a different kernel (hashed chunks instead of the wrapped grid), and
  full-detail data everywhere in view, which the terrain cannot hold.
