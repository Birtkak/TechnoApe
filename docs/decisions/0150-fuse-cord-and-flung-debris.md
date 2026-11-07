# ADR 0150: Fuse cord and flung blast debris

Status: accepted (2026-11-07)

## Context
Fun pass (2026-11-07): TNT (ADR 0065) is lit by hand, fire or another blast, but there is no way to
set a charge off from a distance on a timer, and a blast just removes a sphere: nothing flies.

## Decision
1. **Fuse cord is a sim material (137 `fuse_cord`), not script code.** The kernel's fire rule moves it:
   a burning cell lights one of its 6 neighbours, chosen at random, with `ignite_chance` per tick, so along
   a line the burn moves `tick_hz * ignite_chance / 6` voxels a second. `ignite_chance` is computed from
   `Settings.fuse_speed_mps` (default 0.5 m/s, at most 0.625 m/s at 30 Hz with ignite_chance 1).
   `fire_hp` 70 (2.3 s a cell) keeps the chance of a burn dying on a 10 m cord under 0.5 %.
2. **It lights only itself.** Its `temperature_of_fire` (12) is below every other burnable's
   autoignition (15 and up), and its flame is the new 138 `fuse_spark`: a still fire cell with
   temperature 0 and an 8-tick life that lights nothing. Smoke is plain smoke. Burnt cord is ash. So
   the burn is the visible spark, smoke and a grey line, and tall grass beside it stays (tested).
3. **TNT lights from a burning cord anywhere on or around its block:** `Tnt._cord_burning_at` does one
   native `ShapeTool.select` of cord cells in the block grown by a voxel and checks their burning flag
   (`SimWorld.get_flags`). The old fire check only looked at the middle 2 x 2 of each face, which a cord
   lying on the ground never touches.
4. **Laying it:** a voxel item (`fuse_cord`, cordage + sulfur + charcoal, 64 voxels = 8 m of one-voxel
   cord) placed with the brush at size 1, cells touching by faces (a diagonal of edge-touching cells
   breaks it). `/tnt fuse [length_m]` lays a charge where you look with a lit cord toward you on a stone
   bed; `/scenario fuse` runs it with the TNT kit.
5. **Flung debris (`BlastDebris`, `scripts/items/blast_debris.gd`):** every blast throws
   `min(Settings.tnt_debris_max = 24, removed / 64)` chunks of 2^3 voxels, materials weighted by the
   blast's existing drop sample, at 5-13 m/s upward and outward, at most `tnt_debris_live_max` (96) in the
   air. They fly ballistically, spin, and where they stop (or after 5 s) become loose voxels in the air
   cells there: rock as gravel, soil and grass as dirt, the rest as itself. The matter comes out of the
   ~70 % of the removed voxels that drop nothing, so it is not extra matter. Each chunk is the primed
   TNT box mesh (warmed at load) with its tile texture on a duplicate of the same material: no new
   pipeline mid-game. The atlas image is decoded once at setup.

## Considered
- Rigid pieces (`RigidPieces`) for debris: there is no native call to cut a free body with a velocity
  out of the crater, and each piece costs mesh blocks; small script chunks do the job for 24 nodes.
- A script-driven cord (walk a path per frame): exact speed, but the request preferred sim rules, and
  the sim version burns, smokes and dies in water for free.

## Consequences
- The speed is statistical: a 7 m cord takes 13.6 s against 13.75 s expected (mean of four, test);
  one cord varies about 12 %. Sim chunks beyond `sim_lod_m` (12 m) tick at half rate, so a far cord
  burns at half speed.
- Debris landing voxels go to the other players as voxel ops (Net.recording), the flight does not.
- Tests: `test/test_fuse_cord.gd` (ids and cold fire, end-to-end speed with grass beside, TNT lit on
  arrival, debris cap and loose landing). Evidence: a filmstrip and a 0.1 s burst, `/tnt fuse` in snow.
