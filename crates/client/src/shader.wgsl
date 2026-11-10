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
    _p1: f32,
};
struct Globals { view_proj: mat4x4f, sun: vec4f, bodies: array<Body, 4> }; // sun.w: the water's clock, s
struct Chunk { base: vec3i, info: u32 }; // base in columns and layers; info = face | level << 3 | body << 8

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var<storage, read> quads: array<vec2u>;
@group(0) @binding(2) var<storage, read> chunks: array<Chunk>;
// a block's look from data/blocks.toml (ADR 0013): info = pattern | side << 8; patterns 0 plain, 1 grass, 2 stone
struct Look { colour: vec3f, info: u32 };
@group(0) @binding(3) var<storage, read> looks: array<Look, 256>;

struct VOut {
    @builtin(position) clip: vec4f,
    @location(0) local: vec3f,
    @location(1) @interpolate(flat) dir: u32,
    @location(2) @interpolate(flat) mat: u32,
    @location(3) @interpolate(flat) ci: u32,
    @location(4) normal: vec3f,
};

fn axis(i: u32) -> vec3f { return vec3f(f32(i == 0u), f32(i == 1u), f32(i == 2u)); }

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
    let q = quads[vi / 6u];
    let k = vi % 6u;
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
    let local = p + axis((a + 1u) % 3u) * (du * w) + axis((a + 2u) % 3u) * (dv * h);

    let c = chunks[ii];
    let face = c.info & 7u;
    let level = (c.info >> 3u) & 31u;
    let b = g.bodies[c.info >> 8u];
    // face normal and in-face axes, the same table as engine::world::AX
    var fnv = array<vec3f, 6>(vec3f(1., 0., 0.), vec3f(-1., 0., 0.), vec3f(0., 1., 0.), vec3f(0., -1., 0.), vec3f(0., 0., 1.), vec3f(0., 0., -1.));
    var fsv = array<vec3f, 6>(vec3f(0., 1., 0.), vec3f(0., 0., 1.), vec3f(0., 0., 1.), vec3f(1., 0., 0.), vec3f(1., 0., 0.), vec3f(0., 1., 0.));
    var ftv = array<vec3f, 6>(vec3f(0., 0., 1.), vec3f(0., 1., 0.), vec3f(1., 0., 0.), vec3f(0., 0., 1.), vec3f(0., 1., 0.), vec3f(1., 0., 0.));
    let n = fnv[face];
    let s = fsv[face];
    let t = ftv[face];
    let gp = c.base + (vec3i(local) << vec3u(level));
    let ta = tan((f32(gp.x) - b.half_n) * b.theta);
    let tb = tan((f32(gp.y) - b.half_n) * b.theta);
    let up = normalize(n + s * ta + t * tb);
    let r = b.r0 * exp(f32(gp.z) * b.lnq);
    // the sides of a column are planes through the body's centre; its top is the sphere
    var normal = up;
    if (a == 0u) { normal = normalize(s - n * ta); }
    if (a == 1u) { normal = normalize(t - n * tb); }
    if ((dir & 1u) == 1u) { normal = -normal; }

    var o: VOut;
    o.clip = g.view_proj * vec4f((up * r - b.cam_hi) - b.cam_lo, 1.0);
    o.local = local;
    o.dir = dir;
    o.mat = q.y;
    o.ci = ii;
    o.normal = normal;
    return o;
}

fn hash(p: vec3i) -> f32 {
    var h = u32(p.x) * 73856093u ^ u32(p.y) * 19349663u ^ u32(p.z) * 83492791u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    return f32(h & 1023u) / 1023.0;
}

// Block look: 16 texels per 0.5 m block (ADR 0002, prototypes/block-look/).
// shortcut: texels are hashed here instead of read from 16x16 images, until content brings a texture atlas.
const TEXELS: f32 = 16.0;
const PLAIN: u32 = 0u;
const GRASS: u32 = 1u;
const STONE: u32 = 2u;

// a pixel-art texel on an N x N grid per block face: 3 tones of noise, grass tops, a grass fringe on the
// upper rows of a grass block's sides, dark crack texels in stone
fn texel(mat: u32, dir: u32, cell: vec3i, t: vec2f, hgt: f32) -> vec3f {
    let n = TEXELS;
    let ti = vec2i(floor(t * n));
    let h = hash(cell * i32(n) + vec3i(ti.x, ti.y, i32(dir)));
    let tone = 0.82 + 0.12 * floor(h * 3.0);
    var m = mat & 255u;
    if ((looks[m].info & 255u) == GRASS && dir != 4u) {
        // a side of a grass block takes its side block's look, with 2-4 rows of grass over the top edge
        let fringe = 2.0 + floor(hash(cell * i32(n) + vec3i(ti.x, 7, i32(dir))) * 3.0);
        if (floor(hgt * n) < n - fringe) { m = looks[m].info >> 8u; }
    }
    let look = looks[m];
    let pattern = look.info & 255u;
    var c = look.colour * tone;
    if (pattern == STONE && h > 0.94) { c *= 0.6; }
    if (pattern == GRASS && h > 0.9) { c *= 1.15; }
    return c;
}

@fragment
fn fs(i: VOut) -> @location(0) vec4f {
    let a = i.dir / 2u;
    let sgn = select(1.0, -1.0, (i.dir & 1u) == 1u);
    let c = chunks[i.ci];
    let cell = vec3i(floor(i.local - axis(a) * sgn * 0.5)) + (c.base >> vec3u((c.info >> 3u) & 31u)) + vec3i(i32(c.info & 7u) * 977);
    let f = fract(i.local);
    let t = vec2f(f[(a + 1u) % 3u], f[(a + 2u) % 3u]);
    let col = texel(i.mat, i.dir, cell, t, f.z);
    let light = 0.35 + 0.65 * max(dot(normalize(i.normal), g.sun.xyz), 0.0);
    return vec4f(col * light, 1.0);
}

// ---- water (ADR 0020) ----
// The generated sea is drawn from the chunks' trailing quads (`vs_sea`), laid at the exact sea level; the
// water mesh of tiles that hold stored water (`vs_water`, crates/client/src/water.rs) moves its heights from
// `prev` to `cur` over one server tick. Both are blended over the terrain, seen from above and below.
struct WQuad { i: i32, j: i32, info: u32, t0: f32, prev: vec4f, cur: vec4f };
@group(0) @binding(4) var<storage, read> wq: array<WQuad>;
const MOVE_S: f32 = 0.05;
const ASPECT: f32 = 1920.0 / 1080.0;

struct WOut {
    @builtin(position) clip: vec4f,
    @location(0) rel: vec3f, // from the camera
    @location(1) normal: vec3f,
    @location(2) @interpolate(flat) side: u32,
};

// the outward direction at grid point (gi, gj) of a face, and the face's normal and in-face axes
struct Frame { up: vec3f, n: vec3f, s: vec3f, t: vec3f, ta: f32, tb: f32 };
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

fn wout(b: Body, up: vec3f, layer: f32, normal: vec3f, side: u32) -> WOut {
    let rel = (up * (b.r0 * exp(layer * b.lnq)) - b.cam_hi) - b.cam_lo;
    var o: WOut;
    o.clip = g.view_proj * vec4f(rel, 1.0);
    o.rel = rel;
    o.normal = normal;
    o.side = side;
    return o;
}

@vertex
fn vs_sea(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> WOut {
    let q = quads[vi / 6u];
    let k = vi % 6u;
    let lo = q.x;
    let p = vec2f(f32(lo & 63u), f32((lo >> 6u) & 63u));
    let w = f32(((lo >> 18u) & 31u) + 1u);
    let h = f32(((lo >> 23u) & 31u) + 1u);
    var cu = array<f32, 6>(0., 1., 1., 0., 1., 0.);
    var cv = array<f32, 6>(0., 0., 1., 0., 1., 1.);
    let c = chunks[ii];
    let level = (c.info >> 3u) & 31u;
    let b = g.bodies[c.info >> 8u];
    let gp = vec2f(c.base.xy) + (p + vec2f(cu[k] * w, cv[k] * h)) * f32(1u << level);
    let f = frame_at(c.info & 7u, b, gp.x, gp.y);
    // a far coarse cell that holds the sea level but is solid: the water lies a layer over its top, or the
    // top face hides it
    var layer = b.sea;
    if (q.y == 1u) {
        layer = max(layer, f32(c.base.z + (i32((lo >> 12u) & 63u) << level)) + 1.0);
    }
    return wout(b, f.up, layer, f.up, 0u);
}

@vertex
fn vs_water(@builtin(vertex_index) vi: u32) -> WOut {
    let q = wq[vi / 6u];
    var tri = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
    let c = tri[vi % 6u];
    let hh = mix(q.prev, q.cur, clamp((g.sun.w - q.t0) / MOVE_S, 0.0, 1.0));
    let face = q.info & 7u;
    let b = g.bodies[(q.info >> 3u) & 3u];
    let kind = (q.info >> 5u) & 7u;
    if (kind == 5u) {
        // a droplet: a square of cur.w metres facing the camera, at (i + cur.x, j + cur.y, layer cur.z)
        let f = frame_at(face, b, f32(q.i) + q.cur.x, f32(q.j) + q.cur.y);
        var o = wout(b, f.up, q.cur.z, -normalize(f.up), 2u);
        var cx = array<f32, 6>(-1., 1., 1., -1., 1., -1.);
        var cy = array<f32, 6>(-1., -1., 1., -1., 1., 1.);
        let k = vi % 6u;
        // world size in clip space: 1 / tan(35 degrees), the field of view's half
        o.clip = vec4f(o.clip.xy + vec2f(cx[k] / ASPECT, cy[k]) * q.cur.w * 1.43, o.clip.zw);
        return o;
    }
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
    return wout(b, f.up, z, normal, u32(kind != 0u));
}

@fragment
fn fs_water(i: WOut) -> @location(0) vec4f {
    let v = normalize(-i.rel);
    var n = normalize(i.normal);
    if (dot(n, v) < 0.0) { n = -n; } // seen from under the surface
    let facing = max(dot(n, v), 0.0);
    let fres = 0.08 + 0.92 * pow(1.0 - facing, 5.0);
    let light = 0.45 + 0.55 * max(dot(n, g.sun.xyz), 0.0);
    var col = vec3f(0.06, 0.24, 0.36) * light;
    col = mix(col, vec3f(0.62, 0.78, 0.96), fres * 0.7);
    col += vec3f(pow(max(dot(reflect(-g.sun.xyz, n), v), 0.0), 120.0));
    var a = clamp(0.62 + 0.35 * fres, 0.0, 0.96);
    if (i.side == 2u) {
        return vec4f(0.85, 0.93, 1.0, 0.85);
    }
    if (i.side == 1u) {
        // a front or a fall: churned, lighter
        col = mix(col, vec3f(0.85, 0.92, 1.0), 0.25);
        a = max(a, 0.8);
    }
    return vec4f(col, a);
}
