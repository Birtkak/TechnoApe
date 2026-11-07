# ADR 0163: Fishing with a bite and a reel, snares and pit traps

Status: accepted (2026-11-07)

## Context
The fishing line of ADR 0059 gave a raw fish a few game minutes after a right click: no bobber, no skill, the
same fish everywhere. Fun pass row O2 asks for a real cast, a bite that depends on the fish around, a short
minigame, species by water and biome, junk from fouled water (ADR 0155), and land traps that catch animals
and the careless player. No new material ids, no generator change.

## Decision
1. **`Fishing`** (`scripts/work/fishing/fishing.gd`, a child of Main) owns the rod. StoneAge still takes the
   first right click with a `fishing` item; a line (not the net) goes to `Fishing.use_rod`. States IDLE ->
   FLYING (0.6 s arc) -> WAITING -> BITE -> REEL. The cast steps along the aim up to `fish_cast_m` (14 m) to
   the first water cell; the bobber floats on that column's real surface every frame (no water left: it ends).
   The wait is `fish_bite_max_s / (1 + n)` (n live swimmers within `fish_near_m`, x0.6..1.4, floor
   `fish_bite_min_s`). A bite dips the bobber and opens a `fish_hook_s` window; a right click sets the hook,
   missing it starts a new wait.
2. **The reel** is a tension bar: holding right click raises tension (`fish_tension_up`) and progress
   (1 / (`fish_reel_s` x pull)); letting go lowers tension; the fish surges every 0.7..1.6 s / pull by
   `fish_surge` x pull. Tension 1 snaps the line (extra wear), `fish_slack_s` at zero tension throws the hook,
   progress 1 lands it. Walking `fish_line_m` away or putting the rod away ends it. `auto` (and autopilot)
   hooks at once and reels to hold tension near 0.55, for scripted checks.
3. **Species are data** (`data/fishing.json` `species`): water `river | lake | sea` from the generator column
   (river flag, else sea when the ground is 1 m under sea level, else lake), optional biomes, weight, pull.
   Perch lands as plain `raw_fish`; the others are whole-fish items (`whole_fish` tag) that fillet into raw fish
   in the grid, so racks, salt, smokehouse and fires need no new recipes. A bobber with fouled water within a
   block lands junk (`junk`: nothing, a stick, a bone, fibre).
4. **Traps are work pieces** (`TrapEntity`, kinds `snare` and `pit_trap`, WORK_SPOT voxels: the aim ray sees
   them, bodies pass through). A snare (cordage + stick, placed like any work item) is a 0.5 m block one voxel
   high; a pit cover (4 sticks + 2 fibre) is laid into a hole the player dug, 1 m square and `pit_min_m` deep
   (`Fishing.pit_spot` finds the rim from the aimed spot), two voxels flush with the rim, so whatever walks on
   it falls in. Every 0.2 s Fishing springs armed traps: snares take land animals no wider than `snare_max_m`
   standing on the noose; pits take any land animal that dropped below the cover. The caught animal gets
   `Mob.trap_at` (no brain, x and z pinned, struggling; saved with the mobs), the trap keeps the species.
   An empty hand (or any click) on a full trap takes the animal: it dies at the player's feet with its drops.
   The player springs them too: a snare does `snare_hurt` and holds the foot `snare_hold_s`, a pit
   `pit_hurt` (stakes) on top of any fall. The sign is in the looks: a peg and noose, a patch of straw and earth
   with gaps; aimed at, the look-at panel names the trap and its state.
5. **Companions** are never caught, and `MobManager.safe_ahead` tells a tamed animal a trap ahead is unsafe,
   so the wolf walks round (ADR 0152).
6. `/fish pond|foul|traps [cam]|cast|state` lays scenes and drives a scripted cast; `fishing` in the debug JSON.

## Consequences
- Fish mobs are counted, not consumed: a catch does not remove a fish from the water.
- The pit check is a straight 1 m square; wider or ragged pits are refused with a hint.
- Wild animals do not avoid traps (that is the point); they also walk into empty pits like any 1.5 m drop.
- Trap catches and the rod state are player-local, like the movement toys: no server sync yet.
