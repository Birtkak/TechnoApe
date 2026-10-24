# ADR 0043: Held right clicks, ground pickups as whole pieces

Status: accepted (2026-10-24)

## Context
The developer, after playing: sticks and stones on the ground should pop from the world into an
item lying on the ground after holding right click on them for a second; the same for putting food
on and off campfires; and breaking one voxel of a stick should pop the whole stick, because now it
is "very unclear and messy".

Why it was messy: ground pickups (ADR 0019; sticks as `FineGen::STICK` stamps of 8-12 twig voxels,
stones as small patches) are walk-through plain voxels, and walk-through voxels had collision mask 0,
so the aim ray went straight through them. A stick was only ever dug as part of the brush around
the ground voxel under it: part of it went, the rest stayed, and each twig voxel counted as 1/64 of
a stick, so most digs gave nothing.

## Decision
1. **Pieces.** A ground pickup is a piece: the touching voxels (26 neighbours) of one pickup material,
   at most 64 and within 14 voxels (`GroundPickups.piece`). A dig that touches any voxel of a piece
   takes all of it (`GroundPickups.complete` on the brush selection), and each piece drops its item
   once (`MiningTable.roll_drops`), popping out where it lay instead of at the dig's centre.
2. **Aimable.** Pickup voxels get an aim-only collision box (`GameMaterials.MASK_TARGET`, like work
   spots): the aim ray stops on them, bodies still walk through. `World.raycast` takes a mask; the
   third-person camera and thrown spears pass `VoxelBody.MASK_SOLID` and ignore sticks and work spots.
3. **Held right clicks.** Right click held `Settings.hold_use_s` (1 s) on a pickup (with an empty
   hand, a tool, or the pickup's own item) pops the piece into an item on the ground
   (`Main.pop_pickup`); walking over it picks it up. On a firepit, every move that changes something
   is held: laying fuel, food, ore, ingots or a vessel on, taking things off (they pop out beside the
   fire, toward the player). Holding on keeps going, one thing a second. The firestarter still lights
   at once, and a right click that would do nothing answers at once with its message
   (`FirepitEntity.use(..., dry)` decides). The mining bar shows the hold. `WorkSystem.hold_key` /
   `hold_done` say what is held and finish it; `Player._update_hold` times it.

## Consequences
- Sticks give one stick each, whole, and never leave half sticks behind; pebbles, flint and ore-bearing
  stones the same.
- The firepit is slower to load (a second a log); that is the point (Vintage Story's feel), and
  holding keeps it flowing.
- Other work pieces (ground storage, assembly, the anvil) stay instant clicks for now.
- `craft_test=1` checks it: four logs on in one 4 s hold, meat that needs the hold, the cooked meat
  popping out, a stick picked up whole, a dig on one voxel taking all ten.
