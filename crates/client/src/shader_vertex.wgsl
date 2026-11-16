// The shader, part 1 of 6 (gpu.rs joins them in order): structs, bindings, vertex pulling and the traced geometry.
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
    var u0: vec3f;
    var u1: vec3f;
    if (b.rot0.w == 0.0) {
        let ta1 = tan((g0.x + st - b.half_n) * b.theta);
        let tb1 = tan((g0.y + st - b.half_n) * b.theta);
        u0 = mix(f.up, normalize(f.n + f.s * ta1 + f.t * f.tb), t.x);
        u1 = mix(normalize(f.n + f.s * f.ta + f.t * tb1), normalize(f.n + f.s * ta1 + f.t * tb1), t.x);
    } else {
        // the warped grid (ADR 0131) is not separable: each corner's two tangents
        let gr = body_grid(b);
        let t10 = tx_tan(vec2f(g0.x + st, g0.y), gr);
        let t01 = tx_tan(vec2f(g0.x, g0.y + st), gr);
        let t11 = tx_tan(vec2f(g0.x + st, g0.y + st), gr);
        u0 = mix(f.up, normalize(f.n + f.s * t10.x + f.t * t10.y), t.x);
        u1 = mix(normalize(f.n + f.s * t01.x + f.t * t01.y), normalize(f.n + f.s * t11.x + f.t * t11.y), t.x);
    }
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
    if (a == 0u) { normal = f.si; }
    if (a == 1u) { normal = f.sj; }
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

