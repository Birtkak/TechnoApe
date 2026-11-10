# 0022 – Ray-traced light, and a sky that moves: spin, orbits, eclipses

- **Status:** accepted
- **Date:** 2026-11-10

## Context
The roadmap planned block light, a sun circling the planet, the 20 minute day and shadow maps. The developer
asked for more (2026-11-10):
- ray-traced light, with the sun as the only big source, and the sun a real sphere;
- moonlight as sunlight bounced off a moon that orbits the planet, so it has phases and lights the ground from
  where it is;
- real orbits and the planet's rotation;
- eclipses as a real, physical blockage of the light, with a different share of totality at each place on
  the planet;
- reflections on metal, and later telescopes;
- smooth, coloured block light, and soft but fairly sharp shadows;
- performance stays first (ADR 0004).

The developer also decided that the game requires a GPU with hardware ray tracing (no fallback path).

## Options considered
1. **Shadow maps plus a faked sky.** Cheap, but no real reflections, no physical eclipses, and many cascades on
   a curved planet.
2. **Ray marching through a voxel copy in compute.** Works on any GPU, but needs its own 3D structure next to
   the meshes, and gets costly for reflections and long rays.
3. **Hardware ray queries (wgpu `EXPERIMENTAL_RAY_QUERY`, Vulkan).** Measured in `prototypes/lighting/` on the
   RTX 3070:
   - a camera ray, a soft sun shadow and a reflection ray with its own shadow on every 1080p pixel cost about
     1.8 ms over 2.3 M triangles of real terrain;
   - an edit's BLAS rebuild costs about 0.05 ms.
   - Cons: an RTX 20, RX 6000 or Arc class GPU; wgpu's DX12 backend has none of it.

## Decision
**Option 3, on Vulkan only.**

The client refuses to start without ray queries and says why. The sky is kinematic: circular orbits on rails,
pure functions of the world's time, the same on every machine.

### Time and the sky (`engine::sky`)
- **Time.** World time is `tick / 20 Hz` seconds since the world was made. The save keeps the tick, and
  clients get it from the server.
- **The inertial frame:**
  - centred on the sun (the developer's call, 2026-11-10, so later planets and the sun itself have a place);
  - the ecliptic is its xy plane;
  - the planet's spin axis is its +z, with no axial tilt for now (shortcut: no seasons until the biomes want
    them).
- **The sun.**
  - A sphere at the origin, 4.6 km in radius: 0.53° across from the planet, as Earth's.
- **The planet** circles the sun 1000 km out in the ecliptic, once a year (about 194 m/s), and spins about
  its body z axis.
  - The solar day is 20 min (ADR 0008); the sidereal day follows from it and the year.
  - The spawn, near body +y, lies on the equator, so it gets full days and nights. A world starts at 9 am
    there.
- **The moon.**
  - It orbits at 2.83 planet radii (ADR 0021), inclined 40° to the ecliptic, with its nodes fixed in the
    inertial frame.
  - The synodic month is 8 days and the year 27 days (9 hours of play): 3.4 new moons a year. A whole number
    of months per year would put the new moons at the same places each year, always in or always out of
    eclipse season.
  - It is tidally locked: the same face always looks at the planet.
- **Eclipses** follow from the geometry. A solar eclipse falls somewhere on the planet when the moon's
  penumbra touches it at new moon. At this size that is about a third of new moons, roughly one a year, and
  a lunar eclipse comes about as often. The moon's angular size (1.7°) is larger than the sun's, so total
  eclipses happen in a band about 250 m wide.
- **Defaults to tune.** All the numbers above are constants in `sky.rs`. Shortcut: they move to a data file
  when worlds get chosen skies.

### Reference frames
- A position is in a frame: a body's own (its grid, at the body's nominal centre, as today) or the inertial
  one.
  - A player near a body (within its sphere of influence, with hysteresis) is in that body's frame.
  - Physics, collision, reach and edits then work exactly as now, against that body only.
  - Players stand on the planet as on a merry-go-round. Shortcut: no Coriolis or centrifugal force.
- **Leaving a body.** The server converts the position and the velocity through the inertial frame,
  including the body's motion, so a jump from the moon carries its orbital speed. In deep space you fly
  inertially, relative to the sun: the planet moves at about 194 m/s there, and the moon at that, plus or
  minus about 16 m/s.
- **Rendering.** Every body and the camera get a pose for the frame's time. The shader's per-body uniform
  gains a rotation next to its camera offset, and other players are shown converted into the viewer's frame.

### Light (client)
- **Ray traced.**
  - One BLAS per drawn chunk, from the same quads the mesher makes, with T-junctions split so the geometry
    is watertight. All chunks sit in one TLAS per frame.
  - Bodies far away, and the sun, are analytic spheres in the shader.
  - Shadow rays toward a random point of the sun's disc give soft shadows, and in an eclipse the share of the
    disc the moon covers.
  - Moonlight is a second source: the sun's light reflected by the lit part of the moon, from the moon's real
    position, with its brightness following the phase.
- **As built (2026-11-10), sun, moon and sky:**
  - The terrain stays rasterised; its fragment shader traces the light. The streamer lists the chunks it
    culls as hidden, so terrain out of view still casts shadows.
  - `crates/client/src/rt.rs`: a BLAS per chunk mesh, built from its quads expanded on the GPU (`expand`), at
    most 2^17 quads a frame. Quads are grown by 0.01 cell instead of split at T-junctions: the overlap closes
    the cracks just as well, in one line. The TLAS is rebuilt every frame in the camera's frame, and its
    instance mask is `1 << body`. A surface traces only its own body's terrain; other bodies block light as
    spheres.
  - Direct sun is the share of the disc the other spheres leave (the eclipse), times what the own body's sphere
    leaves (its horizon, for terrain past the streamed range), times one ray to a random point of the disc.
  - The moon's light is `MOON_LIGHT` (bluish, a quarter of the sun's direct light at full moon) times its phase
    seen from the planet, less the share of the sun the planet hides from it (`Sky::covered`, a lunar
    eclipse). It is traced only where the day factor is under a half.
  - The sky's light and colour fade with the sun's height over the local horizon (twilight from -0.2 to 0.15).
    Night ambient is near black until block light comes. A body without a sea has no air: its sky is black.
  - The sky pass draws the sun as a 0.53 degree disc with limb darkening and a glow. The moon passing in front
    of it is terrain, so an eclipse on screen needs nothing special.
  - Cost on the dev PC: about +0.7 ms p99 standing and +1.7 ms flying (BLAS builds as chunks stream),
    and +300 MB VRAM.
- **As built (2026-11-10), block light, ambient occlusion, metal.** The plan above changed in two places:
  - **Block light is traced, not flooded.** Each block with `light` in `data/blocks.toml` is a coloured point
    light (inverse square, out to 8 m), and its shadows are real. The client gathers the 64 nearest lamps
    from the edits within 48 m. Each pixel traces shadow rays to the 3 lamps strongest there; the rest take
    those rays' mean visibility, so a room full of lamps costs 3 rays and not one per lamp. Every lamp also
    adds a share with no ray (12 %) for the light its room bounces back. Why: placing a lamp shows the same
    frame, with nothing re-meshed and no chunks marked for 15 blocks around; the shadows are right; and it is
    less code than a flood across chunk borders. The cost: light does not creep around corners as a flood's
    does, the 12 % aside. Blocks that give light show their own colour, unlit.
  - **Ambient occlusion is per corner in the mesher, not short rays.** The classic voxel kind: two sides and
    the diagonal in front of each corner, 2 bits a corner in the quad's spare high bits. It is smooth and
    costs nothing per frame; one AO ray per pixel would be grainy with no smoothing over frames. Greedy
    merging joins only open faces.
  - **Metal:** `metal` 0..1 in the block data. A metal pixel traces one reflection ray. The ray finds the quad
    it hit through the TLAS instance's custom index, which is the chunk table entry, now with its quad offset.
    It shades that quad with the same light (flat colour, no texels), or the sky on a miss. Fresnel mixes it
    in, tinted by the block's colour, with a glint of the sun. There is no roughness yet.
  - New blocks: copper (metal 0.6), a warm lamp and a blue lamp.
  - Light over 0.8 rolls off (`shoulder`) instead of clipping; colours below are unchanged.
  - Cost on the dev PC: 31 lamps around the camera at night, 2.5 ms p99 frame (1.9 ms GPU); 3 rays per pixel
    at most there.
  - Debug: `place=di:dj:dl:block,...` sets blocks around a stand point for shots.
- **Budget:** the whole lighting pass is at most 3 ms at 1080p on the dev PC. Its benchmark row is in the
  `small` series.

## Consequences
- Day and night, moon phases, moonlit nights, solar and lunar eclipses and reflections all come from one model
  rather than separate effects.
- Telescopes later need only a narrow camera.
- Players without a ray-tracing GPU cannot play. The developer accepted this.
- Moving bodies touch every system that uses world positions:
  - the protocol gains frames and the world time;
  - physics knows which body it stands on;
  - streaming measures distance in each body's frame;
  - the soak and the bots run with the sky moving.
- Known limits:
  - no axial tilt or seasons;
  - orbits are on rails, without n-body gravity;
  - no Coriolis or centrifugal force on a spinning body.
