# Rendering and lighting

The ray-traced light model, the sky's mechanics as the renderer uses them, block and held light, cutout blocks,
glass, painted box models, body shadows, and the sky, shadow and hand fixes. What holds today. Numbers in brackets are
the ADRs (`archive/NNNN-*.md`) a decision came from; INDEX.md lists every number. Chunk choice and the mesher's
rules are in `streaming-and-meshing.md`; the orbits' current numbers are in `bodies-and-orbits.md`; clouds, weather
and water are in `sky-weather-and-water.md`.

## The developer's calls

- The game requires a GPU with hardware ray queries (Vulkan, wgpu `EXPERIMENTAL_RAY_QUERY`); no fallback path; the
  client refuses to start without it and says why (2026-11-10) [0022].
- Ray-traced light with the sun the only big source, and the sun a real sphere; moonlight is sunlight bounced off a
  moon that orbits (phases, lights the ground from where it is); eclipses are real blockage with a different share of
  totality at each place; reflections on metal; later telescopes; smooth coloured block light; soft but fairly sharp
  shadows [0022].
- The inertial frame is centred on the sun, so later planets and the sun have a place [0022].
- Block light is traced, not flooded: light does not creep around corners (2026-11-10) [0022].
- Bounce light is traced per pixel: light reflects and goes around corners (2026-11-10) [0022].
- Lamps get about twice the reach; Luna keeps its true orbit, so a moonless night should be rich rather than empty
  [0072].
- Snap the light to the block texture's texels, no grain (bug 2026-11-14_01-59-35) [0108, supersedes the dither of
  0098 and 0072; stays here as the developer's call].
- Lamps side by side glow as one surface, frame on the outer edge only (2026-11-13) [0085].
- Fast leaves draw open holes (2026-11-13) [0088].
- Tinted see-through glass [0081].
- Add torches; a held torch gives light; add body shadow (2026-11-14) [0117, 0119].
- The block in hand should look like the placed block; a held light source should give light (F9 2026-11-13) [0098].
- Taste calls made by default, open to the developer: the held lamp is as strong as the placed one [0098]; the
  sun-set fade is 7 degrees wide [0038, the developer answered "the sun dimming is fine"].
- The star drift when flying is left physical (13.5 degrees a second at 1 km/s); a damping setting would be a fake
  [0072].

## The sky as the renderer sees it (`engine::sky`)

Circular orbits on rails, pure functions of the world's time, the same on every machine [0022].
- World time is `tick / 20 Hz` seconds since the world was made. The save keeps the tick; clients get it from the
  server [0022].
- The inertial frame is centred on the sun; the ecliptic is its xy plane; the planet's spin axis is its body +z (now
  leaning by the tilt, see bodies-and-orbits.md). A body's pose maps its own frame to the inertial one [0022].
- The sun is a sphere at the origin, 4.6 km in radius (0.53 degrees across from the planet) [0022]. The planet
  circles it 1000 km out once a year (about 194 m/s) and spins about its body z axis; the spawn lies on the equator;
  a world starts at 9 am there [0022].
- The moon is tidally locked, inclined to the ecliptic, with a synodic month that is not a whole number of months
  per year, so new moons meet eclipse seasons at different places (current constants: `sky.rs` `MONTH`, `YEAR`,
  `INCLINATION`, `PERIOD`, `SUN_UNDER_MOON`) [0022, constants amended 0041, 0055].
- A position is in a frame: a body's own (its grid) or the inertial one. A player near a body (within its sphere of
  influence, with hysteresis) is in that body's frame and physics works against that body only. Players stand on the
  planet as on a merry-go-round: no Coriolis or centrifugal force [0022].
- Leaving a body: the server converts position and velocity through the inertial frame including the body's motion;
  in deep space you fly inertially relative to the sun [0022].
- Rendering: every body and the camera get a pose for the frame's time; the per-body uniform has a rotation next to
  its camera offset; other players are shown converted into the viewer's frame [0022].
- The star field is fixed in the inertial frame and turns with `world.pose(frame, time).rot`, the planet's real spin.
  The client's clock eases toward the server's over 0.3 s. Drift in flight is physical: the spin turns the sky 0.2
  degrees a second at 1x; travel at 328 m/s round the 4245 m planet tilts the horizon 4.4 degrees a second, at 1 km/s
  13.5 [0072].
- Stars: a galaxy band twice as bright and a little bluer on moonless nights, faint stars a little brighter, and a
  layer of a few dozen bright stars with a wider glow (`stars()` in `shader.wgsl`) [0072].

## The ray-traced light

- **Acceleration structures** (`crates/client/src/rt.rs`): one BLAS per chunk mesh, built from its quads expanded on
  the GPU (`expand`), at most 2^17 quads a frame. Quads are grown by 0.01 cell instead of splitting T-junctions. The
  TLAS is rebuilt in the camera's frame, instance mask `1 << body`. A surface traces only its own body's terrain;
  other bodies block light as analytic spheres [0022].
- The terrain stays rasterised; its fragment shader traces the light. The streamer lists the chunks it culls as
  hidden, so terrain out of view still casts shadows [0022].
- **Direct sun** is the share of the sun's disc the other spheres leave (the eclipse), times what the own body's
  sphere leaves (its horizon, for terrain past the streamed range), times shadow rays to the sun's disc [0022].
- **Moonlight** is `MOON_LIGHT` (bluish, a quarter of the sun's direct light at full moon) times the phase seen from
  the planet, less the share of the sun the planet hides from it (`Sky::covered`, a lunar eclipse). It is traced only
  where the day factor is under a half, and fades in as `day` goes from 0.5 to 0.3 [0022, 0038 C].
- **Sky light and colour** fade with the sun's height over the local horizon (twilight from -0.2 to 0.15). Night
  ambient is near black until block light comes. A body without a sea has no air: its sky is black [0022].
- **Sunset fade:** `sunset()` fades the sun's direct light from 7 degrees over the horizon to 1.7 below, on bodies
  with a sea (as `sky_light` decides air); Luna's airless edge stays hard. Two smoothsteps per lit pixel [0038 C].
- **Light's colour on other bodies:** a body other than the camera's is lit by white sunlight (`sun_on`; the camera's
  body index rides in `sun_col.w`). In an eclipse what a body with air hides of the sun returns as `COPPER` (0.23,
  0.07, 0.02) along the sunlight's own ray, so the whole moon glows evenly copper red [0037, also bodies-and-orbits].
- The sky pass draws the sun as a 0.53 degree disc with limb darkening and a glow; the moon passing in front is
  terrain, so an eclipse on screen needs nothing special [0022].
- Light over 0.8 rolls off (`shoulder`) instead of clipping [0022].
- **Metal:** `metal` 0..1 in the block data. A metal pixel traces one reflection ray, finds the quad it hit through
  the TLAS instance's custom index (the chunk table entry with its quad offset), shades it with the same light (flat
  colour, no texels), or the sky on a miss. Fresnel, tinted by the block's colour, with a glint of the sun. No
  roughness [0022].
- **Ambient occlusion** per corner in the mesher (see streaming-and-meshing.md); half strength on the bounce [0022].
- **Bounce light**, traced per pixel [0022]:
  - The terrain pass writes a G-buffer (direct light, albedo with the body, normal plus distance f32).
  - `gi_trace` sends one cosine-weighted ray per 2x2 pixels. What it hits is lit by the same `light` plus a flat 35 %
    second bounce; a miss sees the sky's light (blue by day on a body with air, black on the moon). A lamp's face adds
    nothing (its light is already direct).
  - `post.wgsl` `temporal` adds each sample to the pixel's history, found where the point was last frame and checked
    by normal and distance. A history holds 24 frames while moving; a still shot averages without limit. A change of
    frame or a jump of 20 m drops it (`Gi::drop_history`). Then four edge-aware 3x3 a-trous passes (1, 2, 4, 8 px apart)
    weighted by normal, plane distance and brightness. The composite adds albedo times that to the direct light; then
    water and overlays.
  - Known limits: no bounce on water; one traced bounce, the rest flat; ghosting where something moves fast against a
    still background.
- **Budget:** the whole lighting pass is at most 3 ms at 1080p on the dev PC; its benchmark row is in the `small`
  series [0022].
- **Debug:** `place=di:dj:dl:block,...` sets blocks around a stand point for shots [0022].

### Direct light snapped to texels [0108 amends 0098 and 0072]
(owned by 0108; stated here because 0098 and 0072 describe the dither it replaced)
- `terrain_lit` takes its direct light at `lit_at`, the middle of its painting's texel; sun and lamp shadows use 2 rays,
  4 in a penumbra, no random numbers (`sun_seen`, `lamp_seen`; the sharp-shadow setting keeps one ray to the sun's
  middle). The moon is one ray to its middle. `direct_smooth` (post.wgsl, ADR 0098) stays [0108].

### Grain and the direct-light filter [0098]
- `post.wgsl` `direct_smooth` smooths the direct light's light, without its albedo, over the 3x3 around each pixel
  where the neighbours lie on the same surface (`edge_w` as the bounce), albedo put back. Pixels of glowing blocks
  (albedo 0) and grazing metal keep their own light and count for nothing in a neighbour's. The 3x3 also smooths lamps'
  soft shadows and a metal's glint over three pixels; a penumbra wider than the filter's reach is dithered within it
  (the way on: a separate sun-visibility target, or a wider smoothing) [0098].

## Block light

- Each block with `light` in `data/blocks.toml` is a coloured point light, traced (the 64 nearest lamps from edits
  within 48 m `LAMP_REACH`, gathered per frame; a grid with 8 m cells, `gpu.rs lamp_grid`). Each pixel traces rays to
  the 3 lamps strongest there (`LAMP_RAYS` 3); the rest take those rays' mean visibility [0022, 0072].
- **Reach and curve:** `LAMP_R` is 24 m (`gpu.rs` and `shader_light.wgsl`, kept equal); falloff `2 / (1 + 0.1 d^2)`
  faded to nothing at 24 m (about 1.0 at 3 m, 0.38 at 6 m, 0.12 at 10 m). Coloured lamps follow the same curve; caves
  stay dark beyond 24 m [0072].
- **Lamp aim:** `lamp_target` aims each ray at a point of the lamp's sides that the surface can see, inset 5 cm, so a
  lamp is a 1 m light and its shadows are soft. It works in the grid's own coordinates (0037, warp 0131); the jitter
  keeps ADR 0037's guarantees (a floor, a wall and a corner point next to a lamp). Since 0108 the rays are fixed
  points, not random [0072, 0108].
- The lamp grid's cells went 4 to 8 m so a lamp still sits in at most 7 cells along each axis (`LAMP_GRID_LEN`): grid
  memory and per-frame CPU unchanged; the shader sees about 8 times as many lamps a pixel but still traces at most 3
  rays. Stretching the old curve 2x was tried first and washed a lamp's neighbours out by day (three blue lamps
  turned grass and dirt mint white); the chosen curve is as bright as before within 3 m (about 1.0 at 3 m, was 0.81)
  and glows to 20 m [0072].
- Bug reports of the round: "small white dots between the grass blocks" was the 1440p outline gap fixed in ADR 0041
  (nothing to change); "glitched texture" did not reproduce (an old-noise-land save, other time of day; the long
  dark patches are a 7-degree sun's shadows) [0072].
- Same-height blocks still cast soft lamp shadows (physically right; a non-directional "fill" would leak light through
  walls) [0072].
- Blocks that give light show their own colour, unlit. New rows: copper (metal 0.6), a warm lamp and a blue lamp
  [0022].
- **Joined lamps** [0085]: `joins = true` rows (lamp, blue lamp) glow as one surface. In `shader.wgsl` `texel_a`, with a
  join mask the texel index is clamped `JOIN_FRAME` (2) texels in from each joined side before anything else, so the
  frame there reads as the glow next to it; it works in the face's u, v before the per-face turning. The `lamp`
  painting in `tools/paint_textures.py`: a two-texel frame (dark outside, shade inside) round an even glow of lit and
  light patches with no core, four variants. An inner corner (two sides joined, the diagonal not) loses its frame
  corner texel (`shortcut:` in the shader). Any block can join by data; the clamp suits a frame of `JOIN_FRAME`
  texels. Mesher side: streaming-and-meshing.md.

## Held light and the torch [0117, 0098]

- A held block whose row gives light adds one lamp to the frame's lamps (`gpu::held_lamp`), 0.4 m ahead of the eye and
  0.3 m under it, in the row's light colour, through `cap_lamps` (it counts against the Lamps setting). Only while the
  hand is drawn (not with the inventory, a menu, the hammer or the bucket); out for a `flame` under water. In the
  inertial frame (between bodies) there is none [0098, 0117].
- Other players' held lit items are lamps at their hands (`gpu::others_lamps`) from the roster: `net::Player::hand: u16`
  (the held item, 0 none), in `ToClient::Players` (sent once a second and on a roster change; a change of the item in
  hand sends the roster in that tick, `Client::sent_hand`). It is in no recording, save or hash. Shortcut: another
  player's light follows the roster's 1 Hz [0117].
- **Torch** (block id 57): not solid, `see = "cutout"`, `loose`, stack 64, `model = "torch"`, `light = [0.62, 0.36,
  0.14]`, and two new row keys: `hangs` (needs a floor or a wall: placed only there, dropped as itself when it goes;
  refused by name on a solid or falling row) and `flame` (out under water, held or placed). `data/models/torch.toml` +
  `.png`: a 6 cm stick, 47 cm, a charred head bound in cord, 19 cm; `item = 0.6`; a point `flame` and a state `placed`
  playing the "torch flame" row of `data/effects.toml` (the fire's gradient, a tenth of its size, glow 1). No recipe
  yet: `/give torch` and creative's lamps tab [0117].
- **Which way a wall torch leans** is derived, not stored: `fall::held_from` (the floor first, then the first solid
  side in the fixed `SIDES` order). The server's placing rule and the client's drawing read the same function; nothing
  new is saved or sent. A torch on a floor beside a wall stands upright; digging its wall can turn it to its next
  wall. `models::hang` leans a wall torch 30 degrees out with its foot at the wall [0117].
- Placing in water is refused (one clause in the server's `edit`). The tick drops each unheld one as a pile of one,
  counted in `item_made` (`fall::unheld`). A block entering a hanging block's cell (sand, a sliding pile) notes it in
  `Falls::knocked` and the server drops it as one torch (`knock_off`); a flame that water covers pops off the same way
  (checked when water moved and once a second; shortcut: every placed model is looked at) [0117].
- `edit::aim` stops at a block drawn as a model (solid or not), by its cell: the outline is the cell's. A survival dig
  of a block with no dig time goes at the click [0117].
- **One light:** a placed block that gives light is its own lamp; the effects its model's state starts glow with no lamp
  of their own (`Fx::unlit`). A torch is one light placed or held [0117].
- **Hand cubes** [0098]: the texture watcher keeps each painted block's top and side (first variant) beside the means;
  `hud::cube` draws them texel for texel when large (the block in hand, 16 x 16) and as 8 x 8 means when small (hotbar,
  inventory, "looking at"). Cutout paintings' empty pixels count for nothing (an all-empty texel keeps the tone); fine
  faces keep their tones. Cubes show summer's paint only.
- Cost of a torch: a torch's light costs more than a lamp block's for the same count (a lamp block's faces take no light
  and hide the ground under them). The row is in `docs/perf-debt.md`; the idea is a reach per lamp [0117].

## Body shadows [0119]

Players' bodies are shadow proxies in the traced scene (`crates/client/src/proxy.rs`).
- Shape: each part of `ape.toml` at rest as a box, scaled to the player's height (`phys::HEIGHT`), feet on the floor,
  front +z; a plain box until the ape has loaded. One BLAS, built again when the ape's file changes. Limbs do not
  move in the shadow.
- Instances: this client's own player first (in `run=play`, under the drawn eye), then the roster's players within 64 m
  of the camera, 64 at most, each at its feet turned to its look, after the chunks in the TLAS (`PROXY_INST + k`, mask
  `16 << body`).
- Seen only by shadow rays and lamp rays (`shader.wgsl`); never by the camera, the look's aim or the bounce.
- A held lamp's rays pass through its holder's own body (the lamp's `col.w` carries the holder's place plus one); the
  bare arm and the held item pass through the camera's own body. Other players' bodies shade a held light.
- Players only; animals cast none (each kind would need its own BLAS). No wire, save or server change.
- Cost: about 0.3 ms frame-thread time and 0.19 ms GPU a frame while any body in view moves, because the TLAS is built
  again each such frame (`rt.rs`, `shortcut:`; in `docs/perf-debt.md`). The own body turns with the look, so a mouse
  move while standing still rebuilds the whole TLAS: paid nearly every frame in play. Idea: snap the yaw or the place
  before comparing.
- Open: water and particles inside the own torso's box may send sun and lamp rays out through its inside faces (a dark
  rectangle at a swimmer's waterline: suspected, not seen); the death screen keeps the body's shadow; other players'
  shadows step at the roster's 1 Hz.

## Cutout blocks (leaves, shrub, reed) [0068, 0069, 0088]

- **Data:** `see = "cutout"` (`blocks::See`) on a block row. A cutout painting's pixels have alpha 0 or 255, nothing
  between (`textures::decode(bytes, see)` names a refused pixel); an opaque block's file with a not-opaque pixel is
  refused. A face's mean colour leaves its empty pixels out [0069].
- **Pattern** `leaves` (11): leafy texels and holes of `size` texels, `amount` of them, hashed by the texel's place on
  the face's plane (shader `leaf_hole`, Studio `pattern::hole`), so the two coplanar faces between neighbouring leaf
  blocks agree. On an opaque block, with flat colours, or where rays read it, a hole is the block's colour x 0.6; with
  flat colours there are no holes in drawing or rays [0069].
- **Drawing:** cutout quads draw in a pipeline of their own after the opaque terrain (`fs_cut` discards an empty texel;
  opaque terrain keeps early depth). Fast leaves draw level 0's `cut` quads through `fs_cut` too, with open holes; they
  are the BLAS's non-opaque geometry only with fancy leaves. Coarse levels draw a cutout block in its one colour [0069,
  0088]. (Later, 0090 draws cutout depth first with `Equal` for the G-buffer; see plants-and-creatures.md / flora.)
- **Light (candidate loop):** with fancy leaves each chunk's BLAS has two geometries: opaque quads flagged OPAQUE and
  cutout quads not. `trace` in `shader_light.wgsl` runs the candidate loop and confirms a candidate unless its texel is
  empty (`hit_empty`, reading the texel on the ungrown corners with the same barycentrics, summer's look). Sun, moon,
  lamp, bounce and reflection rays all go through it. `Chunk.opq` maps a hit in the second geometry to its quad. Glass
  joins it [0068, 0069].
- **Setting** Graphics, **Leaves**: `fast` (the default) or `fancy` (`settings.rs`); `leaves=fancy|fast` on a run
  overrides; a change remeshes level 0 while old meshes stay drawn. Fancy was meant to be the default (0068) but
  misses ADR 0004's p99 on the forest fly-through (p99 6.3-12.5 ms against 8.3 in most runs; the cause is the
  non-opaque BLAS geometry, not the drawing) [0069].
- **Fast leaves' shadow:** fast leaves are opaque to rays, so they cast a whole block's shadow, also a bare crown
  (since 0090 light through twigs is answered separately) [0088]. Through a hole one sees through the whole crown (no
  inner faces, back faces culled). A mirror or bounce ray that hits a fast leaf still shades an empty texel as the dark
  fill [0088].
- Rays read a leaf texel's summer look; between two painted leaf blocks the two faces' holes differ and light needs a
  hole in both (painted cutout faces are not mirrored across a shared plane) [0069].
- **Studio** (Textures page): an Eraser (X) for a cutout block, empty pixels as a checkerboard, the block's Kind
  (Opaque, Cutout) written to its row in `data/blocks.toml` as one undo step (undo and redo refuse a cutout-to-opaque
  change when files have empty pixels); empty pixels are the reserved colour `art::EMPTY` inside the Studio, alpha 0 in
  the file. A new Kind or a painting with holes reaches the game through a rebuild (blocks.toml is built in). Open
  (low): the reserved colour `[254, 1, 253]` can be made on the Palette page; `check=1` names it [0069].
- Models' box paintings live in the block textures' buffer (a 2 MB texel storage buffer), in layers after the blocks';
  a second buffer only if they outgrow it, and raising the layer count comes first [0068]. The BLAS expand pass and the
  bounce pass bind the texel buffer [0068].
- The still-model plan: clutter shades like the ground around it but casts no shadow [0068].

## Glass [0081]

- `see = "glass"` (`blocks::See::Glass`); rows glass, green glass, amber glass (ids 120-122), loose, in the creative
  screen's "made" tab. A glass painting may have any alpha; unpainted glass is its colour at alpha 0.3 with a rim one
  texel wide at 0.8.
- `fs_glass` on the terrain's `vs`, in the screen pass after the sea and the water: blended, depth-tested, not writing
  depth, back faces culled. Lit by the forward light as the water is (direct light, the flat light from around, the sky
  at a grazing angle, the sun's glint). Chunks far to near; faces inside a chunk unsorted. Glass under the water's
  surface draws over the water (`shortcut:` in `frame.rs`; sort the two by chunk when anyone builds under water).
- Light: glass quads join the cutout quads in the second BLAS geometry; `trace` never confirms a glass candidate and
  multiplies `ray_tint` by `mix(1, texel colour, texel alpha)`. Sun, moon, lamp and bounce rays take the tint, so light
  through green glass falls green. Tinted shadows are on.
- `shortcut:` a ray that looks (bounce, reflection) also takes the tint of glass beyond what it hits, since candidates
  come unordered; exact for shadow rays.
- The indirect buffer grew by `MAX_CHUNKS * 16` bytes for glass draws. The Studio paints whole pixels only and never
  turns glass opaque; painting partial clearness ("how clear") is not built.
- (0090 multiplies `ray_tint` by the fluff card's `fluff.light` on the same path; see flora docs.)

## Painted box models [0081]

- (also items-and-crafting / effects) A model part may hold `boxes` beside its voxels: a corner and size in model
  pixels of 1/32 m (`models::PIXEL`), a turn about its middle, and `uv` where the net of its six faces lies in the
  model's painting `data/models/<name>.png` (`Bx::rects`: top and bottom in the first row, the sides below). A box thin
  on one axis is a plane (two faces back to back on one rectangle). A part may rest turned (`turn`). A painting is 8-bit
  RGBA, 1..128 px a side, every pixel empty or a palette colour; at most 64 boxes a model (`check`, `check_painting`,
  `painting`). A part of boxes alone has one empty voxel.
- Still models are meshed into the chunk and drawn by `vs_still`/`fs_still` into the G-buffer, lit as blocks are; they
  are in no BLAS (no shadow, no light) [0081]. Not built: moving box models in hand or `model=` (the client's voxel
  models still draw voxels only; later ADRs 0114 and 0116 changed that).

## Sky, shadow and hand fixes [0098]

- **Clouds end at the sea:** every cloud ray ends at the sea's sphere (post.wgsl `clouds`), because the sea is not in
  the G-buffer. No cloud is under sea level.
- **Clouds' look:** a filled cell stops light faster, `CL_K` 0.14 [now 0.09 again, 0108]; a cell's edge is half as wide;
  every cell's number is raised by `CL_OPEN` 0.06 in clouds and their shadows alike (`clouds.wgsl`); the march steps are
  4 m near the camera (still capped by the Clouds setting's steps) and the start is interleaved gradient noise;
  lightning strikes from the cloud base. The slab's height is 250 m base, `CL_THICK` 64 (`weather.rs`, held to
  `clouds.wgsl` by a test) [0098, base amended 0105].
- Cost of these fixes: none measurable (the composite's `screen` stage went 1.03 to 0.82 ms); no speed-debt row [0098].

## Air read-out [0072]

(also sky-weather-and-water)
- `Body::over_sea` gives metres over the sea (`Body::sea` is a layer number); both the server's `ToClient::Air` and F3
  use it. The read-outs share the grid's two-slope lapse `air::Params::cool` (0.1 K/m to 150 m, 0.015 above). Above the
  air's top layer (1500 m over the sea) F3 says there is no air; `Weather::here` holds it there too. Left: F3's top uses
  the nominal 1500 m (the grid's is `max(1500, ground + 30)`).

## Open

- A wider smoothing or a separate sun-visibility target if the 3x3 is
  not enough [0098].
- Whether bare crowns should cast no shadow in fast leaves (0090 answered with light through twigs) [0088].
- Animals' shadows and moving limbs wait for a cheaper way to keep moving things in the traced scene [0119].
- Other players' held light and shadows step at 1 Hz until players are drawn moving [0117, 0119].
- The cubes (hotbar and others) show summer paint only [0098].

## No longer holds

- 0022's "no axial tilt" and its orbit numbers (moon at 2.83 planet radii, 40 degree inclination, 20 minute day):
  replaced by 0041 (30 minute day, 2 degree inclination, Luna moved out), 0055 (tilt) and 0027; see bodies-and-orbits.md.
- 0022's plan of shadow maps / ray marching in compute: not taken (option 3, ray queries, holds).
- 0072's lamp reach 12 m and lamp grid 4 m: now 24 m and 8 m (0072's own). Its random per-pixel lamp aim: replaced by
  0108's fixed rays.
- 0098's white-noise to low-discrepancy sun disc point (option c): replaced by 0108's noiseless 2/4 rays.
- 0098's cloud base at 100 m: replaced by 0105 (250 m); its `CL_K` 0.14: replaced by 0108 (0.09).
- 0038 C and 0072 are history for their bug reports; their fixes stand as above.
- 0068's "fancy leaves are the default": replaced by 0069. 0069's "fast leaves are opaque in the drawing": replaced by
  0088 (open holes; rays still opaque).
- 0068's spike light options 1 and 2 (opaque to rays; five queries at most): not taken.
- 0085's status "proposed": built (the code has `joins`).
