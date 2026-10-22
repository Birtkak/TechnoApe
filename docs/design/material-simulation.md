# Material simulation – the Falling Everything model on voxels

- **Date:** 2026-10-21
- **Status:** design, decided by Brent: "use the Falling Everything engine
  logic for how our voxels behave and what states they are in, and Noita's
  way of assigning information to a pixel, but for voxels".
- **References:** Noita `materials.xml` attribute reference
  ([modding wiki](https://noita.wiki.gg/wiki/Modding:_Making_a_custom_material)),
  engine write-up ([80.lv](https://80.lv/articles/noita-a-game-based-on-falling-sand-simulation/)),
  [material categories](https://noita.wiki.gg/wiki/Material), GDC 2019 talk
  "Exploring the Tech and Design of Noita".

## 1. What Noita does (summary of the sources)

**Cell types.** Every pixel has a material; every material has a
`cell_type`: `solid`, `liquid`, `gas` or `fire`. Powders are *not* a type:
sand is a liquid with `liquid_sand="1"`. Static ground is a liquid with
`liquid_static="1"` (and sand with `liquid_sticks_to_ceiling`).

**Update rules.**
- Liquid: try down; if blocked, try left/right (sand: down, then down-diagonals;
  plain liquid: also sideways). Density decides swaps: denser cells sink
  through lighter ones.
- Gas: the inverse: up first, then sideways. Separate speeds for up,
  horizontal, down (`gas_upwards_speed`, `gas_horizontal_speed`, ...).
- Fire: picks a random neighbour and tries to ignite it if `burnable`;
  burning material loses `fire_hp` and turns into flame/smoke materials;
  water stops fire; fire on water makes steam (a reaction).
- Solid: no falling rule; destroyed only by damage (`hp`, `durability`).
  Explosions convert candidate solids into falling sand; collapses are cut
  out as rigid bodies (Box2D), each pixel remembering which body it belongs to.
- Lifetime: liquids and gases with `lifetime > 0` vanish when it runs out.

**Reactions.** A table of `<Reaction>` rows: `input_cell1`, `input_cell2`
(materials or `[tags]`), `output_cell1/2`, `probability`, optional
`req_lifetime`, `direction`, `fast_reaction`, `convert_all`, blob radius.
Tag outputs like `[meltable]_molten` map a whole group to variants.

**Per-pixel information.** Material id, colour variant, which rigid body it
belongs to and its position inside it, burning state and `fire_hp`,
remaining lifetime, stain (liquid on a solid), and a per-cell "moved" mark
for the current frame. World state outside the simulation window is frozen.

**World update.** Chunks of 64×64 with a dirty rect each; bottom-up scan so
falling columns stay consistent; checkerboard passes so neighbouring chunks
are never updated by two threads at once; a pixel may move up to 32 px into
a neighbour, hence "crosses" of ownership.

## 2. Mapping it to TechnoApe voxels

### 2.1 Material definition (data file, one entry per material)
Mirrors `materials.xml` attribute names so Noita knowledge transfers 1:1.
Stored as JSON/TRES under `data/materials/`; loaded into a flat array
indexed by material id at startup; validated by schema + tests.

```
id, name, ui_name, tags[]
cell_type: solid | liquid | gas | fire
# liquid family (powder = liquid + liquid_sand)
liquid_sand, liquid_static, liquid_gravity, liquid_flow_speed, liquid_slime,
liquid_sticks_to_ceiling, liquid_viscosity, stickyness
# gas
gas_speed, gas_upwards_speed, gas_horizontal_speed, gas_downwards_speed
# solid
hp, durability, crackability, solid_break_to_type, solid_static_type,
platform_type, slippery
# shared
density, lifetime, electrical_conductivity, stainable, liquid_stains
# fire
burnable, fire_hp, autoignition_temperature, temperature_of_fire,
on_fire_convert_to_material, on_fire_flame_material, on_fire_smoke_material,
generates_smoke, requires_oxygen
# graphics
color, texture, randomize_colors, gfx_glow
# game (TechnoApe additions)
block_item (what you get when mined), hardness_tier, conductive_heat, ...
```
Reactions: a separate list with the same fields as Noita's `<Reaction>`
(`input_cell1/2`, `output_cell1/2`, `probability`, `req_lifetime`,
`direction`, tags with `[name]` and `[name]_suffix` outputs).

### 2.2 Per-voxel data (the "pixel struct")
Two parallel arrays per chunk (struct-of-arrays; cheap to copy, compress and
send):

| Field | Size | Purpose |
|---|---|---|
| `material` | u16 | index into the material table (65k materials; 0 = air) |
| `state` | u8 bitfield | `burning`, `wet/stain`, `moved` parity, `settled`, `rigid_member`, 3 spare |
| `aux` | u8 | per-material meaning: remaining `lifetime` for liquids/gases, `fire_hp` while burning, colour variant for solids, stain material for stained solids |

Optional later arrays, allocated only for chunks that need them:
`temperature` (u8), `rigid_body_id` (u16) + local position, `velocity`
(i8×3 for airborne particles as in Noita's flying pixels).

Memory: 4 bytes per voxel core; a 16³ chunk = 16 KB; a 128³ active region
= 8 MB. Chunks outside the bubble store only `material` + `aux` compressed.

### 2.3 Update rules on voxels (Falling Everything in 3D)
- **Scan order:** bottom-up in y so columns fall consistently (same as
  Noita); x/z order alternates per tick to avoid directional bias.
- **Powder (liquid + liquid_sand):** down → the four down-diagonals in
  random order → stop. Swaps with anything lighter (`density`) that is a
  liquid or gas. `liquid_static`: never moves until disturbed (ground);
  `liquid_sticks_to_ceiling`: static unless the cell below is air.
- **Liquid:** down → down-diagonals → lateral dispersion up to
  `liquid_flow_speed × DISP` cells along one of four directions, stopping
  at the first ledge (air below) so it falls next tick; cohesion keeps thin
  films from fragmenting; a cell resting on its own liquid with no ledge in
  reach stays still (calm pools). Denser liquids sink through lighter ones.
- **Gas:** up with probability `gas_upwards_speed/255` → sideways with
  `gas_horizontal_speed/255` → along the ceiling; `lifetime` thins clouds.
- **Fire:** a fire voxel picks a random neighbour each tick; if `burnable`
  and `temperature_of_fire ≥ autoignition_temperature` it sets `burning`;
  burning voxels lose `fire_hp` per tick, emit flame/smoke materials into
  air neighbours, and convert to `on_fire_convert_to_material` (or air) at
  0. `requires_oxygen` = needs an air neighbour. Water neighbours extinguish.
- **Solid:** static; mining/explosions reduce `hp`; when an unsupported
  solid region should fall (`solid_static_type`), it becomes a rigid body
  (later; Jolt in Godot 4.7 plays Box2D's role).
- **Reactions:** after movement, for each active voxel with reactive
  neighbours, test the reaction table (`probability` per tick); tags match
  groups; outputs replace both cells. Reaction lookup is a dense
  `[material][material]` → reaction-id table built at load time.
- **Lifetime:** `aux` counts down each tick for materials with `lifetime`.

### 2.4 Chunks, activity and threads (already proven in P5)
- 16³ chunks; per-chunk `active` (anything moved or was woken last tick),
  `dirty_mesh` (contents changed since last push), `edited` (differs from
  generator output; must be saved).
- 8 parity passes (2×2×2 checkerboard) on a persistent thread pool;
  cells may move at most `DISP < 8` voxels into a neighbour so same-pass
  chunks never touch the same cells (Noita's "crosses" in 3D).
- Activity flags are accumulated per chunk and flushed once (false sharing
  cost 4.5× in P5 v1).
- Simulation bubbles (players, machines, burning things) decide which
  chunks may be active at all; outside, chunks are frozen as-is.

### 2.4b Streaming rule (learned 2026-10-22)
A chunk whose world data has not been loaded into the simulation yet is a
**wall**, never air. Every move, dispersion scan, ignition, spawn and
reaction test checks the target chunk's loaded flag. Otherwise material at
the bubble's loading edge flows into "nothing" and is lost when the real
data arrives.

### 2.5 What the renderer gets
Changed chunks are pushed as `material` arrays into godot_voxel (blocky
mesher, model index = material id). Colour variants and burning state map
to model variants or a per-voxel colour channel later.

### 2.6 Ticks and distances (Brent's questions, 2026-10-21)
- **Fixed tick rate like Minecraft.** The material simulation runs at a fixed
  rate (`sim_tick_hz`, 60 in the P5 test, likely 20–30 shipped) on its own
  threads, independent of frame rate. Game logic (machines, crops, mobs) runs
  on a Minecraft-style 20 Hz game tick with *random ticks* for slow
  processes. Rendering interpolates nothing for voxels (cells are discrete);
  entities interpolate between ticks as usual. Server and client share the
  tick definition so deltas line up.
- **Minecraft-style chunk distances, three of them, all settings:**
  `simulation_distance` (chunks around each player/anchor that may be
  active), `render_distance` (full-detail chunks), `lod_distance` /
  `view_distance` (horizon rings). Android profile: small sim + render,
  same horizon.
- **Only visible faces are rendered.** The blocky mesher emits a face only
  where a voxel touches air or a transparent material; interior faces never
  exist. Beyond that: Godot's occlusion culling and godot_voxel's shadow
  occluders hide chunks behind terrain, and greedy meshing (merging coplanar
  faces) is the next step if triangle counts matter on Android.

## 3. Prototype plan
- P5 v4 (next): data-driven material table with the attributes above,
  density swaps, lifetime, fire + a reaction table (water + fire → steam,
  lava + water → stone), powders vs liquids vs gases by attributes only.
  Same benchmarks and filmstrips; a rule change must not regress P5 numbers.
- P6: rigid bodies for collapsing solids (Jolt), temperature grid, stains.
