# Grimorium: rigid bodies and breaking terrain in a 3D Noita-like (2026-10-24)

Source: Burkelbear Games, "Realistic voxel world destruction and rigidbody physics in my 3D
Noita-like Engine!" (YouTube, mWdlTZ_FoBc, 9 min; auto captions read). Grimorium is a roguelike
dungeon crawler in a custom C++ engine with a falling-sand voxel simulation, the closest public
cousin of our SimBubble (we already took its liquid fullness, P10). Asked for by the developer as
research for P12 (rigid voxel pieces, ADR 0037).

## What they do
- **Two systems side by side.** The cellular automaton does fine material behaviour (water, fire,
  sand); a classic rigid body system does the large scale (characters, broken-off terrain,
  projectiles). Same split as ours.
- **Rigid body state:** position, rotation, linear and angular velocity, mass (sum of voxel
  densities, computed when the body forms), restitution, and an inertia tensor from how the voxel
  mass is distributed. Plus game flags: health, destructible, immunities, what happens on
  destruction, active.
- **Sleeping:** a body that stops goes inactive and is skipped until a moving body or a force
  intersects its bounding box.
- **Colliders:** sphere, cylinder, capsule (the player, kept upright), box, and mesh colliders built
  from the body's voxel grid. Triangles are culled with a bounding volume hierarchy; the response
  uses the triangle normal and the relative motion.
- **Terrain contact:** intersect voxel faces with the body's triangles, but take the normal from a
  local **marching cubes** fit of the terrain, so bounces follow the slope instead of the flat face
  of a cube.
- **Island detection:** every destroyed voxel triggers a breadth-first flood fill, capped at a
  voxel count, to see whether the region around it is cut off. To keep it cheap, each 16^3 chunk
  stores flags for which of its six faces have voxels; the fill runs at chunk level across chunk
  faces and only descends to voxels in the chunks the destruction touched.
- **Forming a body:** the island leaves the grid and joins the rigid body system; its mesh comes
  from marching cubes plus simplification passes (fewer triangles, cheaper collisions). Damage to a
  body runs the same island logic inside it, so bodies split further.
- **Rubble:** an island under 8 voxels turns into its powder equivalent and goes back to the
  falling-sand simulation.
- **Projectiles** are separate: no collider, a look-ahead ray from position and velocity each
  frame, per-projectile acceleration (gravity or none, changed by spell modifiers), a local normal
  estimate for bounces.

## What it means for P12
| Their choice | Ours | Take it? |
|---|---|---|
| Capped BFS flood fill per destruction | Same, through `wood` only, early exit on ground | Already the plan |
| Chunk-level face flags to speed up the fill | Ground anchors end almost every fill within a few voxels | Later, when ground ledges and big buildings can break off (fills there get large) |
| Bodies sleep as bodies | Bodies settle back into voxels | Keep ours: one grid stays the truth for the sim, saves and the server (ADR 0037 point 10); a settled log is chopped like any voxel |
| Marching-cubes normals for terrain contact | Voxel face normals | **Yes, cheaply:** take the contact normal from the occupancy gradient of the 3x3x3 cells around the contact (the same smoothing, no triangles), so a log rolls off a slope instead of skidding on stair steps |
| Mesh colliders + BVH | Contact points on the wood surface vs the grid | Keep ours for terrain (a grid lookup beats a BVH); body vs body contacts would need it, later |
| < 8 voxels become powder | Nothing yet | **Yes:** tiny pieces skip the rigid body (wood bits drop as items, crumbs fall as cells) |
| Bodies split when damaged | Settled pieces are voxels again, so a new cut runs the normal check | Covered |
| Projectiles separate | Item and projectile code already separate | Covered |
