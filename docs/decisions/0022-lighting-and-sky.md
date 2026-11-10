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
  - centred on the planet;
  - the ecliptic is its xy plane;
  - the planet's spin axis is its +z, with no axial tilt for now (shortcut: no seasons until the biomes want
    them).
- **The sun.**
  - A sphere 1000 km away, 4.6 km in radius: 0.53° across, as Earth's.
  - It moves once around the ecliptic per year, the planet's orbit seen from the planet.
- **The planet** spins about its body z axis.
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
  inertially, where the moon moves at about 16 m/s.
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
- **Block light.**
  - Coloured (RGB) and smooth, flooded per block on the meshing workers.
  - It is sampled per vertex and blended across faces, and it feeds the same shading.
- **Ambient light.**
  - Short occlusion rays give ambient occlusion.
  - The sky's light: blue by day, black by night.
- **Metal.** Materials gain roughness and metalness; metal pixels cast a reflection ray.
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
