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
    // the water (ADR 0071, gpu.rs `Globals::water`): x the ripples' clock in s, y the fog per metre with the
    // camera under water (0 above), z the light down there
    water: vec4f,
    // the clouds (ADR 0076, clouds.wgsl): x the drift's phase, y its period s, z the planet's sea radius
    // (0: none), w the map's air columns a face edge (0: none yet); y 0 with the clouds off (no shadow, the
    // snow still lies)
    cloud: vec4f,
    // the plants (ADR 0091, gpu.rs `Globals::plants`): xyz the wind at the camera in body 0's frame, m/s (0 in a
    // shot), w how far away they have shrunk into the ground, m (the Vegetation setting)
    plants: vec4f,
};
// base in columns and layers; info = face | level << 3 | body << 8; off: its first quad
// opq: its opaque land quads; the cutout ones follow them, the BLAS's second geometry (ADR 0069)
// clim: the climate at its footprint's corners (engine clutter::chunk_climate; ADR 0091)
struct Chunk { base: vec3i, info: u32, off: u32, opq: u32, clim: u32, sto: u32 };

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(14) var cl_tex: texture_2d_array<f32>;
@group(0) @binding(15) var cl_samp: sampler;
@group(0) @binding(1) var<storage, read> quads: array<vec2u>;
@group(0) @binding(2) var<storage, read> chunks: array<Chunk>;
// a block's look from data/blocks.toml (ADR 0013): info = pattern | side << 8 | metal (0..255) << 16 | gives light
// << 24 | cutout << 25 (ADR 0069); patterns 0 plain, 1 grass, 2 stone, 11 leaves (holes)
// (ADR 0039) and a pattern's second and third colours, its size in texels, amount and grain contrast; the
// painted faces (ADR 0045, gpu.rs `looks_of`: 16 bits a face, layer | variants - 1 << 12 | painted << 15; top
// and side in `tex`, the bottom in `tex2`) and a painted block's one colour, unorm 8-bit, painted << 24
struct Look { colour: vec3f, info: u32, colour2: vec3f, size: u32, colour3: vec3f, amount: f32, contrast: f32, tex: u32, mean: u32, tex2: u32, fluff: u32, through: f32, _b: u32, _c: u32 };
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
const LAMP_CELL: f32 = 8.0;
@group(0) @binding(9) var<storage, read> lamp_cells: array<u32>;

struct VOut {
    // invariant: the cutout blocks' depth and colour passes meet with Equal (ADR 0090)
    @builtin(position) @invariant clip: vec4f,
    @location(0) local: vec3f,
    @location(1) @interpolate(flat) dir: u32,
    @location(2) @interpolate(flat) mat: u32,
    @location(3) @interpolate(flat) ci: u32,
    @location(4) normal: vec3f,
    @location(5) rel: vec3f, // from the camera
    @location(6) ao: f32,    // open 1 .. shut in
    @location(7) plane: vec3f, // a fluff card's point from its face's middle in face units, z its overhang (ADR 0099)
    // a slope's triangle normal, w 1 (ADR 0101): flat, from the triangle's first corner (gpu.rs QUAD_TRIS), as the
    // indexed corners are shared by both triangles (ADR 0103)
    @location(8) @interpolate(flat) slope: vec4f,
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
    // a loose block's slope (bit 31, ADR 0101; mesh.rs): corner (du, dv) a cell lower where bit 20 + du + 2 dv
    // is set, split along the other diagonal when corners (0, 0) and (1, 1) differ, so a lone corner down is one
    // triangle's
    let sloped = (lo >> 31u) == 1u;
    if (sloped && ((q.y >> 20u) & 1u) != ((q.y >> 23u) & 1u)) {
        cu = array<f32, 6>(0., 1., 0., 1., 1., 0.);
    }
    var du = cu[k];
    var dv = cv[k];
    if ((dir & 1u) == 1u) { du = cv[k]; dv = cu[k]; }
    var c = p + axis((a + 1u) % 3u) * (du * (w + 2.0 * grow) - grow) + axis((a + 2u) % 3u) * (dv * (h + 2.0 * grow) - grow);
    if (sloped) { c.z -= f32((q.y >> (20u + u32(du) + 2u * u32(dv))) & 1u); }
    return c;
}

// the quad's own corner, which picks its lattice cell in `place`: a sloped top's half a cell down, so its
// lowered corners lie in the cell rather than past its edge (ADR 0101)
fn quad_at(q: vec2u) -> vec3f {
    let p = vec3f(f32(q.x & 63u), f32((q.x >> 6u) & 63u), f32((q.x >> 12u) & 63u));
    return p - vec3f(0.0, 0.0, select(0.0, 0.5, (q.x >> 31u) == 1u && ((q.x >> 28u) & 7u) == 4u));
}

// a sloped top's triangle `tri` (0, 1): its true normal from where its corners are drawn, in the body's frame
fn slope_normal(c: Chunk, b: Body, q: vec2u, tri: u32) -> vec3f {
    let at = quad_at(q);
    let p0 = place(c, b, quad_corner(q, tri * 3u, 0.0), at);
    let p1 = place(c, b, quad_corner(q, tri * 3u + 1u, 0.0), at);
    let p2 = place(c, b, quad_corner(q, tri * 3u + 2u, 0.0), at);
    return normalize(cross(p1 - p0, p2 - p0));
}

// a quad's look bits for the texel: a slope's corner bits are no crack edges
fn quad_mat(q: vec2u) -> u32 {
    return select(q.y, q.y & ~0x00f00000u, (q.x >> 31u) == 1u);
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
    return terrain_vertex(vi, ii, 0u);
}

// the cutout blocks' quads (ADR 0090): as `vs`, swaying in the wind
@vertex
fn vs_cut(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
    return terrain_vertex(vi, ii, 1u);
}

// the crowns' fluff (ADR 0090, 0099): each cutout face that opens on air (bit 25 of the quad, mesh.rs) again, a
// card a little larger than the face, lying close over it a hand out and barely tilted, whose leaves spill past
// the face's edges in lobes (`fluff_in`), so a crown's edges and corners are soft, never a sheet. Not traced.
@vertex
fn vs_fluff(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
    return terrain_vertex(vi, ii, 2u);
}

// how far the leaves sway, in cells of level 0, at a point (`p` in cells, the chunk's grid) of a quad drawn by
// `vs_cut` or `vs_fluff`: a smooth field of the place and the sky's clock, so faces that share an edge move as
// one and a fixed view stands still (`g.water.w`, the wind here, is 0 in a shot; frame.rs)
fn sway(p: vec3f, reach: f32) -> vec2f {
    if (g.water.w == 0.0) { return vec2f(0.0); }
    let amp = min(0.012 + 0.005 * g.water.w, 0.07) * reach;
    // whole turns an hour (the clock wraps at 3600 s): 1203, 2464, 974 and 2235 turns, about 2.1, 4.3, 1.7, 3.9 rad/s
    let t = g.water.x * (6.2831853 / 3600.0);
    let ph = dot(p, vec3f(0.61, 0.47, 0.83));
    return amp * vec2f(sin(t * 1203.0 + ph) + 0.4 * sin(t * 2464.0 + ph * 2.3), sin(t * 974.0 + ph * 1.3 + 1.0) + 0.4 * sin(t * 2235.0 - ph * 1.7));
}

// an indexed quad's corner (ADR 0103: four a quad, `quad_index` in gpu.rs) as quad_corner's k: 0, 1, 2 and 5, or
// for a slope split along the other diagonal 3, 4, 5 and 0, so each triangle's first corner is its own (gpu.rs
// CORNER_K mirrors this)
fn corner_k(vi: u32, swap: bool) -> u32 {
    let i = vi % 4u;
    if (swap) { return (i + 3u) % 6u; }
    return select(i, 5u, i == 3u);
}

fn terrain_vertex(vi: u32, ii: u32, mode: u32) -> VOut {
    let q = quads[vi / 4u];
    let dir = (q.x >> 28u) & 7u;
    let a = dir / 2u;
    let sloped = (q.x >> 31u) == 1u;
    let swap = sloped && ((q.y >> 20u) & 1u) != ((q.y >> 23u) & 1u);
    let k = corner_k(vi, swap);
    // grown by a millimetre: a merged quad's edge and its neighbours' corners on it are worked out apart, at
    // f32's grain at the body's radius (a quarter millimetre at 4 km), and met with pixel holes of sky between
    // them (bug 2026-11-11_17-38-04, ADR 0041); the overlap closes them, too thin to see as a lip
    var local = quad_corner(q, k, DRAW_GROW);
    // the corner in the face's u, v: as quad_corner lays it out
    var cu = array<u32, 6>(0u, 1u, 1u, 0u, 1u, 0u);
    var cv = array<u32, 6>(0u, 0u, 1u, 0u, 1u, 1u);
    if (swap) {
        cu = array<u32, 6>(0u, 1u, 0u, 1u, 1u, 0u);
    }
    var ku = cu[k];
    var kv = cv[k];
    if ((dir & 1u) == 1u) { ku = cv[k]; kv = cu[k]; }
    let occl = (q.y >> (8u + 2u * (ku + 2u * kv))) & 3u;
    let c = chunks[ii];
    var o: VOut;
    if (mode == 2u && ((q.y >> 25u) & 1u) == 0u) {
        // no fluff on a face against a block: all six corners at one point, nothing drawn
        o.clip = vec4f(0.0, 0.0, 0.0, 1.0);
        return o;
    }
    // where the painting is read: the quad's own unswayed corner, so the painting rides with the vertices
    var paint = quad_corner(q, k, 0.0);
    // a crown's own shade (ADR 0090): darker toward its wood and under it, and in a fluff clump's dents
    var shade = 1.0;
    // a fluff plane on a top face faces the sun square on: dimmed below the floor, else it reads lime (ADR 0090)
    var sun_face = 1.0;
    if (mode != 0u) {
        shade = 1.0 - 0.45 * select(0.0, 1.0 - f32((q.y >> 23u) & 3u) / 3.0, ((q.y >> 20u) & 7u) != 0u);
        if (dir == 5u) { shade *= 0.6; }
        var reach = 0.5;
        if (mode == 2u) {
            let at0 = quad_corner(q, 0u, 0.0);
            let cell0 = vec3i(floor(at0)) + c.base;
            // the block's fluff (`Fluff::packed`, blocks.rs): size, push, droop, long (unread since ADR 0099), dense
            let fw = looks[q.y & 255u].fluff;
            let size = f32(fw & 63u) * 0.05;
            let fpush = f32((fw >> 6u) & 63u) * 0.025;
            let droop = f32((fw >> 12u) & 63u) * 0.025;
            let dense = f32((fw >> 24u) & 63u) / 63.0;
            // clumps: a smooth field of the place, about 2.5 m across, spills some faces' leaves further and others
            // less, so a crown bulges in lobes with dents, never a ball or a box; `dense` keeps that share of cards
            let clump = smoothstep(0.3, 0.7, vnoise(vec3f(cell0) * 0.4 + f32(c.info & 7u) * 17.0));
            if (size == 0.0 || hash(cell0 + vec3i(29, 31, i32(dir) * 31 + 7)) >= dense) {
                o.clip = vec4f(0.0, 0.0, 0.0, 1.0);
                return o;
            }
            // a card over the face (ADR 0099): `size` sets how far its leaves spill past the face's edges (0.34 m at
            // the broadleaf's 1.7), the clump field more or less; a hand out (`push`), tilted only 4-12 degrees
            // about a line turned by the hash so neighbours differ and none is seen edge-on as a sheet
            let hf = hash(cell0 + vec3i(17, 3, i32(dir) * 31 + 11));
            let hg = hash(cell0 + vec3i(5, 23, i32(dir) * 31 + 2));
            let top = dir == 4u;
            shade *= mix(0.55, 1.0, clump);
            sun_face = select(1.0, 0.72, top);
            reach = 1.0;
            // over and under a plant the card stays closer and smaller: one stood off and spilling read from the side
            // as a lid on a shrub or a slab floating under a crown with the sky between; the side cards' lobes round
            // its top and underside
            let under = select(select(1.0, 0.5, top), 0.3, dir == 5u);
            let over = 0.2 * size * mix(0.6, 1.4, clump) * under;
            let su = f32(ku) * (1.0 + 2.0 * over) - over;
            let sv = f32(kv) * (1.0 + 2.0 * over) - over;
            let phi = hf * 6.2831853;
            let slope = tan(radians(4.0 + 8.0 * hg));
            // never behind the face: the tilt rises one side and the other stays a hand out
            let push = max((0.03 + fpush * mix(0.1, 0.4, clump)) + slope * ((su - 0.5) * cos(phi) + (sv - 0.5) * sin(phi)), 0.03) * under;
            // the painting at its own size across the card, shifted half a face so it does not repeat the face under
            // it; one shift for every card, so the lobes' field (`fluff_in`, read at the painting's point) is one of the
            // place and neighbouring cards agree at a shared edge; the mask's texels stay on the painting's grid
            let shift = vec2f(0.5, 0.25);
            let eu = axis((a + 1u) % 3u);
            let ev = axis((a + 2u) % 3u);
            paint = at0 + eu * (su + shift.x) + ev * (sv + shift.y);
            o.plane = vec3f(su - 0.5, sv - 0.5, over);
            // a side's card hangs a little by `droop` (up is the grid's z); a top's would sink into its face
            local = at0 + eu * su + ev * sv + axis(a) * select(push, -push, (dir & 1u) == 1u) - vec3f(0.0, 0.0, select(droop, 0.0, a == 2u) * (push + over));
        }
        local += vec3f(sway(vec3f(c.base) + local, reach), 0.0);
    }
    let b = g.bodies[(c.info >> 8u) & 255u];
    let gp = grid(c, local);
    let f = frame_at(c.info & 7u, b, gp.x, gp.y);
    let r = b.r0 * exp(gp.z * b.lnq);
    // the sides of a column are planes through the body's centre; its top is the sphere
    var normal = f.up;
    if (a == 0u) { normal = normalize(f.s - f.n * f.ta); }
    if (a == 1u) { normal = normalize(f.t - f.n * f.tb); }
    if ((dir & 1u) == 1u) { normal = -normal; }
    if (sloped && dir == 4u) {
        // a slope: its triangle's own normal (ADR 0101), as the traced geometry has it; its painting laid flat
        normal = slope_normal(c, b, q, k / 3u);
        o.slope = vec4f(brot(b, normal), 1.0);
        paint.z = f32((q.x >> 12u) & 63u);
    }

    let at = quad_at(q);
    let rel = brot(b, (place(c, b, local, at) - b.cam_hi) - b.cam_lo);
    o.clip = g.view_proj * vec4f(rel, 1.0);
    o.rel = rel;
    // the shade's floor: a crown's shaded side stays green (the target's), never near black
    o.ao = (1.0 - 0.2 * f32(occl)) * max(shade, 0.7) * sun_face;
    // the texture and the cell from the quad's own corner: in the grown lip, fract() would wrap to the far edge
    // (a dirt line along every grass top edge) and floor() name the neighbour
    o.local = paint;
    o.dir = dir;
    o.mat = quad_mat(q);
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
    let c = Chunk(bd.base, bd.info, bd.off, 0u, 0u, 0u);
    let b = g.bodies[(c.info >> 8u) & 255u];
    if (((q.x >> 28u) & 7u) == 7u) {
        // a still model's or plant's quad (ADR 0091): where `vs_still` draws it, unswayed, unfaded
        var cu = array<f32, 6>(0., 1., 1., 0., 1., 0.);
        var cv = array<f32, 6>(0., 0., 1., 0., 1., 1.);
        let cell = vec3f(f32(q.x & 63u), f32((q.x >> 6u) & 63u), f32((q.x >> 12u) & 63u));
        for (var k = 0u; k < 6u; k++) {
            let p = place(c, b, still_local(q, vec2f(cu[k], cv[k]), c, 1.0), cell);
            let o = (bd.dst + id.x * 6u + k) * 3u;
            verts[o] = p.x;
            verts[o + 1u] = p.y;
            verts[o + 2u] = p.z;
        }
        return;
    }
    for (var k = 0u; k < 6u; k++) {
        let at = quad_at(q);
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
// added to a chunk's index for its stills' instance (rt.rs STILL_INST)
const STILL_INST: u32 = 8388608u;
fn trace(o: vec3f, db: vec3f, body: u32, tmax: f32, flags: u32) -> RayIntersection {
    var rq: ray_query;
    ray_tint = vec3f(1.0);
    // the stills are instances of their own (rt.rs): only shadow and lamp rays, which they stop, ask for them
    let mask = select(1u << body, 17u << body, (flags & RAY_FLAG_TERMINATE_ON_FIRST_HIT) != 0u);
    rayQueryInitialize(&rq, acc, RayDesc(flags, mask, 0.0, tmax, o, db));
    // the season where the ray starts: a bare crown lets it through between its twigs (ADR 0090)
    let sn = season_in(g.bodies[body], o);
    while (rayQueryProceed(&rq)) {
        let c = rayQueryGetCandidateIntersection(&rq);
        let q = hit_quad(c.instance_custom_data, c.geometry_index, c.primitive_index);
        if (((q.x >> 28u) & 7u) == 7u) {
            // a still model or plant (ADR 0091) shades shadow and lamp rays only: a plant only while it stands
            // whole (`still_reach`), by its painting, and past `still_skip` from the ray's start, so a swaying
            // blade does not shade itself
            let b = g.bodies[body];
            // (nearer than 0.44 of the far distance every plant reaches, `plant_reach` times 0.8, a shrub's too
            // while still_far() <= 120 (frame.rs PLANTS_FAR): its hash is not needed)
            let e = length((o + db * c.t - b.cam_hi) - b.cam_lo);
            if ((flags & RAY_FLAG_TERMINATE_ON_FIRST_HIT) != 0u && c.t > still_skip
                && (e < 0.44 * still_far() || e < still_reach(c.instance_custom_data & (STILL_INST - 1u), q))
                && still_texel(q, c.primitive_index, c.barycentrics).a >= 0.5) {
                rayQueryConfirmIntersection(&rq);
            }
        } else if (is_glass(q.y)) {
            // far glass is drawn in its one colour, half clear: its shadow the same
            var t = vec4f(seen_colour(q.y & 255u, vec3f(0.0)), 0.45);
            if (((chunks[c.instance_custom_data].info >> 3u) & 31u) == 0u) { t = hit_texel(c.instance_custom_data, q, c.primitive_index, c.barycentrics, sn); }
            ray_tint *= mix(vec3f(1.0), t.rgb, t.a);
        } else if (!hit_empty(c.instance_custom_data, q, c.primitive_index, c.barycentrics, hit_season(c.instance_custom_data, q, sn))) {
            // a shadow ray through a face of fast leaves takes its share of the light (`fluff.light`, the block row)
            // and goes on: a soft green shade under a crown, not a whole block's (ADR 0090); nearly none left stops it.
            // shortcut: a fast crown is a shell, so any crown passes light squared whatever its thickness; weigh by
            // the distance from the entry face if thin and thick crowns should differ
            if (flags == RAY_FLAG_TERMINATE_ON_FIRST_HIT && ((chunks[c.instance_custom_data].info >> 18u) & 1u) == 1u) {
                ray_tint *= looks[q.y & 255u].through * vec3f(0.9, 1.0, 0.75);
                if (max(ray_tint.r, max(ray_tint.g, ray_tint.b)) < 0.02) { rayQueryConfirmIntersection(&rq); }
            } else {
                rayQueryConfirmIntersection(&rq);
            }
        }
    }
    return rayQueryGetCommittedIntersection(&rq);
}

// the quad a ray hit: instance id (a chunk-table entry, plus STILL_INST for its stills' instance), geometry
// geo (0 the opaque quads, 1 the cutout ones), triangle prim
fn hit_quad(id: u32, geo: u32, prim: u32) -> vec2u {
    let c = chunks[id & (STILL_INST - 1u)];
    if (id >= STILL_INST) { return quads[c.off + c.sto + prim / 2u]; }
    return quads[c.off + select(0u, c.opq, geo == 1u) + prim / 2u];
}

// whether a candidate hit at barycentrics bc is on an empty texel of its cutout block (level 0 only meshes them
// so), in season sn. Fast leaves (the chunk's info bit 18, frame.rs) are whole blocks to a ray (ADR 0069) unless
// bare: then the ray takes the twigs' painting, so light falls through a winter crown and its twigs are lit like
// the bark (ADR 0090); fancy ones take the season's texel
fn hit_empty(ci: u32, q: vec2u, prim: u32, bc: vec2f, sn: vec3f) -> bool {
    // flat colours draw a cutout block whole: the light takes it whole too
    if ((g.opt.x & 4u) != 0u) { return false; }
    if (((chunks[ci].info >> 18u) & 1u) == 1u && !is_bare(q.y & 255u, sn)) { return false; }
    return hit_texel(ci, q, prim, bc, sn).a < 0.5;
}

// season sn at quad q of chunk-table entry ci of level 0: with its column's stagger, as `terrain_texel` draws it
// (a cutout quad never merges, so its corner names its cell)
fn hit_season(ci: u32, q: vec2u, sn: vec3f) -> vec3f {
    let c = chunks[ci];
    let a = ((q.x >> 28u) & 7u) / 2u;
    let sgn = select(1.0, -1.0, ((q.x >> 28u) & 1u) == 1u);
    let at = quad_corner(q, 0u, 0.0);
    return vec3f(sn.xy, stagger(vec2i(floor(at.xy - axis(a).xy * sgn * 0.5)) + c.base.xy, c.info & 7u));
}

// the texel of quad q of chunk-table entry ci at a candidate hit's barycentrics bc on its triangle prim
fn hit_texel(ci: u32, q: vec2u, prim: u32, bc: vec2f, sn: vec3f) -> vec4f {
    let c = chunks[ci];
    let tri = (prim & 1u) * 3u;
    // the BLAS holds the quads grown by GROW; the same barycentrics on the ungrown corners map a hit in the lip
    // onto the quad's edge texel, as `vs` keeps `local` ungrown (a grown lip's fract wraps to the far edge)
    var local = quad_corner(q, tri, 0.0) * (1.0 - bc.x - bc.y) + quad_corner(q, tri + 1u, 0.0) * bc.x + quad_corner(q, tri + 2u, 0.0) * bc.y;
    let dir = (q.x >> 28u) & 7u;
    // a slope's painting lies flat, as `vs` lays it (ADR 0101)
    if ((q.x >> 31u) == 1u && dir == 4u) { local.z = f32((q.x >> 12u) & 63u); }
    let a = dir / 2u;
    let sgn = select(1.0, -1.0, (dir & 1u) == 1u);
    let cell = vec3i(floor(local - axis(a) * sgn * 0.5)) + c.base + vec3i(i32(c.info & 7u) * 977);
    let f = fract(local);
    return texel_a(quad_mat(q), dir, cell, vec2f(f[(a + 1u) % 3u], f[(a + 2u) % 3u]), f.z, sn);
}

// what the terrain leaves to p of the sun's disc, angular radius ar around direction sd (ADR 0108): rays to two
// opposite points of the disc, and two across them where those two disagree, so a penumbra is flat steps of a
// quarter with no noise in it (it was one ray a pixel to a dithered point, bug 2026-11-14_01-59-35); one ray to
// its middle with sharp shadows. `ray_tint` is left the mean tint of the rays that got out.
fn sun_seen(p: vec3f, n: vec3f, sd: vec3f, ar: f32, body: u32) -> f32 {
    if ((g.opt.x & 1u) != 0u) { return ray(p, n, sd, body, 1e5); }
    let t = normalize(cross(sd, select(vec3f(0., 1., 0.), vec3f(1., 0., 0.), abs(sd.y) > 0.9)));
    let bt = cross(sd, t);
    // (0.7 of the radius: about the middles of the disc's quarters by area)
    let r = ar * 0.7;
    var v = 0.0;
    var k = 0.0;
    var tint = vec3f(0.0);
    for (var i = 0u; i < 4u; i++) {
        if (i == 2u && (v == 0.0 || v == 2.0)) { break; }
        let s = ray(p, n, normalize(sd + select(t, bt, i >= 2u) * select(r, -r, (i & 1u) == 1u)), body, 1e5);
        v += s;
        k += 1.0;
        tint += ray_tint * s;
    }
    ray_tint = select(vec3f(1.0), tint / v, v > 0.0);
    return v / k;
}

// ---- the light's texels (ADR 0108) ----
// A block face's light is worked out once a texel of its painting: shadow, lamp and bounce rays leave from the
// texel's middle and take their random numbers from the texel, so every pixel of one texel has one light, a
// shadow's edge runs along texel borders and no pixel's grain is left (bug 2026-11-14_01-59-35). Free of the
// globals: gi.rs's test runs this part alone on the GPU.

// a cube face's normal and in-face axes (frame_at's), by bits as clouds.wgsl's `cl_face`: s 1 2 2 0 0 1, t 2 1 0 2 1 0
fn tx_axis(a: u32) -> vec3f { return vec3f(f32(a == 0u), f32(a == 1u), f32(a == 2u)); }
fn tx_n(f: u32) -> vec3f { return tx_axis(f >> 1u) * select(1.0, -1.0, (f & 1u) == 1u); }
fn tx_s(f: u32) -> vec3f { return tx_axis((1065u >> (2u * f)) & 3u); }
fn tx_t(f: u32) -> vec3f { return tx_axis((390u >> (2u * f)) & 3u); }

// a body's grid: an angle a column, half the columns a face edge, layer 0's radius, the log of a layer's ratio
struct TxGrid { theta: f32, half_n: f32, r0: f32, lnq: f32 };

// body-frame point q (from the body's centre) as its face and grid coordinates: column i, column j, layer
struct TxAt { face: u32, g: vec3f };
fn tx_grid_of(q: vec3f, gr: TxGrid) -> TxAt {
    let a = abs(q);
    var f = 4u + select(0u, 1u, q.z < 0.0);
    if (a.x >= a.y && a.x >= a.z) { f = select(0u, 1u, q.x < 0.0); } else if (a.y >= a.z) { f = 2u + select(0u, 1u, q.y < 0.0); }
    let dn = dot(q, tx_n(f));
    let ij = atan(vec2f(dot(q, tx_s(f)), dot(q, tx_t(f))) / dn) / gr.theta + gr.half_n;
    return TxAt(f, vec3f(ij, log(length(q) / gr.r0) / gr.lnq));
}

// grid point g of face f in the body's frame (the cube-sphere's own, as `frame_at` and `place` lay it)
fn tx_point(f: u32, g: vec3f, gr: TxGrid) -> vec3f {
    let ta = tan((g.xy - gr.half_n) * gr.theta);
    return normalize(tx_n(f) + tx_s(f) * ta.x + tx_t(f) * ta.y) * (gr.r0 * exp(g.z * gr.lnq));
}

// grid point g moved to the middle of its texel, k texels a grid unit, along the two axes other than `axis` (the
// face's normal: 0 column i, 1 column j, 2 the layer), which stays as it is
fn tx_snap(g: vec3f, axis: u32, k: f32) -> vec3f {
    return select((floor(g * k) + 0.5) / k, g, vec3<bool>(axis == 0u, axis == 1u, axis == 2u));
}

// the random numbers' seed of the texel whose middle is gs: its place, its face and which way the face looks
fn tx_seed(f: u32, gs: vec3f, axis: u32, k: f32) -> vec2u {
    var c = vec3i(floor(gs * k));
    // (the normal's axis: the face's plane, which both blocks either side share; the way it looks tells them apart)
    c[axis] = i32(round(gs[axis])) * 8 + i32(axis) + 3 * i32(k);
    var v = vec3u(c) * 1664525u + vec3u(1013904223u + f * 977u);
    v.x += v.y * v.z; v.y += v.z * v.x; v.z += v.x * v.y;
    v ^= v >> vec3u(16u);
    v.x += v.y * v.z; v.y += v.z * v.x;
    return v.xy;
}

// the texels a grid unit of the light at a place whose painting has `k0` a grid unit and where one screen pixel
// spans `px` grid units of the surface: the painting's while a texel spans a pixel or more; past that, halved
// until one does again, so far away the light is still one a texel of a coarser grid of whole texels and never
// finer than the pixels, which would only alias and shimmer as the camera moves
fn tx_k(k0: f32, px: f32) -> f32 {
    return k0 / exp2(clamp(ceil(log2(max(px * k0, 1e-6))), 0.0, 12.0));
}
// ---- end of the light's texels ----

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
const LAMP_R: f32 = 24.0;
const LAMP_RAYS: u32 = 3u;
// a shadow ray goes to the lamp's block, not its middle (`lamp_target`), and stops this far short of it, so
// the lamp never shadows itself
const LAMP_STOP: f32 = 0.01;
const NONE: u32 = 0xffffffffu;

// a lamp's light d2 square metres away: falling off with the square of the distance from 2 m, to nothing at
// LAMP_R. It was 1.5 / (1 + d2) to 8 m, a seventh at 3 m and nothing past 6; the developer asked for "much more
// light" (bug 2026-11-11_17-43-39, ADR 0041): now 7 times that at 3 m, 20 at 6, a glow to 10. Then twice the
// reach (ADR 0072): about as bright within 3 m (stretching the whole curve washed a lamp's neighbours out by
// day), twice as bright at 6 m, a tenth at 10 m and a glow to 20, so one lamp lights a 15 to 20 m room
fn lamp_light(lp: Lamp, d2: f32) -> vec3f {
    let x = 1.0 - d2 / (LAMP_R * LAMP_R);
    return lp.col.rgb * x * x * 2.0 / (1.0 + 0.1 * d2);
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
// largest axis), where the block is a box: o clamped into the lamp's cell and turned back. Moved by j (-1 to 1 an
// axis, in parts of the box) before the clamp (ADR 0072), so the rays end on points spread over the sides o sees:
// the lamp is a 1 m light, not a point, and the shadows it casts are soft (`lamp_seen`, ADR 0108).
fn lamp_target(lp: Lamp, o: vec3f, j: vec3f) -> vec3f {
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
    // the jitter only along the axes o lies within the box on, inset 5 cm: along the others the end stays 2 cm out
    // from the face o sees, and a ray from the floor beside a lamp never ends under that floor
    let inner = vec3f(0.45 * b.theta, 0.45 * b.theta, 0.45 * b.lnq);
    let c = select(clamp(q, at - half, at + half), clamp(q + j * inner, at - inner, at + inner), abs(q - at) < inner);
    let tb = normalize(f.n + f.s * tan(c.x) + f.t * tan(c.y)) * exp(c.z);
    return m * tb + b.ctr.xyz;
}

// what the terrain leaves to p of lamp lp, 0 to 1, as `sun_seen` the sun: rays to two opposite points of the
// sides p sees, two more across them where those disagree; flat steps of a quarter, no noise (ADR 0108; it was one
// ray a pixel to a random point). `ray_tint` is left the mean tint of the rays that got out.
fn lamp_seen(p: vec3f, n: vec3f, lp: Lamp, body: u32) -> f32 {
    let ro = ray_origin(p, n);
    var js = array<vec3f, 4>(vec3f(0.6), vec3f(-0.6), vec3f(0.6, -0.6, 0.0), vec3f(-0.6, 0.6, 0.0));
    var v = 0.0;
    var k = 0.0;
    var tint = vec3f(0.0);
    for (var i = 0u; i < 4u; i++) {
        if (i == 2u && (v == 0.0 || v == 2.0)) { break; }
        // aimed from where the ray starts: from p, the 2 cm offset carried its end into the lamp by its corners
        // (bug 2026-11-10_19-45-23)
        let w = lamp_target(lp, ro, js[i]) - ro;
        let d = length(w);
        var s = 1.0;
        ray_tint = vec3f(1.0);
        if (d >= LAMP_STOP) { s = ray(p, n, w / d, body, d - LAMP_STOP); }
        v += s;
        k += 1.0;
        tint += ray_tint * s;
    }
    ray_tint = select(vec3f(1.0), tint / v, v > 0.0);
    return v / k;
}

fn light(p: vec3f, n: vec3f, body: u32) -> Light {
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
        var seen = occ * sun_seen(p, n, sd, g.sun_pos.w / length(g.sun_pos.xyz - p), body);
        // the planet's clouds (ADR 0076), where the sun is not already hidden
        if (body == 0u && seen > 0.0 && g.cloud.y > 0.0) {
            let b0 = g.bodies[0];
            let m0 = mat3x3f(b0.rot0.xyz, b0.rot1.xyz, b0.rot2.xyz);
            seen *= cl_shadow((p - b0.ctr.xyz) * m0, sd * m0, g.cloud.z, g.cloud);
        }
        // (the glint behind a pane takes its share of the tint)
        o.sun = ecl * seen * dot(ray_tint, vec3f(1.0 / 3.0));
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
        let vis = lamp_seen(p, n, lamps[top[k]], body);
        o.l += topl[k] * vis * ray_tint;
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
    if ((q.x >> 31u) == 1u && dir == 4u) { hn = slope_normal(c, b, q, h.primitive_index & 1u); }
    let m = q.y & 255u;
    if (((looks[m].info >> 24u) & 1u) == 1u) { return select(seen_colour(m, vec3f(0.0)), vec3f(0.0), bounce) * tint; }
    let l = light(p + d * h.t, brot(b, hn), body);
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
    let vd = normalize(w.xyz / w.w);
    // the ray leaves from the middle of the texel this pixel shows, with the texel's random numbers (ADR 0108):
    // the G-buffer has no texel, so it is found from the point on the body's grid and the face from the normal
    // (the sides are planes of one column through the centre, the tops spheres of one layer)
    let bb = g.bodies[body];
    let gr = TxGrid(bb.theta, bb.half_n, bb.r0, bb.lnq);
    let p0 = vd * gb.w;
    let tg = tx_grid_of(to_body(bb, p0), gr);
    let nb = n * mat3x3f(bb.rot0.xyz, bb.rot1.xyz, bb.rot2.xyz);
    let ta = tan((tg.g.xy - gr.half_n) * gr.theta);
    let wi = abs(dot(nb, normalize(tx_s(tg.face) - tx_n(tg.face) * ta.x)));
    let wj = abs(dot(nb, normalize(tx_t(tg.face) - tx_n(tg.face) * ta.y)));
    let wl = abs(dot(nb, tx_point(tg.face, tg.g, gr) / length(tx_point(tg.face, tg.g, gr))));
    var axis = 2u;
    if (wi > wj && wi > wl) { axis = 0u; } else if (wj > wl) { axis = 1u; }
    // a pixel's footprint there, longer on a face seen edge on (a column is about a metre: grid units)
    let w1 = g.inv_vp * vec4f(uv.x * 2.0 - 1.0 + 2.0 / f32(size.x), 1.0 - uv.y * 2.0, 1.0, 1.0);
    let k = tx_k(TEXELS, gb.w * length(normalize(w1.xyz / w1.w) - vd) / max(abs(dot(n, vd)), 0.25));
    let gs = tx_snap(tg.g, axis, k);
    let p = p0 + brot(bb, tx_point(tg.face, gs, gr) - tx_point(tg.face, tg.g, gr));
    let f = g.n_lamps.y;
    let px = tx_seed(tg.face, gs, axis, k) + vec2u(f * 7919u, f * 6271u);
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
    return season_in(b, to_body(b, p));
}

// the same at a point of body b's own frame (`to_body`)
fn season_in(b: Body, bp: vec3f) -> vec3f {
    if (b.year < 0.0) { return vec3f(0.0); }
    // the Studio's year slider at a latitude (view.rs `season_year`): that phase, whole, everywhere
    if (b.year >= 8.0) { return vec3f(b.year - 8.0, 1.0, 0.0); }
    let z = normalize(bp).z;
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

// a joining block's painted frame, texels wide (`lamp` in tools/paint_textures.py)
const JOIN_FRAME: i32 = 2;
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
    var ti = vec2i(floor(t * n));
    // a joining block's sides that meet its own kind (bits 16-19 of the quad, u-, u+, v-, v+; mesh.rs, ADR 0085):
    // the frame there is read from inside, so a wall of lamps glows as one surface with the frame round it.
    // shortcut: an inner corner (both sides joined, the diagonal not) loses its frame texel; fine for flat walls
    let j = (mat >> 16u) & 15u;
    if (j != 0u) {
        let lo = select(vec2i(0), vec2i(JOIN_FRAME), vec2<bool>((j & 1u) != 0u, (j & 4u) != 0u));
        let hi = select(vec2i(15), vec2i(15 - JOIN_FRAME), vec2<bool>((j & 2u) != 0u, (j & 8u) != 0u));
        ti = clamp(ti, lo, hi);
    }
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
    let root = (mat >> 20u) & 7u;
    let cutout = ((looks[m0].info >> 25u) & 1u) == 1u;
    let turn = select(0u, root, cutout);
    let xy = face_xy(ti, t, dir, turn, 15);
    let x = xy.x;
    let y = xy.y;
    // the same at 32 texels a metre, for a fine painting (ADR 0090)
    let fxy = face_xy(clamp(vec2i(floor(t * 32.0)), vec2i(0), vec2i(31)), t, dir, turn, 31);
    var c: vec4f;
    if (painted != 0u) {
        c = paint_texel(painted, cell, xy, fxy, -1);
        if (c.a < 0.5 && !is_glass(m)) { c = vec4f(row_seen(m) * 0.6, 0.0); }
    } else {
        let lk = look_in(m, sn);
        if (leaf_hole(lk, dir, cell, ti)) {
            c = vec4f(lk.colour * 0.6, 0.0);
        } else {
            c = vec4f(pattern_texel(lk, dir, cell, x, y, h, tone), select(1.0, glass_pane(x, y), is_glass(m)));
        }
    }
    // a crack along an edge of the face (bits 20-23 of a rock's quad, u-, u+, v-, v+; a cutout block's are its twigs'; mesh.rs, ADR 0094): a dark line a
    // texel wide on the edge, so the two faces beside a crack draw it as one line two texels wide
    let ck = (mat >> 20u) & 15u;
    if (ck != 0u && !cutout) {
        let e = i32(n) - 1;
        if (((ck & 1u) != 0u && ti.x == 0) || ((ck & 2u) != 0u && ti.x == e) || ((ck & 4u) != 0u && ti.y == 0) || ((ck & 8u) != 0u && ti.y == e)) {
            c = vec4f(c.rgb * 0.3, c.a);
        }
    }
    // a bedding plane across a side face (bits 24-31: its texel row at the face's low end + 8, the rows it climbs to
    // the high end + 4 in the top three; `crack::bed`): neighbouring faces share their edges' rows, so the line runs
    // on along the wall and tilts with the fold
    let bd = mat >> 24u;
    if (bd != 0u && dir < 4u && !cutout) {
        let along = (f32(select(ti.x, ti.y, dir / 2u == 1u)) + 0.5) / n;
        let row = floor(f32(i32(bd & 31u) - 8) + f32(i32(bd >> 5u) - 4) * along + 0.5);
        if (floor(hgt * n) == row) {
            c = vec4f(c.rgb * 0.3, c.a);
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
    // a bare crown's winter painting picks its variant by the block's place in the crown (ADR 0090, part D's
    // layout): beside the wood 0 or 1, a step on 2 (a limb through to the next block), two on 3 (tips)
    let steps = (mat >> 23u) & 3u;
    let role = cutout && root != 0u;
    if (px != 0u) { cx = paint_texel(px, cell, xy, fxy, select(-1, i32(steps), role && r.x == 768u)); }
    if (py != 0u) { cy = paint_texel(py, cell, xy, fxy, select(-1, i32(steps), role && r.y == 768u)); }
    let mixed = mix(c, mix(cx, cy, r.f), sn.y);
    var a = select(select(0.0, 1.0, mixed.a >= 0.5), mixed.a, is_glass(m0));
    // a bare crown is its winter painting alone: bark-coloured twigs, no leaf tint mixed in (a crown turns bare
    // at once where winter outweighs the rest); a block with no way to wood within three steps draws no twigs
    if (cutout && bare_of(r, sn) > 0.5) {
        let w = select(cy, cx, r.x == 768u);
        return vec4f(w.rgb, select(0.0, 1.0, w.a >= 0.5 && root != 0u));
    }
    // the empty texels of the season nearest in the mix
    return vec4f(mixed.rgb, a);
}

// how much winter's bare look weighs in a season mix (`season_rows`, row 768 the winter look)
fn bare_of(r: SeasonRows, sn: vec3f) -> f32 {
    return sn.y * (select(0.0, 1.0 - r.f, r.x == 768u) + select(0.0, r.f, r.y == 768u));
}

// whether block m0 at season sn is bare (a cutout block a season paints, winter outweighing the rest)
fn is_bare(m0: u32, sn: vec3f) -> bool {
    let size = looks[m0].size;
    if ((size & 512u) == 0u || sn.y == 0.0 || ((looks[m0].info >> 25u) & 1u) == 0u) { return false; }
    return bare_of(season_rows(size >> 8u, sn), sn) > 0.5;
}

// a face's texel in its painting (`last` 15, or 31 for a fine one): on a side, x runs left to right as seen
// from outside and y down from its top edge (t is (y, z) on an x face, (z, x) on a y face; along +y on +x and
// along +x on -y read rightwards, the other two are turned). A cutout block's twigs grow from its wood (ADR 0090,
// mesh.rs `twig_bits`): its paintings have them rooted on the bottom edge, turned here so that edge lies on the
// side of the face the wood is on (`root` 1..4: u-, u+, v-, v+), or folded about the middle when the wood is
// behind the face (5); 0 turns nothing
fn face_xy(ti: vec2i, t: vec2f, dir: u32, root: u32, last: i32) -> vec2i {
    if (root == 5u) {
        return vec2i(ti.x, last - i32(floor(abs(t.y - 0.5) * f32(2 * last + 2))));
    }
    if (root != 0u) {
        let d = select(ti.y, ti.x, root < 3u);
        return vec2i(select(ti.x, ti.y, root < 3u), select(d, last - d, (root & 1u) == 1u));
    }
    var x = ti.x;
    var y = ti.y;
    if (dir < 4u) {
        let on_y = dir / 2u == 1u;
        x = select(ti.x, ti.y, on_y);
        y = last - select(ti.y, ti.x, on_y);
        if (dir == 1u || dir == 2u) { x = last - x; }
    }
    return vec2i(x, y);
}

// texel (x, y) of a painted face (`face_paint`): one of its variants by the cell's hash, the same on every face;
// a 0 for an empty pixel (a cutout block's, ADR 0069). A fine painting (bit 14, 32 x 32 a variant as four 16 x 16
// quarters in turn, ADR 0090) reads `fxy` instead. `steps` >= 0: a bare crown's winter painting of four variants
// takes its variant by the block's place in the crown (ADR 0090, part D's layout): beside the wood 0 or 1 by the
// hash, a step on 2 (a limb through), two on 3 (tips)
fn paint_texel(painted: u32, cell: vec3i, xy: vec2i, fxy: vec2i, steps: i32) -> vec4f {
    let nv = ((painted >> 12u) & 3u) + 1u;
    let hv = hash(cell + vec3i(0, 0, 4099));
    var v = min(u32(hv * f32(nv)), nv - 1u);
    if (steps > 0 && nv == 4u) { v = u32(steps) + 1u; } else if (steps == 0 && nv == 4u) { v = min(u32(hv * 2.0), 1u); }
    if (((painted >> 14u) & 1u) == 1u) {
        let f = vec2u(clamp(fxy, vec2i(0), vec2i(31)));
        let q = (f.y >> 4u) * 2u + (f.x >> 4u);
        return unpack4x8unorm(tex[((painted & 4095u) + v * 4u + q) * 256u + (f.y & 15u) * 16u + (f.x & 15u)]);
    }
    let p = vec2u(clamp(xy, vec2i(0), vec2i(15)));
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
    return terrain_lit(i, ground_tint(i, terrain_texel(i).rgb));
}

// a grass block's top takes the climate's tint (ADR 0091), as the plants on it do: its green texels by the
// chunk's climate at the place, no season (its paintings have theirs)
fn ground_tint(i: VOut, c: vec3f) -> vec3f {
    if (i.dir > 4u || (looks[i.mat & 255u].info & 255u) != GRASS) { return c; }
    let ch = chunks[i.ci];
    let cl = chunk_climate(ch, i.local.xy / 32.0);
    let tint = climate_tint(cl.x, cl.y, 2u, 0.0);
    // where the plants have thinned out (from half their far distance) the ground takes the meadow ground's
    // painting (the textures session's three tiles, one by its column's hash): tall-grass greens, a few
    // flowers, so from a hill there is no line where plants end
    let gp = grid(ch, i.local);
    let d = length(i.rel);
    let far = smoothstep(still_far() * 0.5, still_far(), d);
    // past 150 m a pixel spans many texels: the painting fades to its mean colour, no crawling noise
    let blur = smoothstep(150.0, 300.0, d);
    if (i.dir == 4u) {
        if (!leafy(c)) { return c; }
        if (far <= 0.0) { return c * tint; }
        let m = mix(meadow_texel(gp.xy), meadow_mean(), blur);
        return mix(c * tint, m * select(vec3f(1.0), tint, leafy(m)), far);
    }
    // a side: the meadow hangs over the step's top third as a fringe, reaching further down with distance until
    // a far step (and a coarse chunk's, all of whose steps are far) is a grassy ledge, not a band of dirt, so
    // terraces seen from above read as meadow
    let low = mix(0.55, 0.0, smoothstep(still_far(), 3.0 * still_far(), d));
    let t = smoothstep(low, low + 0.3, fract(i.local.z)) * far;
    if (t <= 0.0) { return c; }
    let m = mix(meadow_texel(vec2f(gp.x + gp.y, gp.z)), meadow_mean(), blur);
    return mix(c, m * select(vec3f(1.0), tint, leafy(m)), t);
}

fn meadow_mean() -> vec3f {
    return unpack4x8unorm(sfaces[0].mean).rgb;
}

// const: face 0's flag when it is the meadow ground (engine clutter::MEADOW)
const MEADOW: u32 = 4u;
// the meadow ground's texel at grid point p (metres across, 32 px a metre), the tile picked per column
fn meadow_texel(p: vec2f) -> vec3f {
    let f = sfaces[0];
    let w = f.wh & 65535u;
    let h = f.wh >> 16u;
    let tiles = max(w / 32u, 1u);
    let col = vec2i(floor(p)) + vec2i(1 << 20);
    let tile = min(u32(hash2(vec2u(col)) * f32(tiles)), tiles - 1u);
    let px = min(vec2u(fract(p) * 32.0), vec2u(min(w, 32u) - 1u, h - 1u)) + vec2u(tile * 32u, 0u);
    // its painting's top row is the far side: y down
    let t = unpack4x8unorm(spaint[f.base + (h - 1u - min(px.y, h - 1u)) * f.pw + px.x]);
    return select(vec3f(0.3, 0.58, 0.2), t.rgb, (f.flags & MEADOW) != 0u);
}

@fragment
fn fs_cut(i: VOut) -> GOut {
    let c = terrain_texel(i);
    if (c.a < 0.5) {
        discard;
    }
    return terrain_lit(i, c.rgb);
}

// the cutout blocks' depth pass (ADR 0090): the same holes as `fs_cut`, nothing written but the depth
@fragment
fn fs_cut_depth(i: VOut) -> GOut {
    if (terrain_texel(i).a < 0.5) {
        discard;
    }
    // the targets are masked off: what is returned is never written
    return GOut(vec4f(0.0), vec4f(0.0), vec2u(0u));
}

// their G-buffer after the opaque terrain, where the depth pass left theirs (`Equal`): no discard, early depth
@fragment
fn fs_cut_lit(i: VOut) -> GOut {
    return terrain_lit(i, terrain_texel(i).rgb);
}

// the fluff (`vs_fluff`, ADR 0099): the card keeps the face (its corners rounded) and, past its edges, lobes of
// leaves up to its overhang (`i.plane.z`) from a smooth field of the place about 0.3 m across, in whole texels with
// a ragged rim, so a crown's outline is bumpy leaves, not a square
fn fluff_in(i: VOut) -> bool {
    let tx = (floor(i.plane.xy * 32.0) + 0.5) / 32.0;
    let c = chunks[i.ci];
    // in the chunk's own cells (a coarse level's are larger), as `terrain_texel`, so the field has no seam at its edge
    let cell = vec3i(floor(i.local * 32.0)) + (c.base >> vec3u((c.info >> 3u) & 31u)) * 32;
    let h = hash(cell + vec3i(3, 9, 27 + i32(i.dir)));
    let out = length(max(abs(tx) - 0.38, vec2f(0.0))) - 0.12;
    let lobe = vnoise(vec3f(cell) / 32.0 * 3.3 + vec3f(f32(i.dir & 6u) * 5.1));
    return out < i.plane.z * smoothstep(0.25, 0.75, lobe) * 1.15 + 0.035 * (h - 0.5);
}

@fragment
fn fs_fluff_depth(i: VOut) -> GOut {
    if (!fluff_in(i) || terrain_texel(i).a < 0.5) {
        discard;
    }
    return GOut(vec4f(0.0), vec4f(0.0), vec2u(0u));
}

@fragment
fn fs_fluff_lit(i: VOut) -> GOut {
    return terrain_lit(i, terrain_texel(i).rgb);
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

fn terrain_lit(i: VOut, col_in: vec3f) -> GOut {
    let c = chunks[i.ci];
    let body = (c.info >> 8u) & 255u;
    let look = looks[i.mat & 255u];
    let n = normalize(select(i.normal, i.slope.xyz, i.slope.w > 0.5));
    // where its light is worked out (ADR 0108): the middle of the painting's texel this pixel shows, found on the
    // grid from `local` (the painting's own point: a swaying crown's light rides with its texels) and moved there
    // from the pixel's point along the face (`sun_seen` and `lamp_seen` take no random numbers). A coarse level's texels are as large
    // as it draws them; far away, where a texel is smaller than a pixel, the light's are larger (`tx_k`).
    // (first: the derivatives need every pixel of the quad here)
    let fpx = max(length(dpdx(i.rel)), length(dpdy(i.rel)));
    let b = g.bodies[body];
    let gr = TxGrid(b.theta, b.half_n, b.r0, b.lnq);
    let gp = grid(c, i.local);
    let k = tx_k(TEXELS / f32(1u << ((c.info >> 3u) & 31u)), fpx);
    let gs = tx_snap(gp, i.dir / 2u, k);
    let lit_at = i.rel + brot(b, tx_point(c.info & 7u, gs, gr) - tx_point(c.info & 7u, gp, gr));
    var col = col_in;
    // the planet's lying snow (ADR 0076, the air grid's per column): white over what faces up, the face's own
    // pattern showing through where it lies thin
    if (body == 0u && g.cloud.w > 0.0) {
        let b0 = g.bodies[0];
        let up = normalize(i.rel - b0.ctr.xyz);
        let flat = smoothstep(0.55, 0.85, dot(n, up));
        var lie = 0.0;
        if (flat > 0.0) { lie = cl_snow(up * mat3x3f(b0.rot0.xyz, b0.rot1.xyz, b0.rot2.xyz), g.cloud) * flat; }
        if (lie > 0.0) {
            let lum = dot(col, vec3f(0.3, 0.5, 0.2));
            col = mix(col, vec3f(0.9, 0.92, 0.96), clamp(lie * 1.6 - (0.5 - lum) * 0.6, 0.0, 1.0));
        }
    }
    var o: GOut;
    o.gbuf = gb_pack(n, length(i.rel));
    if (((look.info >> 24u) & 1u) == 1u) {
        // it gives light: it shows its own, and takes none from around
        o.direct = vec4f(col * 1.15, 1.0);
        o.albedo = vec4f(0.0, 0.0, 0.0, f32(body) / 3.0);
        return o;
    }
    let px = vec2u(i.clip.xy);
    // a plant lit as one mass (ADR 0090): the direct light takes its normal bent half way toward up, so the sun
    // side is not lime and the far side not black (its volume is its shade, `ao`); the G-buffer keeps the true
    // normal for the bounce and the denoiser
    var ln = n;
    if (((look.info >> 25u) & 1u) == 1u) { ln = normalize(n + normalize(i.rel - g.bodies[body].ctr.xyz) * 0.5); }
    let l = light(lit_at, ln, body);
    var out = col * l.l;
    // its own shade darkens its sunlit side too, so a plane facing the sun keeps the painting's greens
    if (((look.info >> 25u) & 1u) == 1u) { out *= i.ao; }
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
    let l = light(i.rel, n, body);
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
    // where on its face (ADR 0071): the grid's columns i, j and the layer, for the ripples and the foam
    @location(5) grid: vec3f,
    // a river's flow in columns a second along i and j
    @location(6) gflow: vec2f,
    @location(7) @interpolate(flat) face: u32,
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
    o.gflow = vec2f(0.0);
    o.grid = vec3f(0.0);
    o.face = 0u;
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
        o.grid = vec3f(gp, layer);
        o.face = c.info & 7u;
        let g2 = length(fall);
        if (g2 > 0.0) {
            // downhill, as fast as it is steep: 0.5 m/s on the flat to 3 m/s down rapids
            let d2 = -fall / g2;
            let speed = clamp(0.5 + 30.0 * g2, 0.5, 3.0);
            o.flow = vec4f(brot(b, normalize(-tilt)) * speed, dot(gp, d2));
            o.gflow = d2 * speed;
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
    var o = wout(bi, f.up, layer, f.up, 0u);
    o.grid = vec3f(gp, layer);
    o.face = c.info & 7u;
    return o;
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
    var o = wout(bi, f.up, z, normal, u32(kind != 0u));
    o.grid = vec3f(f32(q.i) + du, f32(q.j) + dv, z);
    o.face = face;
    return o;
}

// ---- the water's look (ADR 0071) ----
// Clear and traced, on the blocks' pixel grid. The bed shows through, fading to blue by the length of the
// way through the water (one ray down the view); the sky, Sol, Luna and the near terrain are in it with a
// Fresnel term (one ray along the reflection); ripples of three octaves, flat over each 1/16 m texel, run with
// the sky's clock and along a river's flow and calm with distance, so from orbit the sea is one blue with
// the sun's glint; foam where the water thins to the shore and down falls. The
// settings' Water row: off draws the old sheet (`water_old`), simple shades without rays of its own.

// the colour of the water's own light at noon (post.wgsl has the same, for the fog under water)
const WATER_FOG: vec3f = vec3f(0.03, 0.16, 0.24);
// what the water takes of the light a metre: red first, blue last
const WATER_ABSORB: vec3f = vec3f(0.7, 0.25, 0.16);
// the ripples' texels a metre: the blocks' own
const RIPPLE_PX: f32 = 16.0;
// how often a second the ripples, foam and falls step on: they jump like pixel art, not glide (the developer)
const WATER_STEPS: f32 = 4.0;
// how far the way down the view is traced (a ray's cost grows with its length): deeper counts as deep
const WATER_SEE: f32 = 16.0;
// how far off the terrain is traced in it: farther, the far chunks are coarse cells that stand a layer over
// the sea at the coast, and a grazing reflection met them as a dark band along the horizon; the sky is enough there
const WATER_MIRROR: f32 = 250.0;
// and how far a reflected ray goes looking for it: past it the sky (a ray's cost grows with its length)
const WATER_REACH: f32 = 150.0;

fn wnoise(p: vec2f) -> f32 {
    let c = floor(p);
    let f = p - c;
    let u = f * f * (3.0 - 2.0 * f);
    let k = bitcast<vec2u>(vec2i(c));
    let a = mix(hash2(k), hash2(k + vec2u(1u, 0u)), u.x);
    let b = mix(hash2(k + vec2u(0u, 1u)), hash2(k + vec2u(1u, 1u)), u.x);
    return mix(a, b, u.y);
}

// the ripples' height at p (columns of the face), at time t: three octaves drifting apart
fn ripple(p: vec2f, t: f32) -> f32 {
    return wnoise(p * 0.45 + vec2f(0.21, 0.13) * t)
        + 0.5 * wnoise(p * vec2f(1.1, 0.9) + vec2f(-0.17, 0.29) * t + 17.0)
        + 0.25 * wnoise(p * 2.3 + vec2f(0.37, -0.31) * t + 41.0);
}

// what a reflected ray from p sees, lit without rays of its own (the water's pass is the dearest near water,
// and a shadow in a ripple's reflection is not missed): the terrain it hits in the sun as the water has it
// (`day`) and the light from around, else the sky. `seen` traces the hit's light (a metal's mirror).
fn seen_lit(p: vec3f, d: vec3f, body: u32, day: f32, sun: f32, tmax: f32) -> vec3f {
    let bb = g.bodies[body];
    let h = trace(to_body(bb, p), d * mat3x3f(bb.rot0.xyz, bb.rot1.xyz, bb.rot2.xyz), body, tmax, 0u);
    if (h.kind == RAY_QUERY_INTERSECTION_NONE) { return sky_colour(d); }
    let c = chunks[h.instance_custom_data];
    let q = hit_quad(h.instance_custom_data, h.geometry_index, h.primitive_index);
    let m = q.y & 255u;
    if (((looks[m].info >> 24u) & 1u) == 1u) { return seen_colour(m, vec3f(0.0)); }
    let b = g.bodies[(c.info >> 8u) & 255u];
    let hp = p + d * h.t;
    let up = normalize(hp - bb.ctr.xyz);
    let sd = normalize(g.sun_pos.xyz - hp);
    // a face's normal from its axis, as `seen` has it; a top's is the up, a bottom's down
    let dir = (q.x >> 28u) & 7u;
    var hn = up;
    if (dir == 5u) { hn = -up; }
    if (dir / 2u != 2u) { hn = normalize(-d - up * dot(-d, up)); }
    // no shadow ray there: the water's own sun stands in, so a pool under a roof mirrors it unlit
    return seen_colour(m, season(b, hp)) * (sun_on(body) * max(dot(hn, sd), 0.0) * sun * day + ambient(day));
}

// the moon's body where a reflected ray from p along d meets its sphere: lit by its phase
fn luna_in(p: vec3f, d: vec3f, body: u32) -> vec3f {
    let mi = u32(g.moon_col.w);
    let m = g.bodies[mi].ctr;
    if (mi == body || m.w <= 0.0) { return vec3f(0.0); }
    let oc = m.xyz - p;
    let tca = dot(oc, d);
    let d2 = dot(oc, oc) - tca * tca;
    if (tca < 0.0 || d2 > m.w * m.w) { return vec3f(0.0); }
    let hit = p + d * (tca - sqrt(m.w * m.w - d2));
    let lit = max(dot(normalize(hit - m.xyz), normalize(g.sun_pos.xyz - hit)), 0.0);
    return vec3f(0.65) * 0.5 * lit * moon_gain(mi);
}

@fragment
fn fs_water(i: WOut) -> @location(0) vec4f {
    // a pixel's footprint on the water in metres, before any branch (derivatives need uniform control flow)
    let foot = max(length(fwidth(i.grid.xy)), 1e-4);
    let mode = (g.opt.x >> 4u) & 3u;
    if (mode >= 2u) { return water_old(i); }
    let b = g.bodies[i.body];
    let t = floor(g.water.x * WATER_STEPS) / WATER_STEPS;
    let px = vec2u(i.clip.xy);
    let v = normalize(-i.rel);
    let dist = length(i.rel);
    var n0 = normalize(i.normal);
    let fall = (i.side & 1u) == 1u;
    // seen from behind: from under the water, or (the camera out of it) the sea past the horizon seen through
    // the nearer sea (water writes no depth), not drawn, else a dark band along the horizon; a fall or a front
    // seen from its dry side is drawn as its face. Before the rays, so what is dropped costs nothing.
    let behind = dot(n0, v) < 0.0;
    if (behind) { n0 = -n0; }
    let below = behind && g.water.y > 0.0;
    if (behind && !fall && !below) { discard; }
    let speed = length(i.gflow);
    // the ripples: on the blocks' grid, calm where a pixel spans more than a few of its texels
    var fsv = array<vec3f, 6>(vec3f(0., 1., 0.), vec3f(0., 0., 1.), vec3f(0., 0., 1.), vec3f(1., 0., 0.), vec3f(1., 0., 0.), vec3f(0., 1., 0.));
    var ftv = array<vec3f, 6>(vec3f(0., 0., 1.), vec3f(0., 1., 0.), vec3f(1., 0., 0.), vec3f(0., 0., 1.), vec3f(0., 1., 0.), vec3f(1., 0., 0.));
    let calm = 1.0 - smoothstep(0.04, 0.4, foot);
    let q = 1.0 / RIPPLE_PX;
    let p = i.grid.xy - i.gflow * t;
    let cell = (floor(p * RIPPLE_PX) + 0.5) * q;
    var n = n0;
    if (!fall && calm > 0.0) {
        let h0 = ripple(cell, t);
        let gr = vec2f(ripple(cell + vec2f(q, 0.0), t) - h0, ripple(cell + vec2f(0.0, q), t) - h0) * RIPPLE_PX;
        let k = (0.07 + 0.05 * min(speed, 3.0)) * calm;
        n = normalize(n0 - (brot(b, fsv[i.face]) * gr.x + brot(b, ftv[i.face]) * gr.y) * k);
        if (dot(n, v) < 0.02) { n = normalize(n + n0 * (0.02 - dot(n, v))); }
    }
    let l = light(i.rel, n, i.body);
    let cosv = max(dot(n, v), 0.0);
    let fres = 0.02 + 0.98 * pow(1.0 - cosv, 5.0);
    // the water's own light, scattered back from inside it
    let inside = WATER_FOG * (l.l * 0.8 + ambient(l.day) * 2.0);
    // texel noise for the foam, steady for a moment, then the next
    let grain = hash2(bitcast<vec2u>(vec2i(floor(p * RIPPLE_PX))) + vec2u(u32(t * 3.0) * 7u, 0u));
    if (below) {
        // from under it: the sky through a window overhead, the deep mirrored around it
        let window = smoothstep(0.6, 0.7, cosv);
        var col = mix(WATER_FOG * g.water.z, sky_colour(-v) * 0.8, window);
        col = mix(col, WATER_FOG * g.water.z, 1.0 - exp(-dist * max(g.water.y, 0.02)));
        return water_out(col, mix(1.0, 0.4, window));
    }
    // the reflection: the terrain traced, else the sky; kept above the flat surface
    var r = reflect(-v, n);
    let rn = dot(r, n0);
    if (rn < 0.01) { r = normalize(r + n0 * (0.01 - rn)); }
    var refl = sky_colour(r);
    if (mode == 0u && (g.opt.x & 2u) == 0u && dist < WATER_MIRROR && fres > 0.08) {
        // only where it shows: looking down the water mirrors 2 to 8 %, and there the sky's colour does (the
        // ray was half the pass's cost on a lake); faded in from there and into the sky's over the last fifth
        // of its reach, so no line shows where the tracing starts or ends
        let traced = seen_lit(ray_origin(i.rel, n0), r, i.body, l.day, l.sun, WATER_REACH);
        refl = mix(traced, refl, max(smoothstep(0.8 * WATER_MIRROR, WATER_MIRROR, dist), 1.0 - smoothstep(0.08, 0.16, fres)));
    }
    refl += luna_in(i.rel, r, i.body);
    // the way through the water to what is under it: traced, or guessed as 3 m deep
    let cos0 = max(dot(n0, v), 0.05);
    var len = 3.0 / cos0;
    var foam = 0.0;
    if (fall) {
        // a fall or a front: churned white, streaking down
        let fp = vec2f(i.grid.x + i.grid.y, i.grid.z + t * 2.5);
        let fc = floor(fp * RIPPLE_PX) * q;
        foam = 0.35 + 0.5 * step(0.45, wnoise(fc * vec2f(5.0, 1.2)));
        len = 1.0;
    } else if (mode == 0u) {
        let m = mat3x3f(b.rot0.xyz, b.rot1.xyz, b.rot2.xyz);
        let h = trace(to_body(b, i.rel - v * 0.01), -v * m, i.body, WATER_SEE, 0u);
        len = select(WATER_SEE, h.t + 0.01, h.kind != RAY_QUERY_INTERSECTION_NONE);
        // far off the coarse terrain meets the ray at the surface (shelves from 400 m showed as dry grey rock):
        // there, where the ripples have calmed too, at least the simple mode's guessed 3 m
        len = mix(len, max(len, 3.0 / cos0), smoothstep(150.0, 300.0, dist));
        // the shore: surf where the water is thinnest over what the view meets (its depth there is the way
        // through times the cosine), in a band that laps in and out, broken into texels: dense where it is
        // thinnest, a few flecks over half a metre. (Four short rays across the surface to the banks were
        // tried first: a beach shelves in whole steps, and within a metre they met no bank.)
        let lap = 0.2 + 0.15 * sin(t * 1.3 + (i.grid.x + i.grid.y) * 0.7);
        // and where the way itself is short: the view meets the bank's wall just under the surface
        let thin = min(len * cos0, 0.5 * len);
        foam = select(0.0, 1.0, abs(thin - lap) < 0.03 + 0.06 * grain && grain > 0.25) * (1.0 - smoothstep(0.1, 0.3, foot));
    }
    // rapids churn white, in texels
    foam = max(foam, select(0.0, 1.0, grain < 0.6 * smoothstep(1.5, 3.0, speed)));
    let tr = exp(-WATER_ABSORB * max(len, 1.0)); // at least a metre's tint: thin sheets over a shelf read as sea
    let tm = dot(tr, vec3f(0.25, 0.45, 0.3));
    // blended over the terrain already drawn (the bed): what is left of it shows; the rest is the water
    var a = 1.0 - (1.0 - fres) * tm;
    var col = refl * fres + inside * (1.0 - tr) * (1.0 - fres);
    // the sun's glint where it reaches: sharp on the ripples near, a wide sheen far off where they are too
    // small to see
    let e = mix(60.0, 900.0, calm);
    col += g.sun_col.rgb * l.sun * pow(max(dot(r, normalize(g.sun_pos.xyz - i.rel)), 0.0), e) * mix(0.6, 2.0, calm);
    col /= max(a, 1e-3);
    let white = vec3f(0.85, 0.92, 1.0) * (l.l + ambient(l.day) * 1.3);
    col = mix(col, white, foam);
    a = mix(a, 1.0, foam * 0.9);
    return water_out(col, a);
}

fn water_out(col: vec3f, a: f32) -> vec4f {
    var lit = shoulder(col);
    if (g.opt.y != 1000u) { lit = pow(lit, vec3f(1000.0 / f32(g.opt.y))); }
    return vec4f(lit, clamp(a, 0.0, 1.0));
}

// the water before ADR 0071: the settings' Water off
fn water_old(i: WOut) -> vec4f {
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
    let l = light(i.rel, n, i.body);
    var col = vec3f(0.06, 0.24, 0.36) * (l.l + ambient(l.day) + 0.1 * l.day);
    // the sky in it, and the sun's glint where the sun reaches
    col = mix(col, g.sky.rgb + vec3f(0.17, 0.13, 0.01) * l.day, fres * 0.7);
    col += g.sun_col.rgb * 1.5 * l.sun * pow(max(dot(reflect(-g.sun.xyz, n), v), 0.0), 120.0);
    var a = clamp(0.62 + 0.35 * fres, 0.0, 0.96);
    // rapids churn white
    col = mix(col, vec3f(0.85, 0.92, 1.0), 0.3 * smoothstep(1.5, 3.0, speed));
    if ((i.side & 1u) == 1u) {
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

// one layer of stars: the sky in cells 1/n across, a star in `chance` of them, `faint` the faintest's light
// and `size` its glow's width in pixels
fn star_layer(d: vec3f, n: f32, chance: f32, px: f32, gain: f32, faint: f32, size: f32) -> vec3f {
    let c = vec3i(floor(d * n));
    let a = hash4(c);
    if (a.x > chance) { return vec3f(0.0); }
    let b = hash4(c + vec3i(7919, 104729, 1299709));
    let s = normalize(vec3f(c) + 0.25 + 0.5 * a.yzw);
    let r = length(cross(d, s)) / (px * size); // in glow widths
    // most stars are faint; their colours from red giants to blue-white
    let bright = pow(b.x, 7.0) * gain + faint;
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
    // a moonless night is rich, not empty (bug 2026-11-11_17-47-11, ADR 0072): the band twice as bright and a
    // little blue, the faint stars brighter, and a few dozen bright ones a third of the sky's cells apart, the
    // size of a named star, with a wider glow
    var c = vec3f(0.08, 0.085, 0.11) * band * (0.5 + vnoise(d * 60.0));
    c += star_layer(d, 110.0, 0.09 * (1.0 + 2.0 * band), px, 0.9, 0.03, 1.0);
    c += star_layer(d, 45.0, 0.05, px, 2.5, 0.03, 1.0);
    c += star_layer(d, 9.0, 0.07, px, 3.0, 1.2, 1.6);
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
        // a loose block gliding into its cell (bit 20, ADR 0101): `p` is its offset in cells
        let gp = cell + 0.5 + (c - 0.5) * q.size + select(vec3f(0.0), q.p, ((q.info >> 20u) & 1u) == 1u);
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
    let l = light(i.rel, normalize(i.normal), (i.info >> 8u) & 3u);
    // a crack shows the block's own texel, dark, as a gap in it
    return fx_out(i, fx_colour(i) * select(1.0, 0.3, look == 3u), l.l);
}

// ---- models (ADR 0047, crates/client/src/models.rs) ----
// One draw per part of a model, one instance per piece: the part's quads (corner x | y << 6 | z << 12 | dir << 18
// in its voxels; w | h << 6; rgba8) placed by the piece's transform straight into the camera's frame. Lit by
// `light()` where they stand, as the terrain and the effects are; not in the traced scene, so they cast no
// shadow.
// ---- still models (ADR 0068, 0081, 0082: clutter) ----
// A painted box face put in a level-0 chunk's mesh as one quad (`clutter::quad`): the cell x6 y6 z6 and its place
// in the cell u4 v4 in lo, the face's index in `sfaces` and its turn in sixteenths in hi. Drawn into the G-buffer
// as the terrain is and lit the same; not traced (no shadow).
// flags: 1 a plant's plane (ADR 0091: both sides drawn, swayed, tinted, faded far away; clutter::PLANT)
// mean: face 0's (the meadow ground's) mean colour, RGBA8
struct SFace { o: vec3f, base: u32, eu: vec3f, pw: u32, ev: vec3f, xy: u32, wh: u32, flags: u32, mean: u32, _c: u32 };
const PLANT: u32 = 1u;
// and 2 a shrub's: wide, lopsided, seen twice as far (clutter::SHRUB)
const SHRUB: u32 = 2u;

// The climate's tint of a green pixel (ADR 0091; engine clutter::tint, line for line): heat and wetness 0 to 1,
// the part of the year (0..8, `year_part`) and how much of a season the place has (0 for the grass block, whose
// paintings have theirs). Yellow-green where it is dry and warm, deep green where it is wet; a plant also fresh
// in spring, yellowing from late summer, dull in winter. Leaves can take it the same way.
fn climate_tint(heat: f32, wet: f32, part: u32, season: f32) -> vec3f {
    var shift = array<f32, 8>(-0.2, -0.1, 0.0, 0.15, 0.35, 0.5, 0.4, 0.3);
    let dry = clamp((1.0 - wet) * (0.35 + 0.65 * heat) + shift[part & 7u] * season, 0.0, 1.0);
    return mix(vec3f(0.62, 0.86, 0.56), vec3f(1.4, 0.86, 0.36), dry * dry) * (1.0 - select(0.0, 0.18 * season, part >= 6u));
}
// heat and wetness at `f` (0..1 across the chunk's footprint, x and y) from its corners' bytes: one field over
// every chunk, whose neighbours share their corners
fn chunk_climate(c: Chunk, f: vec2f) -> vec2f {
    let k = clamp(f, vec2f(0.0), vec2f(1.0));
    var v: array<vec2f, 4>;
    for (var n = 0u; n < 4u; n++) {
        let b = (c.clim >> (8u * n)) & 255u;
        v[n] = vec2f(f32(b & 15u), f32(b >> 4u)) / 15.0;
    }
    return mix(mix(v[0], v[1], k.x), mix(v[2], v[3], k.x), k.y);
}
// the part of the year (engine clutter::part): early or late half of `season`'s x
fn year_part(sn: vec3f) -> u32 {
    return u32(floor((fract(sn.x * 0.25) * 4.0 + 0.5) * 2.0)) & 7u;
}
// a painted pixel the tint takes: a green one (flowers, berries and seed heads keep their colour)
fn leafy(c: vec3f) -> bool { return c.g > c.r && c.g > c.b; }

// A plant's sway (ADR 0091): the wind `w` (camera frame, m/s) bends a point `hm` metres up by its square, in gusts
// that roll across the land with `place` (columns) and run on the sky's clock, so every client and every shot of
// the same time sees the same. Leaves can take it the same way.
fn plant_sway(w: vec3f, hm: f32, place: vec2f, seed: f32) -> vec3f {
    let speed = length(w);
    if (speed < 0.01) { return vec3f(0.0); }
    let t = g.water.x;
    let ph = dot(place, vec2f(0.31, 0.23)) + seed * 1.5;
    // periods that divide the clock's hour, so it has no jump
    let gust = 0.4 + 0.6 * sin(t * 6.2831853 * 0.6 + ph) + 0.2 * sin(t * 6.2831853 * 1.5 + ph * 1.7);
    return w / speed * min(speed * 0.025, 0.3) * min(hm * hm, 1.5) * gust;
}
@group(0) @binding(16) var<storage, read> sfaces: array<SFace>;
@group(0) @binding(17) var<storage, read> spaint: array<u32>;

// a model direction (x, y up, z) turned by the quad's sixteenths about y, into the chunk's axes: model y is the
// chunk's z (up), model z its -y, a turn and not a mirror
fn still_dir(q: vec2u, d: vec3f) -> vec3f {
    let a = f32((q.y >> 16u) & 15u) * (6.2831853 / 16.0);
    let r = vec2f(d.x * cos(a) - d.z * sin(a), d.x * sin(a) + d.z * cos(a));
    return vec3f(r.x, -r.y, d.y);
}

// where a still quad's corner uv lies in its chunk's cells (ADR 0068, 0091): its face's model point, a plant
// sized by its column's hash and pressed into the ground by `fade` (1 whole), turned and set in its cell
fn still_local(q: vec2u, uv: vec2f, c: Chunk, fade: f32) -> vec3f {
    let f = sfaces[q.y & 65535u];
    let lo = q.x;
    let cell = vec3f(f32(lo & 63u), f32((lo >> 6u) & 63u), f32((lo >> 12u) & 63u));
    // its place in the cell: up to a quarter metre off the middle
    let off = (vec2f(f32((lo >> 18u) & 15u), f32((lo >> 22u) & 15u)) / 15.0 - 0.5) * 0.5;
    let c0 = vec3f(0.5, 0.0, 0.5);
    let t = plant_turn(f, c, cell);
    var m = turned(t, f.o + f.eu * uv.x + f.ev * uv.y - c0);
    var at = off;
    if ((f.flags & PLANT) != 0u) {
        m.y *= fade;
        at = t.at;
    }
    return cell + vec3f(0.5 + at.x, 0.5 + at.y, 0.0) + still_dir(q, m);
}

// a plant's turn and size (`still_local`): its height (h.x) and width (k.z) its own, turned to any angle (cos and
// sin in `cs`) and set up to a third of a block off the middle (`at`, k), so no row or grid shows from any side; a
// model's is none
struct Turn { cs: vec2f, sc: vec3f, at: vec2f };
fn plant_turn(f: SFace, c: Chunk, cell: vec3f) -> Turn {
    if ((f.flags & PLANT) == 0u) { return Turn(vec2f(1.0, 0.0), vec3f(1.0), vec2f(0.0)); }
    let col = c.base.xy + vec2i(cell.xy);
    let h = hash4(vec3i(col, i32(c.info & 7u)));
    let k = hash4(vec3i(col, i32(c.info & 7u) + 8));
    let a = k.w * 6.2831853;
    var sc = vec3f(0.85 + 0.3 * k.z, 0.8 + 0.4 * h.x, 0.85 + 0.3 * k.z);
    // a shrub: wider than tall and lopsided, one side fuller than the other
    if ((f.flags & SHRUB) != 0u) { sc = vec3f(1.1 + 0.4 * k.z, 0.7 + 0.25 * h.x, 0.85 + 0.4 * fract(k.x * 7.31)); }
    return Turn(vec2f(cos(a), sin(a)), sc, (k.xy - 0.5) * 0.66);
}
// a model direction turned about y and sized by t
fn turned(t: Turn, m: vec3f) -> vec3f {
    return vec3f(m.x * t.cs.x - m.z * t.cs.y, m.y, m.x * t.cs.y + m.z * t.cs.x) * t.sc;
}

// how far from the eye plants stand (`vs_still` fades them by then)
fn still_far() -> f32 { return g.plants.w; }

// how far one plant reaches, by its hash y: 55 to 100 % of the far distance, so the plants thin in a hashed
// order and no ring shows; a shrub twice as far, within the level-0 chunks it is meshed in (about 128 m)
fn plant_reach(flags: u32, hy: f32) -> f32 {
    // the cap first, then the hash, so shrubs too thin in a hashed order and do not all end on one ring
    let far = select(still_far(), min(still_far() * 2.0, 120.0), (flags & SHRUB) != 0u);
    return far * (0.55 + 0.45 * hy);
}

// how far from the eye still quad q of chunk-table entry ci casts a shadow: a plant until it starts to shrink
// (its own reach, as `vs_still`), a model as far as it is drawn
fn still_reach(ci: u32, q: vec2u) -> f32 {
    let fl = sfaces[q.y & 65535u].flags;
    if ((fl & PLANT) == 0u) { return 1e9; }
    let c = chunks[ci];
    let h = hash4(vec3i(c.base.xy + vec2i(i32(q.x & 63u), i32((q.x >> 6u) & 63u)), i32(c.info & 7u)));
    return plant_reach(fl, h.y) * 0.8;
}

// a shadow ray skips still quads this near its start: a few cm, or for a plant's own pixel (`fs_still`) the
// sway's reach, as its drawn blade sways off its still copy in the BLAS
var<private> still_skip: f32 = 0.04;

// the painting's texel of still quad q at a candidate hit's barycentrics bc on its triangle prim
fn still_texel(q: vec2u, prim: u32, bc: vec2f) -> vec4f {
    let f = sfaces[q.y & 65535u];
    // the corners' uv, (0,0) (1,0) (1,1) then (0,0) (1,1) (0,1), weighed by bc: written out, as an array indexed
    // at run time spills to slow memory, and this runs for every blade a shadow ray passes (ADR 0104)
    let uv = select(vec2f(bc.x, bc.x + bc.y), vec2f(bc.x + bc.y, bc.y), (prim & 1u) == 0u);
    let wh = vec2u(f.wh & 65535u, f.wh >> 16u);
    let px = min(vec2u(uv * vec2f(wh)), wh - 1u) + vec2u(f.xy & 65535u, f.xy >> 16u);
    return unpack4x8unorm(spaint[f.base + px.y * f.pw + px.x]);
}

@vertex
fn vs_still(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
    let q = quads[vi / 4u];
    let fi = q.y & 65535u;
    let f = sfaces[fi];
    var cu = array<f32, 6>(0., 1., 1., 0., 1., 0.);
    var cv = array<f32, 6>(0., 0., 1., 0., 1., 1.);
    let uv = vec2f(cu[corner_k(vi, false)], cv[corner_k(vi, false)]);
    let lo = q.x;
    let cell = vec3f(f32(lo & 63u), f32((lo >> 6u) & 63u), f32((lo >> 12u) & 63u));
    let c = chunks[ii];
    let b = g.bodies[(c.info >> 8u) & 255u];
    let plant = (f.flags & PLANT) != 0u;
    var bend = vec3f(0.0);
    var fade = 1.0;
    var h = vec4f(0.0);
    let col = c.base.xy + vec2i(cell.xy);
    if (plant) {
        // how far it reaches and its gusts' phase, from its column: the same on every client. It shrinks into
        // the ground over the last fifth of its own reach (55 to 100 % of the far one): the plants thin, none pops
        h = hash4(vec3i(col, i32(c.info & 7u)));
        let d = length(brot(b, (place(c, b, cell + vec3f(0.5, 0.5, 0.0), cell) - b.cam_hi) - b.cam_lo));
        let d0 = plant_reach(f.flags, h.y);
        fade = 1.0 - smoothstep(d0 * 0.8, d0, d);
        // past its reach it has shrunk into the ground: nothing more to place (ADR 0103)
        if (fade <= 0.0) {
            var o: VOut;
            o.clip = vec4f(0.0, 0.0, 0.0, 1.0);
            return o;
        }
    }
    let local = still_local(q, uv, c, fade);
    if (plant && (c.info >> 8u) == 0u) { bend = plant_sway(brot(b, g.plants.xyz), local.z - cell.z, vec2f(col), h.z); }
    let gp = grid(c, local);
    let fr = frame_at(c.info & 7u, b, gp.x, gp.y);
    // a plant's plane turns, leans and stretches by its place's hash (`still_local`): its normal from its turned
    // edges (the placed corners' differences), so the sun side is the drawn plane's
    var n = still_dir(q, cross(f.ev, f.eu));
    if (plant) {
        let t = plant_turn(f, c, cell);
        n = cross(still_dir(q, turned(t, f.ev)), still_dir(q, turned(t, f.eu)));
    }
    // a plant's planes are lit as the ground under them is, from either side
    let normal = select(normalize(normalize(fr.s - fr.n * fr.ta) * n.x + normalize(fr.t - fr.n * fr.tb) * n.y + fr.up * n.z), normalize(fr.up), plant);
    var o: VOut;
    let rel = brot(b, (place(c, b, local, cell) - b.cam_hi) - b.cam_lo) + bend;
    o.clip = g.view_proj * vec4f(rel, 1.0);
    o.rel = rel;
    // a plant's plane: how squarely it faces the sun (either side), for `fs_still`'s sun side; 1 for models
    let np = normalize(normalize(fr.s - fr.n * fr.ta) * n.x + normalize(fr.t - fr.n * fr.tb) * n.y + fr.up * n.z);
    o.ao = select(1.0, abs(dot(brot(b, np), normalize(g.sun_pos.xyz - rel))), plant);
    // the painting's pixel, in `local.xy`; the face in `dir`; no block's look (0, air: no light of its own, no metal)
    // and in `local.z` its cell's x | y << 6, for a plant's climate
    o.local = vec3f(vec2f(f32(f.xy & 65535u), f32(f.xy >> 16u)) + uv * vec2f(f32(f.wh & 65535u), f32(f.wh >> 16u)), f32(lo & 4095u));
    o.dir = fi;
    o.mat = 0u;
    o.ci = ii;
    o.normal = brot(b, normal);
    return o;
}

// a still quad's painted texel; clear where it is not drawn: a model's plane is two faces back to back, the one
// turned away goes (the pipeline culls nothing); a plant's is one face seen from both sides
fn still_paint(i: VOut) -> vec4f {
    let f = sfaces[i.dir];
    if ((f.flags & PLANT) == 0u && dot(i.normal, i.rel) > 0.0) { return vec4f(0.0); }
    let x0 = vec2u(f.xy & 65535u, f.xy >> 16u);
    let px = clamp(vec2u(max(floor(i.local.xy), vec2f(0.0))), x0, x0 + vec2u(f.wh & 65535u, f.wh >> 16u) - 1u);
    return unpack4x8unorm(spaint[f.base + px.y * f.pw + px.x]);
}

// the still quads' depth pass (ADR 0103), as the cutout blocks' (`fs_cut_depth`): `fs_still` then shades where
// this left its depth (`Equal`), each pixel once, and the terrain under a plant is not shaded at all
@fragment
fn fs_still_depth(i: VOut) -> GOut {
    if (still_paint(i).a < 0.5) {
        discard;
    }
    return GOut(vec4f(0.0), vec4f(0.0), vec2u(0u));
}

@fragment
fn fs_still(i: VOut) -> GOut {
    let f = sfaces[i.dir];
    let plant = (f.flags & PLANT) != 0u;
    // (the back-to-back faces of a model meet the depth pass's alike: the turned-away one still goes)
    let t = still_paint(i);
    if (t.a < 0.5) {
        discard;
    }
    if (plant) {
        // its ground's tint and season, and darker to the root, as grass is in its own shade
        var col = t.rgb;
        let c = chunks[i.ci];
        let cell = u32(i.local.z + 0.5);
        let cxy = vec2f(f32(cell & 63u), f32((cell >> 6u) & 63u));
        if (leafy(col)) {
            let cl = chunk_climate(c, (cxy + 0.5) / 32.0);
            let sn = season(g.bodies[(c.info >> 8u) & 255u], i.rel);
            col *= climate_tint(cl.x, cl.y, year_part(sn), sn.y);
            // each plant a little off its neighbours: warmer or cooler, lighter or darker (its column's hash)
            let h = hash4(vec3i(c.base.xy + vec2i(cxy), i32(c.info & 7u)));
            col *= mix(vec3f(0.94, 1.0, 1.06), vec3f(1.06, 1.0, 0.92), h.w) * (0.9 + 0.2 * h.z);
        }
        // depth: dark at the root, lit at the tips; the planes of a cross a shade apart by how they are turned
        let up = 1.0 - (i.local.y - f32(f.xy >> 16u)) / f32(f.wh >> 16u);
        let turn = abs(f.eu.x) / max(length(f.eu), 1e-4);
        still_skip = 0.35;
        // the planes facing the sun lighter, those edge-on to it darker, so a tuft has a sun side
        var j = i;
        j.ao = 1.0;
        return terrain_lit(j, col * mix(0.42, 1.0, clamp(up * 1.3, 0.0, 1.0)) * mix(0.82, 1.0, turn) * mix(0.8, 1.08, i.ao));
    }
    return terrain_lit(i, t.rgb);
}

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
    let l = light(i.rel, normalize(i.normal), (i.info >> 8u) & 3u);
    return fx_out(i, unpack4x8unorm(i.b).rgb, l.l);
}
