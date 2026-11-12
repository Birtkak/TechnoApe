enable wgpu_ray_query;

// Vertex pulling: no vertex buffers. One 8-byte quad per face; its 6 vertices are made here and mapped from
// grid coordinates (column i, column j, layer) onto the cube-sphere (ADR 0003).
// Every vertex is computed from its global integer grid point, so neighbouring quads and chunks agree exactly.
struct Body {
    cam_hi: vec3f, // camera relative to the body's centre, split in two floats per axis
    r0: f32,       // radius of layer 0
    cam_lo: vec3f,
    lnq: f32,      // layer l has radius r0 * exp(l * lnq)
    theta: f32,    // angle of one column: (pi / 2) / n
    half_n: f32,
    sea: f32,      // the sea level as a layer (ADR 0020), negative for none
    year: f32,     // its looks' season: the year in quarter turns for the north (ADR 0061), negative for none
    // the body's frame turned into the camera's (ADR 0022): columns of the rotation
    rot0: vec4f,
    rot1: vec4f,
    rot2: vec4f,
    ctr: vec4f,    // the body's centre from the camera, in the camera's frame; w its radius, 0 for no body
};
// sun: the direction to the sun, w the water's clock in s; sun_pos: the sun's centre from the camera, w its
// radius; sun_col: its light through the air here, w the camera's body (`sun_on`); moon_col: the moon's light (rgb, w its body index); sky: the sky's colour here
struct Globals {
    view_proj: mat4x4f,
    sun: vec4f,
    bodies: array<Body, 4>,
    inv_vp: mat4x4f,
    sun_pos: vec4f,
    sun_col: vec4f,
    moon_col: vec4f,
    sky: vec4f,
    n_lamps: vec4u, // x: lamps in `lamps`, y: the frame number (the bounce rays' seed); z, w: the bodies that
                    // can cover the sun's and the moon's discs, a bit each (`eclipse_masks` in main.rs)
    // the camera's frame turned into the inertial one (the stars'), as columns; x.w the air around the
    // camera (0 in space .. 1), y.w the sun's height over the horizon (sine)
    stars: array<vec4f, 3>,
    // x: the settings' graphics rows, a bit each: 1 sharp sun shadows, 2 no reflections, 4 flat block
    // colours, 8 no stars (`App::gfx_bits` in main.rs); 0 is the picture the game is tuned for.
    // y: the settings' brightness in thousandths (post.wgsl has it as `tune.w`), for what is drawn after
    // the composite
    opt: vec4u,
    // the planet's tide (ADR 0067, engine `sky::Tide`): Luna's and the sun's directions in its frame, w their
    // pull in layers; its base is in bodies[0].sea (gpu.rs `Globals::tide`)
    tide: array<vec4f, 2>,
};
// base in columns and layers; info = face | level << 3 | body << 8; off: its first quad
// opq: its opaque land quads; the cutout ones follow them, the BLAS's second geometry (ADR 0069)
struct Chunk { base: vec3i, info: u32, off: u32, opq: u32 };

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var<storage, read> quads: array<vec2u>;
@group(0) @binding(2) var<storage, read> chunks: array<Chunk>;
// a block's look from data/blocks.toml (ADR 0013): info = pattern | side << 8 | metal (0..255) << 16 | gives light
// << 24 | cutout << 25 (ADR 0069); patterns 0 plain, 1 grass, 2 stone, 11 leaves (holes)
// (ADR 0039) and a pattern's second and third colours, its size in texels, amount and grain contrast; the
// painted faces (ADR 0045, gpu.rs `looks_of`: 16 bits a face, layer | variants - 1 << 12 | painted << 15; top
// and side in `tex`, the bottom in `tex2`) and a painted block's one colour, unorm 8-bit, painted << 24
struct Look { colour: vec3f, info: u32, colour2: vec3f, size: u32, colour3: vec3f, amount: f32, contrast: f32, tex: u32, mean: u32, tex2: u32 };
// four rows a block (ADR 0061): its own look, summer's, at [m]; spring's, autumn's and winter's 256, 512 and
// 768 on. `size` holds, over the size's 8 bits, bit 8 for a block with seasons and its stagger from bit 16
// (1/1024 quarter turn)
@group(0) @binding(3) var<storage, read> looks: array<Look, 1024>;
// the painted faces' 16 x 16 layers, RGBA8 a u32, rows from the top, read with no filter and no mips: one texel
// as the patterns hash one (a buffer, not a texture array: an image took a new 256 MB block of VRAM, ADR 0045)
@group(0) @binding(11) var<storage, read> tex: array<u32>;
// the colour a ray that meets block m sees in season sn: its own, or a painted block's mean
fn seen_colour(m: u32, sn: vec3f) -> vec3f {
    let seasons = looks[m].size >> 8u;
    if (seasons == 0u || sn.y == 0.0) { return row_seen(m); }
    let r = season_rows(seasons, sn);
    return mix(row_seen(m), mix(row_seen(r.x + m), row_seen(r.y + m), r.f), sn.y);
}
// look row r's one colour: its paint's mean, else its colour
fn row_seen(r: u32) -> vec3f {
    return select(looks[r].colour, unpack4x8unorm(looks[r].mean).rgb, (looks[r].mean >> 24u) != 0u);
}
// every chunk's terrain near and far, drawn or not, one instance each with mask 1 << body (ADR 0022), in its
// body's frame relative to its centre: the instances carry no transform, a ray is turned into that frame here
// (ADR 0025, B)
@group(0) @binding(5) var acc: acceleration_structure;
// the lamps near the camera (ADR 0022): pos from the camera, w its body; col its light
struct Lamp { pos: vec4f, col: vec4f };
@group(0) @binding(8) var<storage, read> lamps: array<Lamp>;
// the lamp grid (ADR 0025, H): LAMP_G^3 cells of LAMP_CELL metres in the camera's frame, the camera in the
// middle; cell c's lamps, those LAMP_R or nearer, are lamp_cells[lamp_cells[c] .. lamp_cells[c + 1]]
// (`lamp_grid` in main.rs, which holds the same numbers)
const LAMP_G: i32 = 32;
const LAMP_CELL: f32 = 4.0;
@group(0) @binding(9) var<storage, read> lamp_cells: array<u32>;

struct VOut {
    @builtin(position) clip: vec4f,
    @location(0) local: vec3f,
    @location(1) @interpolate(flat) dir: u32,
    @location(2) @interpolate(flat) mat: u32,
    @location(3) @interpolate(flat) ci: u32,
    @location(4) normal: vec3f,
    @location(5) rel: vec3f, // from the camera
    @location(6) ao: f32,    // open 1 .. shut in
};

fn axis(i: u32) -> vec3f { return vec3f(f32(i == 0u), f32(i == 1u), f32(i == 2u)); }

// corner k of quad q in its chunk's cells; grow widens it by that many cells on every side
fn quad_corner(q: vec2u, k: u32, grow: f32) -> vec3f {
    let lo = q.x;
    let p = vec3f(f32(lo & 63u), f32((lo >> 6u) & 63u), f32((lo >> 12u) & 63u));
    let w = f32(((lo >> 18u) & 31u) + 1u);
    let h = f32(((lo >> 23u) & 31u) + 1u);
    let dir = (lo >> 28u) & 7u;
    let a = dir / 2u;
    // corner order of two CCW triangles; mirrored for the negative faces
    var cu = array<f32, 6>(0., 1., 1., 0., 1., 0.);
    var cv = array<f32, 6>(0., 0., 1., 0., 1., 1.);
    var du = cu[k];
    var dv = cv[k];
    if ((dir & 1u) == 1u) { du = cv[k]; dv = cu[k]; }
    return p + axis((a + 1u) % 3u) * (du * (w + 2.0 * grow) - grow) + axis((a + 2u) % 3u) * (dv * (h + 2.0 * grow) - grow);
}

// a point of chunk c, in its cells, on the cube-sphere: the grid point and its frame
fn grid(c: Chunk, local: vec3f) -> vec3f {
    return vec3f(c.base) + local * f32(1u << ((c.info >> 3u) & 31u));
}

// where a point of chunk c (in its cells) is drawn, from the body's centre: trilinear between the exact grid
// points on the chunk's lattice (`Body::lattice`, 1 << info bits 16-17 cells), so a merged quad's edge, inside
// one lattice cell, passes through its neighbours' corners (ADR 0025 I2). `at` is the quad's own corner, which
// picks the cell.
fn place(c: Chunk, b: Body, local: vec3f, at: vec3f) -> vec3f {
    let s = f32(1u << ((c.info >> 3u) & 31u));
    let n = f32(1u << ((c.info >> 16u) & 3u));
    let l0 = floor(at / n) * n;
    let t = (local - l0) / n;
    let g0 = vec3f(c.base) + l0 * s;
    let st = n * s;
    // the four columns' directions from one frame: the cube's axes are shared, only the tangents differ
    let f = frame_at(c.info & 7u, b, g0.x, g0.y);
    let ta1 = tan((g0.x + st - b.half_n) * b.theta);
    let tb1 = tan((g0.y + st - b.half_n) * b.theta);
    let u0 = mix(f.up, normalize(f.n + f.s * ta1 + f.t * f.tb), t.x);
    let u1 = mix(normalize(f.n + f.s * f.ta + f.t * tb1), normalize(f.n + f.s * ta1 + f.t * tb1), t.x);
    let r = mix(b.r0 * exp(g0.z * b.lnq), b.r0 * exp((g0.z + st) * b.lnq), t.z);
    return mix(u0, u1, t.y) * r;
}

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
    let q = quads[vi / 6u];
    let dir = (q.x >> 28u) & 7u;
    let a = dir / 2u;
    let k = vi % 6u;
    // grown by a millimetre: a merged quad's edge and its neighbours' corners on it are worked out apart, at
    // f32's grain at the body's radius (a quarter millimetre at 4 km), and met with pixel holes of sky between
    // them (bug 2026-11-11_17-38-04, ADR 0041); the overlap closes them, too thin to see as a lip
    let local = quad_corner(q, k, DRAW_GROW);
    // the corner in the face's u, v: as quad_corner lays it out
    var cu = array<u32, 6>(0u, 1u, 1u, 0u, 1u, 0u);
    var cv = array<u32, 6>(0u, 0u, 1u, 0u, 1u, 1u);
    var ku = cu[k];
    var kv = cv[k];
    if ((dir & 1u) == 1u) { ku = cv[k]; kv = cu[k]; }
    let occl = (q.y >> (8u + 2u * (ku + 2u * kv))) & 3u;
    let c = chunks[ii];
    let b = g.bodies[(c.info >> 8u) & 255u];
    let gp = grid(c, local);
    let f = frame_at(c.info & 7u, b, gp.x, gp.y);
    let r = b.r0 * exp(gp.z * b.lnq);
    // the sides of a column are planes through the body's centre; its top is the sphere
    var normal = f.up;
    if (a == 0u) { normal = normalize(f.s - f.n * f.ta); }
    if (a == 1u) { normal = normalize(f.t - f.n * f.tb); }
    if ((dir & 1u) == 1u) { normal = -normal; }

    var o: VOut;
    let at = quad_corner(q, 0u, 0.0);
    let rel = brot(b, (place(c, b, local, at) - b.cam_hi) - b.cam_lo);
    o.clip = g.view_proj * vec4f(rel, 1.0);
    o.rel = rel;
    o.ao = 1.0 - 0.2 * f32(occl);
    // the texture and the cell from the quad's own corner: in the grown lip, fract() would wrap to the far edge
    // (a dirt line along every grass top edge) and floor() name the neighbour
    o.local = quad_corner(q, k, 0.0);
    o.dir = dir;
    o.mat = q.y;
    o.ci = ii;
    o.normal = brot(b, normal);
    return o;
}

// ---- the traced geometry (ADR 0022) ----
// Each new chunk's land quads become 6 vertices each in its body's frame, from the body's centre, for its BLAS.
// Grown by GROW cells, so greedy quads meeting at T-junctions overlap instead of leaving hairline cracks.
struct Build { base: vec3i, info: u32, off: u32, n: u32, dst: u32, _p: u32 };
@group(0) @binding(6) var<storage, read> builds: array<Build>;
@group(0) @binding(7) var<storage, read_write> verts: array<f32>;
const GROW: f32 = 0.01;
// the drawn quads' own overlap, in cells (`vs`)
const DRAW_GROW: f32 = 0.001;

@compute @workgroup_size(64)
fn expand(@builtin(global_invocation_id) id: vec3u) {
    let bd = builds[id.y];
    if (id.x >= bd.n) { return; }
    let q = quads[bd.off + id.x];
    let c = Chunk(bd.base, bd.info, bd.off, 0u);
    let b = g.bodies[(c.info >> 8u) & 255u];
    for (var k = 0u; k < 6u; k++) {
        let at = quad_corner(q, 0u, 0.0);
        let p = place(c, b, quad_corner(q, k, GROW), at);
        let o = (bd.dst + id.x * 6u + k) * 3u;
        verts[o] = p.x;
        verts[o + 1u] = p.y;
        verts[o + 2u] = p.z;
    }
}

// ---- light (ADR 0022) ----
// The sun is the one source. Direct light is what the bodies' spheres leave of its disc (eclipses, and the
// horizon of the body far away) times one ray through the terrain to a point of the disc (soft shadows).
// The moon reflects it by its phase; lamps add theirs. `light` is the direct light only: the light of the
// sky and of everything around, bounced, comes from the bounce pass (`gi_trace`, post.wgsl).
const PI: f32 = 3.14159265;
const DAY_AMB: vec3f = vec3f(0.33, 0.35, 0.40);
const NIGHT_AMB: vec3f = vec3f(0.018, 0.022, 0.035);
// the night sky's own faint light (`sky_colour`)
const NIGHT_GLOW: vec3f = vec3f(0.02, 0.03, 0.07);

fn hash2(p: vec2u) -> f32 {
    var h = p.x * 73856093u ^ p.y * 19349663u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    h = h ^ (h >> 16u);
    return f32(h & 65535u) / 65535.0;
}

// the share of a disc of angular radius rs that a disc of angular radius rb, d away, covers
fn cover(rs: f32, rb: f32, d: f32) -> f32 {
    if (d >= rs + rb) { return 0.0; }
    if (d <= rb - rs) { return 1.0; }
    if (d <= rs - rb) { return (rb / rs) * (rb / rs); }
    if (rb > 20.0 * rs) {
        // a horizon: the body's edge is a straight line across the disc, x radii from its centre
        let x = clamp((d - rb) / rs, -1.0, 1.0);
        return (acos(x) - x * sqrt(1.0 - x * x)) / PI;
    }
    // two discs in the small-angle plane: the lens they share
    let l1 = rs * rs * acos(clamp((d * d + rs * rs - rb * rb) / (2.0 * d * rs), -1.0, 1.0));
    let l2 = rb * rb * acos(clamp((d * d + rb * rb - rs * rs) / (2.0 * d * rb), -1.0, 1.0));
    let k = sqrt(max((-d + rs + rb) * (d + rs - rb) * (d - rs + rb) * (d + rs + rb), 0.0)) * 0.5;
    return clamp((l1 + l2 - k) / (PI * rs * rs), 0.0, 1.0);
}

// what body i's sphere leaves of a light disc at lp of radius lr, seen from p; a point inside the sphere
// (under its nominal surface) leaves it to the terrain's rays
fn left_by(i: u32, p: vec3f, lp: vec3f, lr: f32) -> f32 {
    let bc = g.bodies[i].ctr;
    let v = bc.xyz - p;
    let dist = length(v);
    let ld = lp - p;
    if (bc.w <= 0.0 || dist <= bc.w || dist - bc.w > length(ld)) { return 1.0; }
    let rs = asin(min(lr / length(ld), 1.0));
    let rb = asin(bc.w / dist);
    return 1.0 - cover(rs, rb, atan2(length(cross(ld, v)), dot(ld, v)));
}

// where a ray from surface point p facing n starts: off the surface, more so far from the camera
fn ray_origin(p: vec3f, n: vec3f) -> vec3f {
    return p + n * (0.02 + length(p) * 2e-5);
}

// a point from the camera into body b's frame, relative to its centre; a direction d turns as `d * mat3x3f(rot)`
fn to_body(b: Body, p: vec3f) -> vec3f {
    return b.cam_hi + (b.cam_lo + p * mat3x3f(b.rot0.xyz, b.rot1.xyz, b.rot2.xyz));
}

// one ray through the terrain of body `body`: 1 if it gets out, or as far as tmax from `ray_origin`
fn ray(p: vec3f, n: vec3f, d: vec3f, body: u32, tmax: f32) -> f32 {
    let b = g.bodies[body];
    let o = to_body(b, ray_origin(p, n));
    let db = d * mat3x3f(b.rot0.xyz, b.rot1.xyz, b.rot2.xyz);
    return select(1.0, 0.0, trace(o, db, body, tmax, RAY_FLAG_TERMINATE_ON_FIRST_HIT).kind != RAY_QUERY_INTERSECTION_NONE);
}

// A ray through the terrain (ADR 0069, light option 3): the opaque geometry commits its hits by itself; a cutout
// quad (a BLAS's second geometry, not flagged opaque) is a candidate, confirmed unless its texel at the hit is
// empty, so the sun, the lamps and the bounce pass through a crown's holes. Shadow, lamp and bounce rays all
// come here. `flags`: a shadow ray takes the first hit, a ray that looks takes the nearest.
// Glass (ADR 0081) is in the same geometry and never confirmed: each glass candidate multiplies `ray_tint` by what
// its texel lets through, so light through a green pane comes out green. (shortcut: candidates come unordered, so
// a ray that looks also takes the tint of glass beyond what it hits; exact only for shadow rays)
var<private> ray_tint: vec3f;
fn trace(o: vec3f, db: vec3f, body: u32, tmax: f32, flags: u32) -> RayIntersection {
    var rq: ray_query;
    ray_tint = vec3f(1.0);
    rayQueryInitialize(&rq, acc, RayDesc(flags, 1u << body, 0.0, tmax, o, db));
    while (rayQueryProceed(&rq)) {
        let c = rayQueryGetCandidateIntersection(&rq);
        let q = hit_quad(c.instance_custom_data, c.geometry_index, c.primitive_index);
        if (is_glass(q.y)) {
            let t = hit_texel(c.instance_custom_data, q, c.primitive_index, c.barycentrics);
            ray_tint *= mix(vec3f(1.0), t.rgb, t.a);
        } else if (!hit_empty(c.instance_custom_data, q, c.primitive_index, c.barycentrics)) {
            rayQueryConfirmIntersection(&rq);
        }
    }
    return rayQueryGetCommittedIntersection(&rq);
}

// the quad a ray hit: chunk-table entry ci, geometry geo (0 the opaque quads, 1 the cutout ones), triangle prim
fn hit_quad(ci: u32, geo: u32, prim: u32) -> vec2u {
    let c = chunks[ci];
    return quads[c.off + select(0u, c.opq, geo == 1u) + prim / 2u];
}

// whether a candidate hit at barycentrics bc is on an empty texel of its cutout block (level 0 only meshes them
// so; summer's look, `texel_a` without a season)
fn hit_empty(ci: u32, q: vec2u, prim: u32, bc: vec2f) -> bool {
    // flat colours draw a cutout block whole: the light takes it whole too
    if ((g.opt.x & 4u) != 0u) { return false; }
    return hit_texel(ci, q, prim, bc).a < 0.5;
}

// the texel of quad q of chunk-table entry ci at a candidate hit's barycentrics bc on its triangle prim
fn hit_texel(ci: u32, q: vec2u, prim: u32, bc: vec2f) -> vec4f {
    let c = chunks[ci];
    let tri = (prim & 1u) * 3u;
    // the BLAS holds the quads grown by GROW; the same barycentrics on the ungrown corners map a hit in the lip
    // onto the quad's edge texel, as `vs` keeps `local` ungrown (a grown lip's fract wraps to the far edge)
    let local = quad_corner(q, tri, 0.0) * (1.0 - bc.x - bc.y) + quad_corner(q, tri + 1u, 0.0) * bc.x + quad_corner(q, tri + 2u, 0.0) * bc.y;
    let dir = (q.x >> 28u) & 7u;
    let a = dir / 2u;
    let sgn = select(1.0, -1.0, (dir & 1u) == 1u);
    let cell = vec3i(floor(local - axis(a) * sgn * 0.5)) + c.base + vec3i(i32(c.info & 7u) * 977);
    let f = fract(local);
    return texel_a(q.y, dir, cell, vec2f(f[(a + 1u) % 3u], f[(a + 2u) % 3u]), f.z, vec3f(0.0));
}

// a direction to a point of a disc of angular radius ar around direction s, from a per-pixel number pair
fn in_disc(s: vec3f, ar: f32, px: vec2u) -> vec3f {
    let t = normalize(cross(s, select(vec3f(0., 1., 0.), vec3f(1., 0., 0.), abs(s.y) > 0.9)));
    let b = cross(s, t);
    let r = ar * sqrt(hash2(px));
    let ph = 2.0 * PI * hash2(px + vec2u(7919u, 104729u));
    return normalize(s + (t * cos(ph) + b * sin(ph)) * r);
}

struct Light { l: vec3f, sun: f32, day: f32 };

// the sky's light over point p of body `body`, for bounce rays that leave: blue by day where the body has
// air (a sea), fading through twilight; no sun's disc
const DAY_SKY: vec3f = vec3f(0.32, 0.46, 0.66);
fn sky_light(p: vec3f, body: u32) -> vec3f {
    let b = g.bodies[body];
    let day = smoothstep(-0.2, 0.15, dot(normalize(p - b.ctr.xyz), normalize(g.sun_pos.xyz - p)));
    return DAY_SKY * day * select(0.0, 1.0, b.sea >= 0.0) + NIGHT_AMB;
}

// the share of the sun's direct light the air lets through near the horizon, where the body has air (a sea):
// it fades over the last few degrees, so the line where the sun sets behind the body's sphere is a gradient
// seen from far away, not a hard edge (ADR 0038 C); 1 on a body without air
fn sunset(up: vec3f, sd: vec3f, body: u32) -> f32 {
    return select(1.0, smoothstep(-0.03, 0.12, dot(up, sd)), g.bodies[body].sea >= 0.0);
}

// a flat stand-in for the light from around, where no bounce ray is traced: water, and what rays hit
fn ambient(day: f32) -> vec3f {
    return mix(NIGHT_AMB, DAY_AMB, day);
}

// the light on a surface of body `body` at p (from the camera) with normal n
const LAMP_R: f32 = 12.0;
const LAMP_RAYS: u32 = 3u;
// a shadow ray goes to the lamp's block, not its middle (`lamp_target`), and stops this far short of it, so
// the lamp never shadows itself
const LAMP_STOP: f32 = 0.01;
const NONE: u32 = 0xffffffffu;

// a lamp's light d2 square metres away: falling off with the square of the distance from 2 m, to nothing at
// LAMP_R. It was 1.5 / (1 + d2) to 8 m, a seventh at 3 m and nothing past 6; the developer asked for "much more
// light" (bug 2026-11-11_17-43-39, ADR 0041): now 7 times that at 3 m, 20 at 6, a glow to 10
fn lamp_light(lp: Lamp, d2: f32) -> vec3f {
    let x = 1.0 - d2 / (LAMP_R * LAMP_R);
    return lp.col.rgb * x * x * 3.0 / (1.0 + 0.25 * d2);
}

// the sunlight on body `body`: through the air as the camera sees the sun on its own body; white on
// another, which has its own sky (bug 2026-11-11_13-17-26: the moon took the camera's sunset red every night)
fn sun_on(body: u32) -> vec3f {
    return select(vec3f(0.65), g.sun_col.rgb, body == u32(g.sun_col.w));
}
// the sunlight left in a planet's full shadow, bent round it through its air: dim copper, even over the moon
const COPPER: vec3f = vec3f(0.65, 0.2, 0.06) * 0.35;
// how much brighter the moon is drawn in the night sky of a body with air (ADR 0070, the developer's to tune):
// the eye adapts to the night, and the moon, lit by the full sun, is far the brightest thing in it; drawn at
// the ground's exposure it read grey. 1 draws it as it is.
const MOON_NIGHT_GAIN: f32 = 2.6;
// the gain on the sunlight on `body` as the camera sees it: MOON_NIGHT_GAIN on the moon from inside another
// body's air at night, faded in with the dusk; 1 elsewhere (by day, from space, standing on the moon)
fn moon_gain(body: u32) -> f32 {
    if (body != u32(g.moon_col.w) || body == u32(g.sun_col.w)) { return 1.0; }
    let night = 1.0 - smoothstep(-0.1, 0.05, g.stars[1].w);
    return mix(1.0, MOON_NIGHT_GAIN, night * g.stars[0].w);
}

// where a shadow ray from o aims at lamp lp: the point of its block nearest o, 2 cm out from the faces o is
// beyond (out along the others too, or in, would put a flat ray's end inside the blocks beside it). Aimed at
// the middle, the ray to a lamp set in a floor or wall passed through the blocks beside it wherever it came
// in flatter than 30 degrees, and the lamp's light ended there in a hard cone over the walls; a lamp on the
// ground cast spikes from its own corners (bugs 2026-11-11_13-00-53, 2026-11-10_19-45-23; ADR 0037).
// Worked in the grid's own coordinates (the face's two angles and the log of the radius, its face the up's
// largest axis), where the block is a box: o clamped into the lamp's cell and turned back.
fn lamp_target(lp: Lamp, o: vec3f) -> vec3f {
    let b = g.bodies[u32(lp.pos.w)];
    let m = mat3x3f(b.rot0.xyz, b.rot1.xyz, b.rot2.xyz);
    let up = normalize(lp.pos.xyz - b.ctr.xyz);
    let ub = up * m;
    let a = abs(ub);
    var axis = 2u;
    if (a.x >= a.y && a.x >= a.z) { axis = 0u; } else if (a.y >= a.z) { axis = 1u; }
    let f = frame_at(axis * 2u + select(0u, 1u, ub[axis] < 0.0), b, 0.0, 0.0);
    let r = length(lp.pos.xyz - b.ctr.xyz);
    let at = vec3f(atan(dot(ub, f.s) / dot(ub, f.n)), atan(dot(ub, f.t) / dot(ub, f.n)), log(r));
    let ob = (o - b.ctr.xyz) * m;
    let ro = length(ob);
    let q = vec3f(atan(dot(ob, f.s) / dot(ob, f.n)), atan(dot(ob, f.t) / dot(ob, f.n)), log(ro));
    // half a cell each way (an angle's metres are about the radius times it), and 2 cm
    let half = vec3f(0.5 * b.theta, 0.5 * b.theta, 0.5 * b.lnq) + 0.02 / r;
    let c = clamp(q, at - half, at + half);
    let tb = normalize(f.n + f.s * tan(c.x) + f.t * tan(c.y)) * exp(c.z);
    return m * tb + b.ctr.xyz;
}

fn light(p: vec3f, n: vec3f, body: u32, px: vec2u) -> Light {
    let up = normalize(p - g.bodies[body].ctr.xyz);
    let sd = normalize(g.sun_pos.xyz - p);
    // eclipses: every other body's sphere over the sun's disc; what a body with air hides of it comes round
    // its rim bent and reddened (a lunar eclipse: the whole moon copper, ADR 0037)
    var ecl = 1.0;
    var copper = 0.0;
    for (var i = 0u; i < 4u; i++) {
        if (i != body && ((g.n_lamps.z >> i) & 1u) == 1u) {
            let l = left_by(i, p, g.sun_pos.xyz, g.sun_pos.w);
            ecl *= l;
            if (g.bodies[i].sea >= 0.0) { copper = max(copper, 1.0 - l); }
        }
    }
    var o: Light;
    o.day = smoothstep(-0.2, 0.15, dot(up, sd)) * ecl;
    let mi = u32(g.moon_col.w);
    let moon = g.moon_col.rgb * select(1.0, 0.0, body == mi);
    o.l = vec3f(0.0);
    let ndl = dot(n, sd);
    // (the sun's light fades as it nears the horizon on a body with air, ADR 0038 C)
    let occ = left_by(body, p, g.sun_pos.xyz, g.sun_pos.w) * sunset(up, sd, body);
    if (ndl > 0.0 && occ > 0.0 && max(ecl, copper) > 0.0) {
        let ar = g.sun_pos.w / length(g.sun_pos.xyz - p);
        // soft: towards a point of the sun's disc; sharp: towards its middle
        var to_sun = sd;
        if ((g.opt.x & 1u) == 0u) { to_sun = in_disc(sd, ar, px); }
        let seen = occ * ray(p, n, to_sun, body, 1e5);
        o.sun = ecl * seen;
        o.l += (sun_on(body) * ecl + COPPER * copper) * ndl * seen * moon_gain(body) * ray_tint;
    }
    // the moon's light where the sun's is weak, through the same terrain
    if (o.day < 0.5 && max(moon.r, moon.b) > 0.001) {
        let mc = g.bodies[mi].ctr;
        let md = normalize(mc.xyz - p);
        let ndm = dot(n, md);
        if (ndm > 0.0) {
            var mv = 1.0;
            for (var i = 0u; i < 4u; i++) {
                if (i != mi && ((g.n_lamps.w >> i) & 1u) == 1u) { mv *= left_by(i, p, mc.xyz, mc.w); }
            }
            // faded in as the sun's ambient fades out, not switched on at once (ADR 0038 C)
            mv *= 1.0 - smoothstep(0.3, 0.5, o.day);
            if (mv > 0.0) {
                let mr = ray(p, n, md, body, 1e5);
                o.l += moon * ndm * mv * mr * ray_tint;
            }
        }
    }
    // the lamps: shadow rays to the LAMP_RAYS strongest here; the others get their mean visibility
    var top = array<u32, LAMP_RAYS>(NONE, NONE, NONE);
    var topw = array<f32, LAMP_RAYS>(0.0, 0.0, 0.0);
    var topl = array<vec3f, LAMP_RAYS>(vec3f(0.0), vec3f(0.0), vec3f(0.0));
    var rest = vec3f(0.0);
    var e0 = 0u;
    var e1 = 0u;
    let gc = vec3i(floor(p / LAMP_CELL)) + LAMP_G / 2;
    if (g.n_lamps.x > 0u && all(gc >= vec3i(0)) && all(gc < vec3i(LAMP_G))) {
        let ci = u32((gc.z * LAMP_G + gc.y) * LAMP_G + gc.x);
        e0 = lamp_cells[ci];
        e1 = lamp_cells[ci + 1u];
    }
    for (var e = e0; e < e1; e++) {
        let i = lamp_cells[e];
        let lp = lamps[i];
        let v = lp.pos.xyz - p;
        let d2 = dot(v, v);
        if (u32(lp.pos.w) != body || d2 > LAMP_R * LAMP_R) { continue; }
        let ndv = dot(n, v);
        if (ndv <= 0.0) { continue; }
        // its direct light here before its shadow, worked out once (ADR 0025, H)
        var lj = lamp_light(lp, d2) * (ndv * inverseSqrt(d2));
        var wj = dot(lj, vec3f(0.3, 0.5, 0.2));
        if (wj <= 0.0) { continue; }
        // weaker than the short list's weakest (it is sorted, strongest first): straight to `rest`
        if (wj <= topw[LAMP_RAYS - 1u]) {
            rest += lj;
            continue;
        }
        // keep the strongest: insert into the short list, the weakest falls to `rest`
        var j = i;
        for (var k = 0u; k < LAMP_RAYS; k++) {
            if (wj > topw[k]) {
                let tj = top[k];
                let tw = topw[k];
                let tl = topl[k];
                top[k] = j;
                topw[k] = wj;
                topl[k] = lj;
                j = tj;
                wj = tw;
                lj = tl;
            }
        }
        if (j != NONE) { rest += lj; }
    }
    var seen_n = 0.0;
    var seen_v = 0.0;
    for (var k = 0u; k < LAMP_RAYS; k++) {
        if (top[k] == NONE) { continue; }
        let lp = lamps[top[k]];
        // aimed from where the ray starts: from p, the 2 cm offset carried its end into the lamp by its corners
        // (bug 2026-11-10_19-45-23)
        let ro = ray_origin(p, n);
        let v = lamp_target(lp, ro) - ro;
        let d = length(v);
        let vis = select(ray(p, n, v / d, body, d - LAMP_STOP), 1.0, d < LAMP_STOP);
        o.l += topl[k] * vis * select(ray_tint, vec3f(1.0), d < LAMP_STOP);
        seen_n += 1.0;
        seen_v += vis;
    }
    if (seen_n > 0.0) { o.l += rest * (seen_v / seen_n); }
    return o;
}

// what a ray from p sees: the terrain it hits, lit, or the sky. A metal's reflection (bounce false) sees the
// sun's disc and lamps' faces; a bounce ray sees the sky's light only, and lamps' faces as nothing, since a
// lamp's light is already direct light
fn seen(p: vec3f, d: vec3f, body: u32, px: vec2u, bounce: bool, tmax: f32) -> vec3f {
    let bb = g.bodies[body];
    let db = d * mat3x3f(bb.rot0.xyz, bb.rot1.xyz, bb.rot2.xyz);
    let h = trace(to_body(bb, p), db, body, tmax, 0u);
    // what the glass on the way lets through, before `light` traces again
    let tint = ray_tint;
    if (h.kind == RAY_QUERY_INTERSECTION_NONE) { return select(sky_colour(d), sky_light(p, body), bounce) * tint; }
    let c = chunks[h.instance_custom_data];
    let q = hit_quad(h.instance_custom_data, h.geometry_index, h.primitive_index);
    let dir = (q.x >> 28u) & 7u;
    let a = dir / 2u;
    let b = g.bodies[(c.info >> 8u) & 255u];
    let gp = grid(c, quad_corner(q, 0u, 0.0));
    let f = frame_at(c.info & 7u, b, gp.x, gp.y);
    var hn = f.up;
    if (a == 0u) { hn = normalize(f.s - f.n * f.ta); }
    if (a == 1u) { hn = normalize(f.t - f.n * f.tb); }
    if ((dir & 1u) == 1u) { hn = -hn; }
    let m = q.y & 255u;
    if (((looks[m].info >> 24u) & 1u) == 1u) { return select(seen_colour(m, vec3f(0.0)), vec3f(0.0), bounce) * tint; }
    let l = light(p + d * h.t, brot(b, hn), body, px);
    // the light from around at the hit: a flat share, the second bounce
    return seen_colour(m, season(b, p + d * h.t)) * (l.l + ambient(l.day) * select(1.0, 0.35, bounce)) * tint;
}

// ---- the bounce (ADR 0022) ----
// One cosine-weighted ray per 2x2 pixels, from one of them in turn over four frames (BOUNCE_AT); what it
// sees is that pixel's light from around, noisy.
// post.wgsl gathers it over frames and smooths it, and the composite multiplies it by the albedo.
@group(1) @binding(0) var gb_t: texture_2d<u32>;
@group(1) @binding(1) var albedo_t: texture_2d<f32>;
@group(1) @binding(2) var noisy: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8)
fn gi_trace(@builtin(global_invocation_id) id: vec3u) {
    let half = textureDimensions(noisy);
    if (id.x >= half.x || id.y >= half.y) { return; }
    let size = textureDimensions(gb_t);
    var at = array<vec2u, 4>(vec2u(0u, 0u), vec2u(1u, 1u), vec2u(1u, 0u), vec2u(0u, 1u));
    let q = min(id.xy * 2u + at[g.n_lamps.y % 4u], size - 1u);
    let ge = textureLoad(gb_t, q, 0).xy;
    let gb = vec4f(gb_normal(ge.x), bitcast<f32>(ge.y));
    if (gb.w <= 0.0) {
        textureStore(noisy, id.xy, vec4f(0.0));
        return;
    }
    let body = u32(round(textureLoad(albedo_t, q, 0).a * 3.0));
    let n = gb.xyz;
    let uv = (vec2f(q) + 0.5) / vec2f(size);
    let w = g.inv_vp * vec4f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 1.0, 1.0);
    let p = normalize(w.xyz / w.w) * gb.w;
    let f = g.n_lamps.y;
    let px = q + vec2u(f * 7919u, f * 6271u);
    // a cosine-weighted direction around n
    let t = normalize(cross(n, select(vec3f(0., 1., 0.), vec3f(1., 0., 0.), abs(n.y) > 0.9)));
    let bt = cross(n, t);
    let u1 = hash2(px);
    let ph = 2.0 * PI * hash2(px + vec2u(4513u, 9157u));
    let r = sqrt(u1);
    let d = normalize(t * (r * cos(ph)) + bt * (r * sin(ph)) + n * sqrt(1.0 - u1));
    let o = p + n * (0.02 + length(p) * 2e-5);
    textureStore(noisy, id.xy, vec4f(seen(o, d, body, px, true, 400.0), 1.0));
}


fn hash(p: vec3i) -> f32 {
    var h = u32(p.x) * 73856093u ^ u32(p.y) * 19349663u ^ u32(p.z) * 83492791u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    return f32(h & 1023u) / 1023.0;
}

// Block look: 16 texels per 1 m block (ADR 0033) (ADR 0002, prototypes/block-look/).
// shortcut: texels are hashed here instead of read from 16x16 images, until content brings a texture atlas.
const TEXELS: f32 = 16.0;
const PLAIN: u32 = 0u;
const GRASS: u32 = 1u;
const STONE: u32 = 2u;

// The patterns with parts (ADR 0039), the port of `part` in tools/review/workbench.html: which part texel
// (x, y) of a 16 x 16 face is (0 the colour, 1 colour2, 2 colour3), the part it blends towards, how far, and a
// shade. y counts down from a side's top edge, as on the page. Whole numbers and `hash` only.
fn part(look: Look, pattern: u32, dir: u32, cell: vec3i, x: i32, y: i32) -> vec4f {
    let s = i32(look.size);
    let amt = look.amount;
    let cz = cell.x * 7 + cell.y * 13 + cell.z * 5 + i32(dir);
    let e = min(min(x, y), min(15 - x, 15 - y));
    switch (pattern) {
        case 3u: { // pebbles
            let h = hash(vec3i(x + cz * 16, y, 31));
            if (h < amt * 0.25) { return vec4f(1.0, 1.0, 0.0, 1.0); }
            if (h > 1.0 - amt * 0.1) { return vec4f(2.0, 2.0, 0.0, 1.0); }
        }
        case 4u: { // speckle
            let cx = x / s;
            let cy = y / s;
            if (hash(vec3i(cx + cz, cy, 91)) < amt) {
                let ox = cx * s + i32(floor(hash(vec3i(cx + cz, cy, 17)) * f32(s)));
                let oy = cy * s + i32(floor(hash(vec3i(cx + cz, cy, 23)) * f32(s)));
                let ax = f32(abs(x - ox));
                let ay = f32(abs(y - oy));
                let dist = max(ax, ay) + min(ax, ay) / 2.0;
                let r = f32(s) * 0.42;
                if (dist <= r * 0.4) { return vec4f(2.0, 2.0, 0.0, 1.0); }
                if (dist <= r) { return vec4f(1.0, 1.0, 0.0, 1.0); }
            }
        }
        case 5u: { // frame
            if (e < s) { return vec4f(1.0, 1.0, 0.0, 1.0); }
            let t = min(1.0, f32(e - s) / f32(max(1, 7 - s)));
            // floor(v + 0.5): the page's Math.round, not WGSL's round half to even
            return vec4f(0.0, 2.0, floor(t * amt * 4.0 + 0.5) / 4.0, 1.0);
        }
        case 6u: { // weave
            let off = i32(floor(hash(vec3i(cz, y, 5)) * f32(s)));
            if (hash(vec3i((x + off) / s + cz * 3, y, 9)) < amt) { return vec4f(1.0, 1.0, 0.0, 1.0); }
        }
        case 7u: { // planks
            let board = x / s + cz;
            if (x % s == 0 || y == i32(floor(hash(vec3i(board, 0, 3)) * 16.0))) { return vec4f(1.0, 1.0, 0.0, 1.0); }
            return vec4f(0.0, 1.0, select(0.0, 0.35, hash(vec3i(board, y, 4)) < amt * 0.3), 0.92 + 0.16 * hash(vec3i(board, 1, 4)));
        }
        case 8u: { // tiles
            if (x % s == 0 || y % s == 0) { return vec4f(1.0, 1.0, 0.0, 1.0); }
            if (hash(vec3i(x + cz * 16, y, 41)) < amt * 0.3) { return vec4f(2.0, 2.0, 0.0, 1.0); }
        }
        case 9u: { // bricks
            let h = max(2, s);
            let row = y / h;
            let xo = x + (row % 2) * h;
            if (y % h == 0 || xo % (2 * h) == 0) { return vec4f(1.0, 1.0, 0.0, 1.0); }
            return vec4f(0.0, 2.0, select(0.0, amt, y % h == 1), 0.94 + 0.12 * hash(vec3i(xo / (2 * h) + cz, row, 6)));
        }
        case 10u: { // rings
            if ((e / s) % 2 == 1) { return vec4f(1.0, 1.0, 0.0, 1.0); }
        }
        case 11u: { // leaves (their holes are `leaf_hole`'s)
            let h = hash(vec3i(x / 2 + cz * 8, y / 2, 71));
            if (h < 0.3) { return vec4f(1.0, 1.0, 0.0, 1.0); }
            if (h > 0.9) { return vec4f(2.0, 2.0, 0.0, 1.0); }
        }
        default: {}
    }
    return vec4f(0.0, 0.0, 0.0, 1.0);
}

// The season of the looks at p (from the camera, its frame) on body b, the port of engine `sky::season_at`:
// x the year's phase in quarter turns (0 spring, 1 summer, 2 autumn, 3 winter; the south half a year on), y how
// much of a season the place has, 0 near the equator to 1 from 35 degrees on (the sines of 10 and 35 degrees);
// z the column's own share of its block's stagger, -1..1, 0 until `stagger` sets it.
fn season(b: Body, p: vec3f) -> vec3f {
    if (b.year < 0.0) { return vec3f(0.0); }
    // the Studio's year slider at a latitude (view.rs `season_year`): that phase, whole, everywhere
    if (b.year >= 8.0) { return vec3f(b.year - 8.0, 1.0, 0.0); }
    let z = normalize(to_body(b, p)).z;
    return vec3f(b.year + select(0.0, 2.0, z < 0.0), smoothstep(0.1736, 0.5736, abs(z)), 0.0);
}
// How far the column `col` (columns of level 0, so a tree keeps its season as its chunk's level changes) of
// face `face` runs ahead of or behind its place's season, -1..1: two thirds by its patch of 4 x 4 columns, a
// third its own, so a tree turns nearly as one and trees turn one by one.
fn stagger(col: vec2i, face: u32) -> f32 {
    let f = i32(face);
    return (hash(vec3i(col.x >> 2u, col.y >> 2u, 77 + f)) * 2.0 + hash(vec3i(col.x, col.y, 87 + f)) - 1.5) / 1.5;
}
// Block m's look in season sn (ADR 0061): the two nearest seasons' rows mixed by the phase, and that with its
// own look by the place's share of a season; sn.z of the block's stagger ahead or behind. A block without
// seasons, or a place without, is its row untouched.
fn look_in(m: u32, sn: vec3f) -> Look {
    var a = looks[m];
    let seasons = a.size >> 8u;
    a.size &= 255u;
    if (seasons == 0u || sn.y == 0.0) { return a; }
    let r = season_rows(seasons, sn);
    let f = r.f;
    let x = looks[r.x + m];
    let y = looks[r.y + m];
    a.colour = mix(a.colour, mix(x.colour, y.colour, f), sn.y);
    a.colour2 = mix(a.colour2, mix(x.colour2, y.colour2, f), sn.y);
    a.colour3 = mix(a.colour3, mix(x.colour3, y.colour3, f), sn.y);
    a.amount = mix(a.amount, mix(x.amount, y.amount, f), sn.y);
    a.contrast = mix(a.contrast, mix(x.contrast, y.contrast, f), sn.y);
    return a;
}

// The two nearest seasons' look rows (their offsets, 256 a season) and how far from the first to the second,
// for a block's `seasons` (`Look::size` from bit 8: its stagger from bit 16) in season sn
struct SeasonRows { x: u32, y: u32, f: f32 };
fn season_rows(seasons: u32, sn: vec3f) -> SeasonRows {
    let q = fract((sn.x + sn.z * f32(seasons >> 8u) / 1024.0) * 0.25) * 4.0;
    let i = min(u32(q), 3u);
    let j = (i + 1u) & 3u;
    // phase 0 is spring's row (1), 1 the block's own (0), then autumn's and winter's
    return SeasonRows(select(i, 1u - i, i < 2u) * 256u, select(j, 1u - j, j < 2u) * 256u, q - f32(i));
}

// look row r's paint of face `dir` (16 bits: layer | variants - 1 << 12 | painted << 15), 0 for none (a
// season's row without paint of its own has the base's, `looks_of`)
fn face_paint(r: u32, dir: u32) -> u32 {
    return (select(looks[r].tex, looks[r].tex2, dir == 5u) >> select(0u, 16u, dir < 4u)) & 65535u;
}

fn part_colour(look: Look, i: f32) -> vec3f {
    return select(select(look.colour, look.colour2, i > 0.5), look.colour3, i > 1.5);
}

// `texel_a`'s colour
fn texel(mat: u32, dir: u32, cell: vec3i, t: vec2f, hgt: f32, sn: vec3f) -> vec3f {
    return texel_a(mat, dir, cell, t, hgt, sn).rgb;
}

const LEAVES: u32 = 11u;
fn is_glass(m: u32) -> bool { return ((looks[m & 255u].info >> 26u) & 1u) == 1u; }
// an unpainted glass block's alpha at texel (x, y): a clear middle, a rim one texel wide, so a wall reads as panes
fn glass_pane(x: i32, y: i32) -> f32 {
    return select(0.3, 0.8, x == 0 || y == 0 || x == 15 || y == 15);
}
// floor(a / b) for b > 0, as Rust's div_euclid (the Studio's `pattern::hole`)
fn fdiv(a: i32, b: i32) -> i32 { return (a - select(0, b - 1, a < 0)) / b; }
// a hole of the leaves pattern at texel ti of face dir of the block at cell (ADR 0069, studio `pattern::hole`):
// hashed by the texel's place on the face's plane, so the two faces between neighbouring leaf blocks agree
fn leaf_hole(look: Look, dir: u32, cell: vec3i, ti: vec2i) -> bool {
    // flat colours: one texel a face, never a hole (a face whose middle is a hole would be all hole colour)
    if ((look.info & 255u) != LEAVES || (g.opt.x & 4u) != 0u) { return false; }
    let a = dir / 2u;
    let s = max(i32(look.size & 255u), 1);
    let pu = cell[(a + 1u) % 3u] * 16 + ti.x;
    let pv = cell[(a + 2u) % 3u] * 16 + ti.y;
    let plane = cell[a] + select(0, 1, (dir & 1u) == 0u);
    return hash(vec3i(fdiv(pu, s), fdiv(pv, s), plane * 4 + i32(a) + 61)) < look.amount;
}

// a pixel-art texel on an N x N grid per block face: 3 tones of noise, grass tops, a grass fringe on the
// upper rows of a grass block's sides, dark crack texels in stone, the patterns with parts; a block with a
// `side` takes that block's look on its sides and foot. a: 0 for an empty texel (a painting's alpha 0, a leaves
// hole, ADR 0069), whose colour is what an opaque block or fast leaves show there: the block's colour, darker
fn texel_a(mat: u32, dir: u32, cell: vec3i, t: vec2f, hgt: f32, sn: vec3f) -> vec4f {
    let n = TEXELS;
    let ti = vec2i(floor(t * n));
    let h = hash(cell * i32(n) + vec3i(ti.x, ti.y, i32(dir)));
    let tone = 0.82 + 0.12 * floor(h * 3.0);
    let m0 = mat & 255u;
    var m = m0;
    // a painted face: its own file, else the block its side and foot take (ADR 0045)
    var painted = face_paint(m, dir);
    if (painted == 0u && (looks[m].info & 255u) == GRASS && dir != 4u) {
        // a side of a grass block takes its side block's look, with 2-4 rows of grass over the top edge
        // hashed by the texel's column along the side: t is (z, x) on a y face, so ti.x there is the row, and
        // the fringe came out as stray rows of grass down the side (bug 2026-11-11_17-48-16, ADR 0041)
        let along = select(ti.x, ti.y, dir / 2u == 1u);
        let fringe = 2.0 + floor(hash(cell * i32(n) + vec3i(along, 7, i32(dir))) * 3.0);
        if (floor(hgt * n) < n - fringe) { m = (looks[m].info >> 8u) & 255u; }
    } else if (painted == 0u && dir != 4u) {
        // any other block's side is its own id unless it names one (the mask drops metal and light)
        m = (looks[m].info >> 8u) & 255u;
    }
    painted = face_paint(m, dir);
    // on a side, x runs left to right as seen from outside and y down from its top edge (t is (y, z) on an x
    // face, (z, x) on a y face; along +y on +x and along +x on -y read rightwards, the other two are turned)
    var x = ti.x;
    var y = ti.y;
    if (dir < 4u) {
        let on_y = dir / 2u == 1u;
        x = select(ti.x, ti.y, on_y);
        y = 15 - select(ti.y, ti.x, on_y);
        if (dir == 1u || dir == 2u) { x = 15 - x; }
    }
    var c: vec4f;
    if (painted != 0u) {
        c = paint_texel(painted, cell, x, y);
        if (c.a < 0.5 && !is_glass(m)) { c = vec4f(row_seen(m) * 0.6, 0.0); }
    } else {
        let lk = look_in(m, sn);
        if (leaf_hole(lk, dir, cell, ti)) {
            c = vec4f(lk.colour * 0.6, 0.0);
        } else {
            c = vec4f(pattern_texel(lk, dir, cell, x, y, h, tone), select(1.0, glass_pane(x, y), is_glass(m)));
        }
    }
    // a block a season paints (bit 9 of its size, `looks_of`): the two nearest seasons' rows, each its paint of
    // this face or else the look above, mixed as look_in mixes; a row without paint is the summer look's face,
    // so a grass side a season does not paint stays its side block's
    let size = looks[m0].size;
    if ((size & 512u) == 0u || sn.y == 0.0) { return c; }
    let r = season_rows(size >> 8u, sn);
    var cx = c;
    var cy = c;
    let px = face_paint(r.x + m0, dir);
    let py = face_paint(r.y + m0, dir);
    if (px != 0u) { cx = paint_texel(px, cell, x, y); }
    if (py != 0u) { cy = paint_texel(py, cell, x, y); }
    let mixed = mix(c, mix(cx, cy, r.f), sn.y);
    // the empty texels of the season nearest in the mix
    return vec4f(mixed.rgb, select(0.0, 1.0, mixed.a >= 0.5));
}

// texel (x, y) of a painted face (`face_paint`): one of its variants by the cell's hash, the same on every face;
// a 0 for an empty pixel (a cutout block's, ADR 0069)
fn paint_texel(painted: u32, cell: vec3i, x: i32, y: i32) -> vec4f {
    let nv = ((painted >> 12u) & 3u) + 1u;
    let v = min(u32(hash(cell + vec3i(0, 0, 4099)) * f32(nv)), nv - 1u);
    let p = vec2u(clamp(vec2i(x, y), vec2i(0), vec2i(15)));
    return unpack4x8unorm(tex[((painted & 4095u) + v) * 256u + p.y * 16u + p.x]);
}

// a look's pattern texel (x, y) of face `dir`; h the texel's hash, tone its noise
fn pattern_texel(look: Look, dir: u32, cell: vec3i, x: i32, y: i32, h: f32, tone: f32) -> vec3f {
    let pattern = look.info & 255u;
    // contrast 1 is exactly the look before ADR 0039
    let k = select(1.0 + look.contrast * (tone - 1.0), tone, look.contrast == 1.0);
    var c = look.colour * k;
    if (pattern == STONE && h > 0.94) { c *= 0.6; }
    if (pattern == GRASS && h > 0.9) { c *= 1.15; }
    if (pattern > STONE) {
        let q = part(look, pattern, dir, cell, x, y);
        c = mix(part_colour(look, q.x), part_colour(look, q.y), q.z) * (k * q.w);
    }
    return c;
}

// the terrain: opaque quads (`fs`), with early depth; the cutout blocks' quads of level 0 (`fs_cut`, ADR 0069),
// which leave an empty texel out. A cutout block on a coarse level is its one colour (no stretched holes), and
// through `fs` (fast leaves) an empty texel shows the block's colour, darker
@fragment
fn fs(i: VOut) -> GOut {
    return terrain_lit(i, terrain_texel(i).rgb);
}

@fragment
fn fs_cut(i: VOut) -> GOut {
    let c = terrain_texel(i);
    if (c.a < 0.5) {
        discard;
    }
    return terrain_lit(i, c.rgb);
}

fn terrain_texel(i: VOut) -> vec4f {
    let a = i.dir / 2u;
    let sgn = select(1.0, -1.0, (i.dir & 1u) == 1u);
    let c = chunks[i.ci];
    let cell = vec3i(floor(i.local - axis(a) * sgn * 0.5)) + (c.base >> vec3u((c.info >> 3u) & 31u)) + vec3i(i32(c.info & 7u) * 977);
    let f = fract(i.local);
    // flat colours: every pixel of a face takes its middle texel
    let t = select(vec2f(f[(a + 1u) % 3u], f[(a + 2u) % 3u]), vec2f(0.5), (g.opt.x & 4u) != 0u);
    let body = (c.info >> 8u) & 255u;
    var sn = season(g.bodies[body], i.rel);
    let level = (c.info >> 3u) & 31u;
    sn.z = stagger((vec2i(floor(i.local.xy - axis(a).xy * sgn * 0.5)) + (c.base.xy >> vec2u(level))) << vec2u(level), c.info & 7u);
    if (level > 0u && ((looks[i.mat & 255u].info >> 25u) & 1u) == 1u) { return vec4f(seen_colour(i.mat & 255u, sn), 1.0); }
    // far glass: its one colour, half clear (ADR 0081)
    if (level > 0u && is_glass(i.mat)) { return vec4f(seen_colour(i.mat & 255u, sn), 0.45); }
    let col = texel_a(i.mat, i.dir, cell, t, f.z, sn);
    // flat colours: one texel a face, never a hole; glass keeps its clearness
    return vec4f(col.rgb, select(col.a, 1.0, (g.opt.x & 4u) != 0u && !is_glass(i.mat)));
}

fn terrain_lit(i: VOut, col: vec3f) -> GOut {
    let c = chunks[i.ci];
    let body = (c.info >> 8u) & 255u;
    let look = looks[i.mat & 255u];
    let n = normalize(i.normal);
    var o: GOut;
    o.gbuf = gb_pack(n, length(i.rel));
    if (((look.info >> 24u) & 1u) == 1u) {
        // it gives light: it shows its own, and takes none from around
        o.direct = vec4f(col * 1.15, 1.0);
        o.albedo = vec4f(0.0, 0.0, 0.0, f32(body) / 3.0);
        return o;
    }
    let px = vec2u(i.clip.xy);
    let l = light(i.rel, n, body, px);
    var out = col * l.l;
    var diffuse = 1.0;
    let metal = f32((look.info >> 16u) & 255u) / 255.0;
    if (metal > 0.0 && (g.opt.x & 2u) == 0u) {
        // a metal mirrors what is around it, tinted by its colour, more so at a grazing angle
        let v = -normalize(i.rel);
        let fres = metal + (1.0 - metal) * pow(1.0 - max(dot(n, v), 0.0), 5.0);
        let r = seen(i.rel + n * (0.02 + length(i.rel) * 2e-5), reflect(-v, n), body, px, false, 4000.0);
        out = mix(out, r * mix(vec3f(1.0), col, metal), fres);
        diffuse = 1.0 - fres;
        // and the sun's glint, where the sun reaches
        out += col * g.sun_col.rgb * 3.0 * l.sun * metal * pow(max(dot(reflect(-v, n), normalize(g.sun_pos.xyz - i.rel)), 0.0), 48.0);
    }
    o.direct = vec4f(out, 1.0);
    // the corner occlusion darkens the light from around a little more, in detail the bounce smooths away
    o.albedo = vec4f(col * diffuse * mix(1.0, i.ao, 0.5), f32(body) / 3.0);
    return o;
}

// glass (ADR 0081): after the light, blended over the frame as the water is, lit by the same direct light and
// the flat light from around; the sky in it at a grazing angle, the sun's glint
@fragment
fn fs_glass(i: VOut) -> @location(0) vec4f {
    let t = terrain_texel(i);
    if (t.a < 0.01) { discard; }
    let body = (chunks[i.ci].info >> 8u) & 255u;
    let v = normalize(-i.rel);
    let n = normalize(i.normal);
    let l = light(i.rel, n, body, vec2u(i.clip.xy));
    let fres = 0.04 + 0.96 * pow(1.0 - max(dot(n, v), 0.0), 5.0);
    var col = t.rgb * (l.l + ambient(l.day) + 0.1 * l.day);
    col = mix(col, g.sky.rgb + vec3f(0.17, 0.13, 0.01) * l.day, fres * 0.6);
    col += g.sun_col.rgb * 1.5 * l.sun * pow(max(dot(reflect(-g.sun.xyz, n), v), 0.0), 200.0);
    let a = clamp(t.a + fres * 0.5 * (1.0 - t.a), 0.0, 0.97);
    var lit = shoulder(col);
    if (g.opt.y != 1000u) { lit = pow(lit, vec3f(1000.0 / f32(g.opt.y))); }
    return vec4f(lit, a);
}

// the G-buffer: direct light; albedo, a the body / 3; normal (camera frame) and distance from the camera,
// 0 for the sky, packed by `gb_pack`
struct GOut {
    @location(0) direct: vec4f,
    @location(1) albedo: vec4f,
    @location(2) gbuf: vec2u,
};

// the normal folded onto an octahedron as two 16-bit numbers, and the distance's f32 bits: 8 bytes a pixel
// instead of 16 (ADR 0025, J2)
fn gb_pack(n: vec3f, d: f32) -> vec2u {
    var o = n.xy / (abs(n.x) + abs(n.y) + abs(n.z));
    if (n.z < 0.0) { o = (1.0 - abs(o.yx)) * select(vec2f(-1.0), vec2f(1.0), o >= vec2f(0.0)); }
    return vec2u(pack2x16snorm(o), bitcast<u32>(d));
}

// the G-buffer's normal, folded onto an octahedron as two 16-bit numbers (ADR 0025, J2; `gb_pack` in shader.wgsl)
fn gb_normal(e: u32) -> vec3f {
    let o = unpack2x16snorm(e);
    var n = vec3f(o, 1.0 - abs(o.x) - abs(o.y));
    let t = max(-n.z, 0.0);
    n.x += select(t, -t, n.x >= 0.0);
    n.y += select(t, -t, n.y >= 0.0);
    return normalize(n);
}

// bright light rolls off above 0.8 instead of clipping; everything under it is left as it is
fn shoulder(c: vec3f) -> vec3f {
    let k = 0.8;
    return select(c, k + (1.0 - k) * (1.0 - exp(-(c - k) / (1.0 - k))), c > vec3f(k));
}

// ---- water (ADR 0020) ----
// The generated sea is drawn from the chunks' trailing quads (`vs_sea`), laid at the exact sea level; the
// water mesh of tiles that hold stored water (`vs_water`, crates/client/src/water.rs) moves its heights from
// `prev` to `cur` over one server tick. Both are blended over the terrain, seen from above and below.
struct WQuad { i: i32, j: i32, info: u32, t0: f32, prev: vec4f, cur: vec4f };
@group(0) @binding(4) var<storage, read> wq: array<WQuad>;
const MOVE_S: f32 = 0.05;

struct WOut {
    @builtin(position) clip: vec4f,
    @location(0) rel: vec3f, // from the camera
    @location(1) normal: vec3f,
    @location(2) @interpolate(flat) side: u32,
    @location(3) @interpolate(flat) body: u32,
    // a river's flow (W5): xyz its direction from the camera times its speed (m/s), w the distance along it (m)
    @location(4) flow: vec4f,
};

// the outward direction at grid point (gi, gj) of a face, and the face's normal and in-face axes
struct Frame { up: vec3f, n: vec3f, s: vec3f, t: vec3f, ta: f32, tb: f32 };
// a vector in the body's frame, in the camera's
fn brot(b: Body, v: vec3f) -> vec3f {
    return mat3x3f(b.rot0.xyz, b.rot1.xyz, b.rot2.xyz) * v;
}

fn frame_at(face: u32, b: Body, gi: f32, gj: f32) -> Frame {
    var fnv = array<vec3f, 6>(vec3f(1., 0., 0.), vec3f(-1., 0., 0.), vec3f(0., 1., 0.), vec3f(0., -1., 0.), vec3f(0., 0., 1.), vec3f(0., 0., -1.));
    var fsv = array<vec3f, 6>(vec3f(0., 1., 0.), vec3f(0., 0., 1.), vec3f(0., 0., 1.), vec3f(1., 0., 0.), vec3f(1., 0., 0.), vec3f(0., 1., 0.));
    var ftv = array<vec3f, 6>(vec3f(0., 0., 1.), vec3f(0., 1., 0.), vec3f(1., 0., 0.), vec3f(0., 0., 1.), vec3f(0., 1., 0.), vec3f(1., 0., 0.));
    var f: Frame;
    f.n = fnv[face];
    f.s = fsv[face];
    f.t = ftv[face];
    f.ta = tan((gi - b.half_n) * b.theta);
    f.tb = tan((gj - b.half_n) * b.theta);
    f.up = normalize(f.n + f.s * f.ta + f.t * f.tb);
    return f;
}

fn wout(bi: u32, up: vec3f, layer: f32, normal: vec3f, side: u32) -> WOut {
    let b = g.bodies[bi];
    let rel = brot(b, (up * (b.r0 * exp(layer * b.lnq)) - b.cam_hi) - b.cam_lo);
    var o: WOut;
    o.clip = g.view_proj * vec4f(rel, 1.0);
    o.rel = rel;
    o.normal = brot(b, normal);
    o.side = side;
    o.body = bi;
    o.flow = vec4f(0.0);
    return o;
}

// A sea quad is drawn as SEA_SUB x SEA_SUB pieces: one flat chord sags under the sphere (0.25 m for a 90 m
// quad), and where a neighbour's corner meets its edge mid-way that sag opened a seam onto the seabed. In
// pieces the sag is SEA_SUB^2 times smaller (4 mm). Each piece is an instance: chunk * SEA_SUB^2 + piece
// (main.rs SEA_SUB is the same).
const SEA_SUB: u32 = 8u;

@vertex
fn vs_sea(@builtin(vertex_index) vi: u32, @builtin(instance_index) inst: u32) -> WOut {
    let q = quads[vi / 6u];
    let k = vi % 6u;
    let ii = inst / (SEA_SUB * SEA_SUB);
    let piece = inst % (SEA_SUB * SEA_SUB);
    let sub = vec2f(f32(piece % SEA_SUB), f32(piece / SEA_SUB));
    let lo = q.x;
    let p = vec2f(f32(lo & 63u), f32((lo >> 6u) & 63u));
    let w = f32(((lo >> 18u) & 31u) + 1u);
    let h = f32(((lo >> 23u) & 31u) + 1u);
    var cu = array<f32, 6>(0., 1., 1., 0., 1., 0.);
    var cv = array<f32, 6>(0., 0., 1., 0., 1., 1.);
    let c = chunks[ii];
    let level = (c.info >> 3u) & 31u;
    let b = g.bodies[(c.info >> 8u) & 255u];
    let river = (lo >> 28u) == 5u;
    var at = (sub + vec2f(cu[k], cv[k])) / f32(SEA_SUB);
    if (river && w * h == 1.0) {
        // a river's column is small: one piece draws it, the others are empty
        if (piece != 0u) { var o: WOut; o.clip = vec4f(0.0, 0.0, -1.0, 1.0); return o; }
        at = vec2f(cu[k], cv[k]);
    }
    let s = f32(1u << level);
    let gp = vec2f(c.base.xy) + (p + at * vec2f(w, h)) * s;
    let f = frame_at(c.info & 7u, b, gp.x, gp.y);
    let bi = (c.info >> 8u) & 255u;
    if (river) {
        // its level at the column's middle and its fall a column along i and j (world.rs river_quads)
        let fall = vec2f(f32(i32(q.y << 7u) >> 25u), f32(i32(q.y) >> 25u)) / 256.0;
        let layer = f32(c.base.z) + f32(q.y & 0x3ffffu) / 64.0 + dot(fall, (at * vec2f(w, h) - 0.5) * s);
        let tilt = f.s * fall.x + f.t * fall.y;
        var o = wout(bi, f.up, layer, normalize(f.up - tilt * (b.lnq / b.theta)), 0u);
        let g2 = length(fall);
        if (g2 > 0.0) {
            // downhill, as fast as it is steep: 0.5 m/s on the flat to 3 m/s down rapids
            let d2 = -fall / g2;
            let speed = clamp(0.5 + 30.0 * g2, 0.5, 3.0);
            o.flow = vec4f(brot(b, normalize(-tilt)) * speed, dot(gp, d2));
        }
        return o;
    }
    // a far coarse cell that holds the sea level but is solid: the water lies a layer over its top, or the
    // top face hides it
    var layer = b.sea;
    if (bi == 0u) {
        // the tide (ADR 0067): the port of `Tide::at`, a dozen multiplications a vertex
        let l = dot(f.up, g.tide[0].xyz);
        let s = dot(f.up, g.tide[1].xyz);
        layer += g.tide[0].w * l * l + g.tide[1].w * s * s;
        // the waterline rule: never within 1/64 layer of a block's top, where the two would flicker; the
        // sea steps over it, 3 cm in an instant
        let fr = layer - round(layer);
        if (abs(fr) < 1.0 / 64.0) { layer = round(layer) + select(-1.0, 1.0, fr >= 0.0) / 64.0; }
    }
    if (q.y == 1u) {
        layer = max(layer, f32(c.base.z + (i32((lo >> 12u) & 63u) << level)) + 1.0);
    }
    return wout(bi, f.up, layer, f.up, 0u);
}

@vertex
fn vs_water(@builtin(vertex_index) vi: u32) -> WOut {
    let q = wq[vi / 6u];
    var tri = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
    let c = tri[vi % 6u];
    let hh = mix(q.prev, q.cur, clamp((g.sun.w - q.t0) / MOVE_S, 0.0, 1.0));
    let face = q.info & 7u;
    let bi = (q.info >> 3u) & 3u;
    let b = g.bodies[bi];
    let kind = (q.info >> 5u) & 7u;
    var du = 0.0;
    var dv = 0.0;
    var z = 0.0;
    if (kind == 0u) {
        var cu = array<f32, 4>(0., 1., 1., 0.);
        var cv = array<f32, 4>(0., 0., 1., 1.);
        du = cu[c];
        dv = cv[c];
        z = hh[c];
    } else {
        // the edge's two ends A and B; corners 0 A low, 1 B low, 2 B high, 3 A high
        var au = array<f32, 4>(0., 1., 0., 0.);
        var av = array<f32, 4>(0., 0., 0., 1.);
        var bu = array<f32, 4>(0., 1., 1., 1.);
        var bv = array<f32, 4>(1., 1., 0., 1.);
        let e = kind - 1u;
        let at_b = c == 1u || c == 2u;
        du = select(au[e], bu[e], at_b);
        dv = select(av[e], bv[e], at_b);
        var zs = array<f32, 4>(hh.z, hh.w, hh.y, hh.x);
        z = zs[c];
    }
    let f = frame_at(face, b, f32(q.i) + du, f32(q.j) + dv);
    var normal = f.up;
    if (kind == 0u) {
        // the slope from the corners: a layer is r * lnq high, a column r * theta wide
        let k = b.lnq / b.theta;
        let gi = (hh.y + hh.z - hh.x - hh.w) * 0.5;
        let gj = (hh.z + hh.w - hh.x - hh.y) * 0.5;
        normal = normalize(f.up - (f.s * gi + f.t * gj) * k);
    } else if (kind <= 2u) {
        normal = normalize(f.s - f.n * f.ta) * select(-1.0, 1.0, kind == 2u);
    } else {
        normal = normalize(f.t - f.n * f.tb) * select(-1.0, 1.0, kind == 4u);
    }
    return wout(bi, f.up, z, normal, u32(kind != 0u));
}

@fragment
fn fs_water(i: WOut) -> @location(0) vec4f {
    let v = normalize(-i.rel);
    var n = normalize(i.normal);
    if (dot(n, v) < 0.0) { n = -n; } // seen from under the surface
    let speed = length(i.flow.xyz);
    if (speed > 0.0) {
        // a river: ripples across it, running downstream at its speed
        let ph = i.flow.w - g.sun.w * speed;
        let a = sin(ph * 3.1) + 0.6 * sin(ph * 7.7 + 1.3) + 0.3 * sin(ph * 17.0 + 4.0);
        n = normalize(n + i.flow.xyz / speed * a * 0.07);
    }
    let facing = max(dot(n, v), 0.0);
    let fres = 0.08 + 0.92 * pow(1.0 - facing, 5.0);
    let l = light(i.rel, n, i.body, vec2u(i.clip.xy));
    var col = vec3f(0.06, 0.24, 0.36) * (l.l + ambient(l.day) + 0.1 * l.day);
    // the sky in it, and the sun's glint where the sun reaches
    col = mix(col, g.sky.rgb + vec3f(0.17, 0.13, 0.01) * l.day, fres * 0.7);
    col += g.sun_col.rgb * 1.5 * l.sun * pow(max(dot(reflect(-g.sun.xyz, n), v), 0.0), 120.0);
    var a = clamp(0.62 + 0.35 * fres, 0.0, 0.96);
    // rapids churn white
    col = mix(col, vec3f(0.85, 0.92, 1.0), 0.3 * smoothstep(1.5, 3.0, speed));
    if (i.side == 1u) {
        // a front or a fall: churned, lighter
        col = mix(col, vec3f(0.85, 0.92, 1.0), 0.25);
        a = max(a, 0.8);
    }
    var lit = shoulder(col);
    if (g.opt.y != 1000u) { lit = pow(lit, vec3f(1000.0 / f32(g.opt.y))); }
    return vec4f(lit, a);
}

// ---- the sky (ADR 0022) ----
// A triangle over the screen at the far plane, drawn first: the sky's colour here, the sun as a sphere 0.53
// degrees across with the air's glow around it by day. Bodies in front of it (the moon in an eclipse, the
// planet from space) are terrain, drawn over it.
struct SkyOut { @builtin(position) clip: vec4f, @location(0) ndc: vec2f };

@vertex
fn vs_sky(@builtin(vertex_index) vi: u32) -> SkyOut {
    let uv = vec2f(f32((vi << 1u) & 2u), f32(vi & 2u)) * 2.0 - 1.0;
    var o: SkyOut;
    o.clip = vec4f(uv, 0.0, 1.0);
    o.ndc = uv;
    return o;
}

@fragment
fn fs_sky(i: SkyOut) -> GOut {
    let w = g.inv_vp * vec4f(i.ndc, 1.0, 1.0);
    let d = normalize(w.xyz / w.w);
    // a pixel's width as an angle, before any branch (derivatives need uniform control flow)
    let px = max(length(fwidth(d)), 1e-5);
    var o: GOut;
    var st = vec3f(0.0);
    if ((g.opt.x & 8u) == 0u) { st = stars(d, px); }
    o.direct = vec4f(sky_colour(d) + st, 1.0);
    o.albedo = vec4f(0.0);
    o.gbuf = vec2u(0u);
    return o;
}

// the air the sun's light passes on its way down: white in space, yellow by day, orange to red low down
fn sun_tint() -> vec3f {
    let h = g.stars[1].w;
    let mass = min(1.0 / (max(h, 0.0) + 0.035), 28.0);
    return mix(vec3f(1.0), exp(-vec3f(0.02, 0.06, 0.16) * mass) / exp(-vec3f(0.02, 0.06, 0.16)), g.stars[0].w);
}

// the sky in direction d
fn sky_colour(d: vec3f) -> vec3f {
    let s = normalize(g.sun_pos.xyz);
    let rs = g.sun_pos.w / length(g.sun_pos.xyz);
    let ang = atan2(length(cross(d, s)), dot(d, s));
    let air = g.stars[0].w;
    let tint = sun_tint();
    var c = g.sky.rgb;
    // in the air the sky is lighter and a little warmer towards the horizon and deeper overhead: up is away
    // from the nearer body (their centres are from the camera)
    var ctr = g.bodies[0].ctr.xyz;
    if (g.bodies[1].ctr.w > 0.0 && dot(g.bodies[1].ctr.xyz, g.bodies[1].ctr.xyz) < dot(ctr, ctr)) { ctr = g.bodies[1].ctr.xyz; }
    let el = clamp(dot(d, normalize(-ctr)), 0.0, 1.0);
    c = c * mix(1.0, 0.8, el * air) + vec3f(0.55, 0.60, 0.62) * g.sky.w * pow(1.0 - el, 5.0) * 0.45;
    // a night in the air is never black: airglow and starlight, a faint blue, brighter towards the horizon,
    // on every night, so one with neither Sol nor Luna up is not black either (the developer's call on bug
    // 2026-11-11_17-47-11, ADR 0041); frame.rs `haze` takes its horizon value, so far ground fades into it
    c += NIGHT_GLOW * air * (1.0 - smoothstep(0.0, 0.15, g.sky.w)) * (0.5 + pow(1.0 - el, 3.0));
    let glow = exp(-ang * 12.0) * 0.35 + exp(-ang * 80.0) * 0.6;
    c += g.sun_col.rgb * tint * vec3f(1.0, 0.9, 0.75) * glow * g.sky.w;
    // a low sun reddens the sky around it
    let h = g.stars[1].w;
    let low = (1.0 - smoothstep(0.0, 0.3, h)) * smoothstep(-0.15, 0.0, h) * air;
    c = mix(c, vec3f(0.95, 0.45, 0.15) * max(g.sky.w, 0.15), 0.75 * exp(-ang * 1.6) * low);
    // in the air the sun's glare makes it look bigger and softer-edged; in space it is its own size, sharp
    let r = rs * (1.0 + 1.3 * air);
    let edge = mix(0.02, 0.35, air);
    // the aureole: in the air a bright halo a few sun widths across, under the disc's soft edge too
    c += tint * vec3f(1.0, 0.85, 0.6) * exp(-max(ang - r, 0.0) / (rs * 3.0)) * 0.6 * air;
    // out of the air nothing softens it: a glare tight round the disc and a faint wide one, so it reads as a
    // light and not a grey dot (bug 2026-11-10_20-45-58)
    c += vec3f(1.0, 0.95, 0.88) * (exp(-max(ang - r, 0.0) / (rs * 1.5)) * 0.5 + exp(-ang * 25.0) * 0.05) * (1.0 - air);
    let disc = 1.0 - smoothstep(1.0 - edge, 1.0, ang / r);
    if (disc > 0.0) {
        // limb darkening: the disc dims towards its edge
        let x = min(ang / r, 1.0);
        // brighter than anything around it; the air's colour shows through the glare
        let face = mix(0.8, 1.0, sqrt(1.0 - x * x)) * mix(vec3f(6.0, 5.9, 5.8), tint * vec3f(2.4, 2.1, 1.7), air);
        c = mix(c, face, disc);
    }
    return c;
}

// pcg3d (Jarzynski and Olano 2020): no rows or streaks where neighbouring cells' numbers correlate
fn pcg3d(p: vec3u) -> vec3u {
    var v = p * 1664525u + 1013904223u;
    v.x += v.y * v.z; v.y += v.z * v.x; v.z += v.x * v.y;
    v = v ^ (v >> vec3u(16u));
    v.x += v.y * v.z; v.y += v.z * v.x; v.z += v.x * v.y;
    return v;
}
fn hash4(p: vec3i) -> vec4f {
    let a = pcg3d(vec3u(p));
    let b = pcg3d(a ^ vec3u(0x9e3779b9u));
    return vec4f(vec4u(a, b.x) >> vec4u(8u)) / 16777215.0;
}

// one layer of stars: the sky in cells 1/n across, a star in `chance` of them
fn star_layer(d: vec3f, n: f32, chance: f32, px: f32, gain: f32) -> vec3f {
    let c = vec3i(floor(d * n));
    let a = hash4(c);
    if (a.x > chance) { return vec3f(0.0); }
    let b = hash4(c + vec3i(7919, 104729, 1299709));
    let s = normalize(vec3f(c) + 0.25 + 0.5 * a.yzw);
    let r = length(cross(d, s)) / px; // in pixels
    // most stars are faint; their colours from red giants to blue-white
    let bright = pow(b.x, 7.0) * gain + 0.015;
    let col = mix(mix(vec3f(1.0, 0.72, 0.5), vec3f(1.0, 0.95, 0.88), smoothstep(0.0, 0.5, b.y)), vec3f(0.72, 0.82, 1.0), smoothstep(0.55, 1.0, b.y));
    return col * bright * exp(-r * r * 1.2) * step(0.0, dot(d, s));
}

fn vnoise(p: vec3f) -> f32 {
    let i = vec3i(floor(p));
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    var v = 0.0;
    for (var k = 0u; k < 8u; k++) {
        let o = vec3i(i32(k & 1u), i32((k >> 1u) & 1u), i32(k >> 2u));
        let wgt = mix(1.0 - u, u, vec3f(o));
        v += hash4(i + o).x * wgt.x * wgt.y * wgt.z;
    }
    return v;
}

// the stars and the galaxy's band, fixed in the inertial frame, where the sky is dark enough to show them
fn stars(dc: vec3f, px: f32) -> vec3f {
    let dark = 1.0 - smoothstep(0.004, 0.08, dot(g.sky.rgb, vec3f(0.3, 0.5, 0.2)));
    // none in front of the sun's disc
    let to_sun = normalize(g.sun_pos.xyz);
    let behind = dot(dc, to_sun) > cos(g.sun_pos.w / length(g.sun_pos.xyz) * (1.0 + 1.3 * g.stars[0].w));
    if (dark <= 0.0 || behind) { return vec3f(0.0); }
    let d = normalize(mat3x3f(g.stars[0].xyz, g.stars[1].xyz, g.stars[2].xyz) * dc);
    let gal = normalize(vec3f(0.31, -0.46, 0.83));
    let lat = dot(d, gal);
    let band = exp(-lat * lat / 0.02) * (0.4 + vnoise(d * 7.0) * 0.8 + vnoise(d * 23.0) * 0.4);
    var c = vec3f(0.045, 0.045, 0.055) * band * (0.5 + vnoise(d * 60.0));
    c += star_layer(d, 110.0, 0.09 * (1.0 + 2.0 * band), px, 0.9);
    c += star_layer(d, 45.0, 0.05, px, 2.5);
    return c * dark;
}

// ---- effects (ADR 0031, crates/client/src/fx.rs) ----
// One instanced draw into the G-buffer: 36 vertices a particle, a cube (debris, or a whole cell) or a square
// facing the camera (its last 30 vertices folded away). Lit by `light()` as the terrain is, so the sun, its
// shadows and the lamps fall on them, and the bounce light after the G-buffer; not in the traced scene, so
// they cast no shadow. p: in its body's frame from the centre; info = block | body << 8 | look << 10 (0
// debris, 1 square, 2 block, 3 crack: a dig's cracks over a real cell, angle its share done, ADR 0034); a, b: a
// debris' texel seed, a square's colour (b, rgba8), a cell's columns (a = i | j << 15) and layer | face << 16 (b).
struct FxP { p: vec3f, size: f32, info: u32, angle: f32, a: u32, b: u32 };
@group(0) @binding(10) var<storage, read> fxp: array<FxP>;

struct FxOut {
    @builtin(position) clip: vec4f,
    @location(0) rel: vec3f,
    @location(1) normal: vec3f,
    @location(2) uvw: vec3f, // in the cube, 0..1 a side
    @location(3) @interpolate(flat) info: u32, // as FxP's, and the face's dir << 12
    @location(4) @interpolate(flat) a: u32,
    @location(5) @interpolate(flat) b: u32,
    @location(6) @interpolate(flat) k: f32, // a crack's share done
};

// corner k (0..5) of face `dir` (axis dir / 2, negative when odd) of the unit cube, 0..1: CCW from outside,
// in quad_corner's order
fn cube_corner(dir: u32, k: u32) -> vec3f {
    var cu = array<f32, 6>(0., 1., 1., 0., 1., 0.);
    var cv = array<f32, 6>(0., 0., 1., 0., 1., 1.);
    var du = cu[k];
    var dv = cv[k];
    if ((dir & 1u) == 1u) { du = cv[k]; dv = cu[k]; }
    let a = dir / 2u;
    let side = select(1.0, 0.0, (dir & 1u) == 1u);
    return axis(a) * side + axis((a + 1u) % 3u) * du + axis((a + 2u) % 3u) * dv;
}

fn fx_hash(x: u32) -> u32 {
    var h = x * 747796405u + 2891336453u;
    h = ((h >> ((h >> 28u) + 4u)) ^ h) * 277803737u;
    return (h >> 22u) ^ h;
}

// the rotation by an angle (cos cs, sin sn) about unit axis ax
fn turn_about(ax: vec3f, cs: f32, sn: f32) -> mat3x3f {
    let t = 1.0 - cs;
    return mat3x3f(
        vec3f(t * ax.x * ax.x + cs, t * ax.x * ax.y + sn * ax.z, t * ax.x * ax.z - sn * ax.y),
        vec3f(t * ax.x * ax.y - sn * ax.z, t * ax.y * ax.y + cs, t * ax.y * ax.z + sn * ax.x),
        vec3f(t * ax.x * ax.z + sn * ax.y, t * ax.y * ax.z - sn * ax.x, t * ax.z * ax.z + cs),
    );
}

@vertex
fn vs_fx(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> FxOut {
    let q = fxp[ii];
    let look = (q.info >> 10u) & 3u;
    let face = (q.info >> 16u) & 3u;
    let bi = (q.info >> 8u) & 3u;
    let b = g.bodies[bi];
    let dir = vi / 6u;
    let k = vi % 6u;
    var o: FxOut;
    o.info = q.info | dir << 12u;
    o.a = q.a;
    o.b = q.b;
    o.k = q.angle;
    var pb: vec3f; // in the body's frame from its centre
    var nb: vec3f;
    if (look >= 2u) {
        // a whole cell, scaled about its middle, its corners from the grid as `vs` places the terrain's
        let cell = vec3f(f32(q.a & 0x7fffu), f32((q.a >> 15u) & 0x7fffu), f32(q.b & 0xffffu));
        let c = cube_corner(dir, k);
        let gp = cell + 0.5 + (c - 0.5) * q.size;
        let f = frame_at(q.b >> 16u, b, gp.x, gp.y);
        pb = f.up * (b.r0 * exp(gp.z * b.lnq));
        let a = dir / 2u;
        nb = f.up;
        if (a == 0u) { nb = normalize(f.s - f.n * f.ta); }
        if (a == 1u) { nb = normalize(f.t - f.n * f.tb); }
        if ((dir & 1u) == 1u) { nb = -nb; }
        o.uvw = c;
    } else if (look == 1u && face != 2u) {
        // a square facing the camera: corners along the camera's right and the body's up there, turned by
        // `angle`; along its flight (face 1, ADR 0050): its long side along the flight seen from the camera,
        // `angle` metres longer, trailing behind it
        if (dir > 0u) {
            o.clip = vec4f(0.0, 0.0, -1.0, 1.0);
            return o;
        }
        let rel = brot(b, (q.p - b.cam_hi) - b.cam_lo);
        let fwd = normalize(rel);
        var along = brot(b, normalize(q.p));
        if (face == 1u) {
            along = brot(b, vec3f(f32(q.a & 1023u), f32((q.a >> 10u) & 1023u), f32((q.a >> 20u) & 1023u)) / 511.5 - 1.0);
        }
        var right = cross(fwd, along);
        if (dot(right, right) < 1e-6) { right = cross(fwd, vec3f(0.0, 0.0, 1.0)); }
        right = normalize(right);
        let up = cross(right, fwd);
        var cx = array<f32, 6>(-1., 1., 1., -1., 1., -1.);
        var cy = array<f32, 6>(-1., -1., 1., -1., 1., 1.);
        if (face == 1u) {
            o.rel = rel - up * (0.5 * q.angle) + right * (cx[k] * 0.5 * q.size) + up * (cy[k] * 0.5 * (q.size + q.angle));
        } else {
            let cs = cos(q.angle);
            let sn = sin(q.angle);
            o.rel = rel + (right * (cx[k] * cs - cy[k] * sn) + up * (cx[k] * sn + cy[k] * cs)) * (0.5 * q.size);
        }
        o.clip = g.view_proj * vec4f(o.rel, 1.0);
        o.normal = -fwd;
        o.uvw = vec3f(cx[k], cy[k], 0.0) * 0.5 + 0.5;
        return o;
    } else {
        // debris: a cube turned by `angle` about an axis of its own; a tumbling square (face 2) the same, a
        // tenth as thick
        let h = fx_hash(q.a);
        let ax = normalize(vec3f(f32(h & 255u), f32((h >> 8u) & 255u), f32((h >> 16u) & 255u)) - 127.25);
        let c = cube_corner(dir, k);
        let turn = turn_about(ax, cos(q.angle), sin(q.angle));
        pb = q.p + turn * ((c - 0.5) * q.size * vec3f(1.0, 1.0, select(1.0, 0.1, look == 1u)));
        nb = turn * (axis(dir / 2u) * select(1.0, -1.0, (dir & 1u) == 1u));
        o.uvw = c;
    }
    let rel = brot(b, (pb - b.cam_hi) - b.cam_lo);
    o.rel = rel;
    o.clip = g.view_proj * vec4f(rel, 1.0);
    if (look >= 2u) {
        // a whole cell over the real one (its mesh is late on purpose while it grows): a centimetre nearer in
        // depth only, so it wins over the terrain's faces where they meet; cracks two, over the very same faces
        let near = g.view_proj * vec4f(rel * max(1.0 - select(0.01, 0.02, look == 3u) / length(rel), 0.0), 1.0);
        o.clip.z = near.z / near.w * o.clip.w;
    }
    o.normal = brot(b, nb);
    return o;
}

// a particle's colour: a square's own; a cell's texels as `fs` has them; a debris cube's 2 x 2 texels a side
// from a place of its block picked by its seed
fn fx_colour(i: FxOut) -> vec3f {
    let look = (i.info >> 10u) & 3u;
    let mat = i.info & 255u;
    let dir = (i.info >> 12u) & 7u;
    let a = dir / 2u;
    let uv = vec2f(i.uvw[(a + 1u) % 3u], i.uvw[(a + 2u) % 3u]);
    let flat_t = (g.opt.x & 4u) != 0u;
    let sn = season(g.bodies[(i.info >> 8u) & 3u], i.rel);
    if (look == 1u) {
        // a square's colour; a texture's (ADR 0050): its block's top face times it (a billboard's uv is uvw.xy)
        if (((i.info >> 18u) & 1u) == 1u) {
            let st = select(uv, i.uvw.xy, ((i.info >> 16u) & 3u) != 2u);
            return texel(mat, 4u, vec3i(0), select(st, vec2f(0.5), flat_t), 0.5, sn) * unpack4x8unorm(i.b).rgb;
        }
        return unpack4x8unorm(i.b).rgb;
    }
    if (look >= 2u) {
        let cell = vec3i(i32(i.a & 0x7fffu), i32((i.a >> 15u) & 0x7fffu), i32(i.b & 0xffffu)) + vec3i(i32(i.b >> 16u) * 977);
        return texel(mat, dir, cell, select(uv, vec2f(0.5), flat_t), i.uvw.z, sn);
    }
    let h = fx_hash(i.a);
    let base = vec2f(f32(h % 14u), f32((h >> 4u) % 14u));
    // a third of a grass block's pieces show its top, the others its side, some with the fringe
    let d = select(0u, 4u, (h >> 8u) % 3u == 0u);
    let hgt = f32((h >> 12u) & 15u) / 16.0;
    let cell = vec3i(i32(h & 0xfffu), i32((h >> 12u) & 0xfffu), 7);
    return texel(mat, d, cell, select((base + floor(uv * 2.0) + 0.5) / TEXELS, vec2f(0.5), flat_t), hgt, sn);
}

fn fx_out(i: FxOut, col: vec3f, l: vec3f) -> GOut {
    let body = (i.info >> 8u) & 3u;
    var o: GOut;
    o.gbuf = gb_pack(normalize(i.normal), length(i.rel));
    let look = (i.info >> 10u) & 3u;
    if (((looks[i.info & 255u].info >> 24u) & 1u) == 1u && (look == 0u || look == 2u)) {
        // a piece of a lamp shows its own light
        o.direct = vec4f(col * 1.15, 1.0);
        o.albedo = vec4f(0.0, 0.0, 0.0, f32(body) / 3.0);
        return o;
    }
    // a glowing particle (ADR 0050) adds its own light, glow = info >> 24 over 32
    o.direct = vec4f(col * (l + f32(i.info >> 24u) / 32.0), 1.0);
    o.albedo = vec4f(col, f32(body) / 3.0);
    return o;
}

// whether this texel of a face is cracked, a share k of the way through its dig (ADR 0034): rays from near the
// face's middle, more and longer in ten steps, each a texel wide and wandering a little, picked by the cell
// and the face so no two faces crack alike
fn cracked(i: FxOut) -> bool {
    let dir = (i.info >> 12u) & 7u;
    let a = dir / 2u;
    let t = floor(vec2f(i.uvw[(a + 1u) % 3u], i.uvw[(a + 2u) % 3u]) * TEXELS);
    let p = (t + 0.5) / TEXELS - 0.5;
    let stage = floor(clamp(i.k, 0.0, 1.0) * 10.0) / 10.0;
    if (stage <= 0.0) { return false; }
    let seed = fx_hash(i.a ^ fx_hash(i.b ^ (dir * 0x9e3779b9u)));
    let rays = 2u + u32(stage * 5.0);
    let r = length(p);
    let tw = 1.0 / TEXELS;
    for (var n = 0u; n < rays; n++) {
        let h = fx_hash(seed + n * 7919u);
        let reach = stage * (0.35 + f32(h & 255u) / 255.0 * 0.4);
        if (r > reach || r < tw * 0.5) { continue; }
        // wanders by a texel or so every two texels out
        let wander = (f32(fx_hash(h ^ u32(r * 8.0)) & 255u) / 255.0 - 0.5) * 0.5;
        let ang = f32((h >> 8u) & 1023u) / 1024.0 * 6.2831853 + wander;
        let d = abs(dot(p, vec2f(-sin(ang), cos(ang))));
        if (d < tw * 0.55 && dot(p, vec2f(cos(ang), sin(ang))) > 0.0) { return true; }
    }
    return false;
}

// lit as the terrain is: its own sun ray (and the lamps' and the moon's) for every pixel; one visibility per
// particle was 20 % cheaper and lost the lamps, the moon and the soft edge (prototypes/effects/)
@fragment
fn fs_fx(i: FxOut) -> GOut {
    let look = (i.info >> 10u) & 3u;
    if (look == 3u && !cracked(i)) { discard; }
    let l = light(i.rel, normalize(i.normal), (i.info >> 8u) & 3u, vec2u(i.clip.xy));
    // a crack shows the block's own texel, dark, as a gap in it
    return fx_out(i, fx_colour(i) * select(1.0, 0.3, look == 3u), l.l);
}

// ---- models (ADR 0047, crates/client/src/models.rs) ----
// One draw per part of a model, one instance per piece: the part's quads (corner x | y << 6 | z << 12 | dir << 18
// in its voxels; w | h << 6; rgba8) placed by the piece's transform straight into the camera's frame. Lit by
// `light()` where they stand, as the terrain and the effects are; not in the traced scene, so they cast no
// shadow.
struct MPiece { x0: vec4f, x1: vec4f, x2: vec4f, q0: u32, body: u32, _a: u32, _b: u32 };
@group(0) @binding(12) var<storage, read> mquads: array<vec4u>;
@group(0) @binding(13) var<storage, read> mpieces: array<MPiece>;

@vertex
fn vs_model(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> FxOut {
    let m = mpieces[ii];
    let q = mquads[m.q0 + vi / 6u];
    let dir = (q.x >> 18u) & 7u;
    let a = dir / 2u;
    var ext = vec3f(1.0);
    ext[(a + 1u) % 3u] = f32(q.y & 63u);
    ext[(a + 2u) % 3u] = f32((q.y >> 6u) & 63u);
    let corner = vec3f(f32(q.x & 63u), f32((q.x >> 6u) & 63u), f32((q.x >> 12u) & 63u));
    let v = vec4f(corner + cube_corner(dir, vi % 6u) * ext, 1.0);
    let n = vec4f(axis(a) * select(1.0, -1.0, (dir & 1u) == 1u), 0.0);
    var o: FxOut;
    o.rel = vec3f(dot(m.x0, v), dot(m.x1, v), dot(m.x2, v));
    o.clip = g.view_proj * vec4f(o.rel, 1.0);
    o.normal = vec3f(dot(m.x0, n), dot(m.x1, n), dot(m.x2, n));
    o.info = m.body << 8u;
    o.b = q.z;
    return o;
}

@fragment
fn fs_model(i: FxOut) -> GOut {
    let l = light(i.rel, normalize(i.normal), (i.info >> 8u) & 3u, vec2u(i.clip.xy));
    return fx_out(i, unpack4x8unorm(i.b).rgb, l.l);
}
