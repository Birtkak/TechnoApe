// The planet's clouds from its air grid (ADR 0076), shared by shader.wgsl (their shadow on the ground, the
// snow lying on it) and post.wgsl (the clouds themselves, marched over the finished picture). Each declares
// `cl_tex` (texture_2d_array<f32>, frame.rs `Sky::upload`) and `cl_samp` (a linear sampler) before using these.
//
// The map is one texel per air column, CL_TEX texels a side: layers 0 to 5 the six faces' (cover, rain's
// root share, wind east, wind north; the winds biased by a half), layers 6 to 11 their (lying snow, the air's
// temperature + 128 / 255). Its columns are laid as the engine's cube-sphere grid (`bake::grid`): column i
// along a face's s axis, j along t, equal angles.
//
// A cloud is a slab of cells between CL_BASE and CL_BASE + CL_THICK over the sea, CL_SUB cells across an air
// column (16 m on Terra): a cell is filled where the column's cover at its middle beats the cell's own number
// (clumped by a coarser noise), its top higher the more it beats it, its edge soft where it barely does. The
// cells slide with the column's wind in two phases half a period apart, faded across (as a river's flow), so
// the pattern never shears however long the wind blows; the cover itself moves as the grid carries it.

const CL_TEX: f32 = 128.0;
// over the hills, under the mountains: the 4 km planet's stretched peaks (up to 350 m over the sea, bake::land
// `stretch`) stand into and through the slab (ADR 0105; 0098 had 100 for the old 120 m peaks, 0076 300)
const CL_BASE: f32 = 250.0;
const CL_THICK: f32 = 64.0;
const CL_SUB: f32 = 8.0;
const CL_PI: f32 = 3.14159265;
// a filled cell's light falloff: 1 - exp(-CL_K * metres) of the light behind it is stopped
// (0.14, was 0.09: a cell's edge is crisper, less of it half seen through; bug 2026-11-13_14-38-44)
const CL_K: f32 = 0.14;

struct ClFace { n: vec3f, s: vec3f, t: vec3f };
// (axes by bits, not local arrays: an array indexed at run time spills to slow memory on most GPUs)
fn cl_axis(a: u32) -> vec3f { return vec3f(f32(a == 0u), f32(a == 1u), f32(a == 2u)); }
fn cl_face(f: u32) -> ClFace {
    let n = cl_axis(f >> 1u) * select(1.0, -1.0, (f & 1u) == 1u);
    // each face's s and t axes two bits a face (`bake::grid::AX`): s 1 2 2 0 0 1, t 2 1 0 2 1 0
    return ClFace(n, cl_axis((1065u >> (2u * f)) & 3u), cl_axis((390u >> (2u * f)) & 3u));
}

// the face of body-frame direction d and its continuous column coordinates there (x, y), m columns a face edge
struct ClAt { f: u32, xy: vec2f };
fn cl_locate(d: vec3f, m: f32) -> ClAt {
    let a = abs(d);
    var f = 4u + select(0u, 1u, d.z < 0.0);
    if (a.x >= a.y && a.x >= a.z) { f = select(0u, 1u, d.x < 0.0); } else if (a.y >= a.z) { f = 2u + select(0u, 1u, d.y < 0.0); }
    let fa = cl_face(f);
    let dn = dot(d, fa.n);
    let xy = (atan(vec2f(dot(d, fa.s), dot(d, fa.t)) / dn) / (CL_PI * 0.5) * 2.0 + 1.0) * 0.5 * m;
    return ClAt(f, xy);
}

// the map's texel of a face at column coordinates xy, bilinear between columns and held at the face's edge
// shortcut: the cover, the wind and the cells' pattern step at the cube's twelve edges (each face is sampled
// alone); pad each face with its neighbours' columns if the seam shows
fn cl_map(f: u32, xy: vec2f, m: f32, layer: u32) -> vec4f {
    let uv = clamp(xy, vec2f(0.5), vec2f(m - 0.5)) / CL_TEX;
    return textureSampleLevel(cl_tex, cl_samp, uv, f + layer, 0.0);
}

fn cl_hash(p: vec3i) -> f32 {
    var v = vec3u(p) * 1664525u + 1013904223u;
    v.x += v.y * v.z; v.y += v.z * v.x; v.z += v.x * v.y;
    v = v ^ (v >> vec3u(16u));
    return f32((v.x + v.y * v.z) & 0xffffffu) / 16777216.0;
}

// what every cell's number is raised by: the sky a little more open than the cover says (bug 2026-11-13_14-38-44)
const CL_OPEN: f32 = 0.06;

// the cell's own number: a clump noise over four cells and the cell's own, so clouds come in heaps
fn cl_number(f: u32, c: vec2i) -> f32 {
    let q = c >> vec2u(2u);
    let t = (vec2f(c & vec2i(3)) + 0.5) / 4.0;
    let fi = i32(f) * 977;
    let a = mix(cl_hash(vec3i(q, fi)), cl_hash(vec3i(q + vec2i(1, 0), fi)), t.x);
    let b = mix(cl_hash(vec3i(q + vec2i(0, 1), fi)), cl_hash(vec3i(q + vec2i(1, 1), fi)), t.x);
    return 0.7 * mix(a, b, t.y) + 0.3 * cl_hash(vec3i(c, fi + 13)) + CL_OPEN;
}

// the wind at body-frame direction d, body-frame metres a second (the planet spins about z: east is z x d)
fn cl_wind(d: vec3f, m: f32) -> vec3f {
    let at = cl_locate(d, m);
    let w = (cl_map(at.f, at.xy, m, 0u).ba - 128.0 / 255.0) * 255.0 * 0.25;
    let east = normalize(cross(vec3f(0.0, 0.0, 1.0), d) + vec3f(1e-6, 0.0, 0.0));
    return east * w.x + cross(d, east) * w.y;
}

// a point's cloud: its density 0 to 1 and the rain its cell holds (the root share). q is body-frame metres from
// the centre; sea the sea's radius; wind the wind there (`cl_wind`, once a ray: it changes slowly); cfg as
// `Globals::cloud` (x the drift's phase 0 to 1, y its period s, w the map's columns a face edge).
struct ClS { d: f32, rain: f32 };
fn cl_sample(q: vec3f, sea: f32, wind: vec3f, cfg: vec4f) -> ClS {
    var o = ClS(0.0, 0.0);
    let r = length(q);
    let h = (r - sea - CL_BASE) / CL_THICK;
    if (h < 0.0 || h > 1.0) { return o; }
    let m = cfg.w;
    let at = cl_locate(q / r, m);
    let fa = cl_face(at.f);
    // the wind in this face's cells a second (a column is a quarter turn over m of the sea's radius)
    let wf = vec2f(dot(wind, fa.s), dot(wind, fa.t)) / (sea * CL_PI * 0.5 / m) * CL_SUB;
    let p = at.xy * CL_SUB;
    for (var k = 0u; k < 2u; k++) {
        let ph = fract(cfg.x + 0.5 * f32(k));
        let wk = 1.0 - abs(2.0 * ph - 1.0);
        let off = wf * (ph * cfg.y);
        let c = vec2i(floor(p - off));
        let mid = (vec2f(c) + 0.5 + off) / CL_SUB;
        let t = cl_map(at.f, mid, m, 0u);
        let margin = t.r - cl_number(at.f, c);
        // the more the cover beats the cell's number, the taller the cell; a thin cell is a soft one
        if (margin > 0.0 && h < 0.3 + margin * 2.0) {
            o.d += wk * smoothstep(0.0, 0.04, margin);
            o.rain += wk * t.g;
        }
    }
    return o;
}

// `cl_number`'s clump read anywhere, not per cell: p in cells (a cell's middle gives its own clump)
fn cl_clump(f: u32, p: vec2f) -> f32 {
    let g = p / 4.0;
    let q = vec2i(floor(g));
    let t = fract(g);
    let fi = i32(f) * 977;
    let a = mix(cl_hash(vec3i(q, fi)), cl_hash(vec3i(q + vec2i(1, 0), fi)), t.x);
    let b = mix(cl_hash(vec3i(q + vec2i(0, 1), fi)), cl_hash(vec3i(q + vec2i(1, 1), fi)), t.x);
    return mix(a, b, t.y);
}

// a point's cloud as its shadow sees it, 0 to 1: `cl_sample`'s field made smooth (the cover and the clump read
// where the point is, the cells' own numbers by their mean, the edge as wide as those numbers spread), so a
// shadow takes the cloud's shape with a soft edge and no cell's square in it
fn cl_soft(q: vec3f, sea: f32, wind: vec3f, cfg: vec4f) -> f32 {
    let m = cfg.w;
    let at = cl_locate(normalize(q), m);
    let fa = cl_face(at.f);
    let wf = vec2f(dot(wind, fa.s), dot(wind, fa.t)) / (sea * CL_PI * 0.5 / m) * CL_SUB;
    let cover = cl_map(at.f, at.xy, m, 0u).r;
    var d = 0.0;
    for (var k = 0u; k < 2u; k++) {
        let ph = fract(cfg.x + 0.5 * f32(k));
        let wk = 1.0 - abs(2.0 * ph - 1.0);
        let margin = cover - (0.7 * cl_clump(at.f, at.xy * CL_SUB - wf * (ph * cfg.y)) + 0.15 + CL_OPEN);
        d += wk * smoothstep(-0.15, 0.15, margin);
    }
    return clamp(d, 0.0, 1.0);
}

// the share of the sun's light the clouds let through to a point p (body-frame metres from the centre) on its
// way from direction sd: the cloud where that ray crosses the slab's middle
fn cl_shadow(p: vec3f, sd: vec3f, sea: f32, cfg: vec4f) -> f32 {
    if (cfg.w <= 0.0) { return 1.0; }
    let rm = sea + CL_BASE + CL_THICK * 0.5;
    let b = dot(p, sd);
    let c = dot(p, p) - rm * rm;
    if (c > 0.0) { return 1.0; }
    let t = -b + sqrt(b * b - c);
    let q = p + sd * t;
    return 1.0 - 0.8 * cl_soft(q, sea, cl_wind(normalize(q), cfg.w), cfg);
}

// lying snow at body-frame direction d, 0 to 1 (the share of SNOW_TOP in weather.rs)
fn cl_snow(d: vec3f, cfg: vec4f) -> f32 {
    if (cfg.w <= 0.0) { return 0.0; }
    let at = cl_locate(d, cfg.w);
    return cl_map(at.f, at.xy, cfg.w, 6u).r;
}
