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
    _p1: f32,
    // the body's frame turned into the camera's (ADR 0022): columns of the rotation
    rot0: vec4f,
    rot1: vec4f,
    rot2: vec4f,
    ctr: vec4f,    // the body's centre from the camera, in the camera's frame; w its radius, 0 for no body
};
// sun: the direction to the sun, w the water's clock in s; sun_pos: the sun's centre from the camera, w its
// radius; sun_col: its light; moon_col: the moon's light (rgb, w its body index); sky: the sky's colour here
struct Globals {
    view_proj: mat4x4f,
    sun: vec4f,
    bodies: array<Body, 4>,
    inv_vp: mat4x4f,
    sun_pos: vec4f,
    sun_col: vec4f,
    moon_col: vec4f,
    sky: vec4f,
    n_lamps: vec4u, // x: lamps in `lamps`
};
// base in columns and layers; info = face | level << 3 | body << 8; off: its first quad
struct Chunk { base: vec3i, info: u32, off: u32 };

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var<storage, read> quads: array<vec2u>;
@group(0) @binding(2) var<storage, read> chunks: array<Chunk>;
// a block's look from data/blocks.toml (ADR 0013): info = pattern | side << 8 | metal (0..255) << 16 | gives light
// << 24; patterns 0 plain, 1 grass, 2 stone
struct Look { colour: vec3f, info: u32 };
@group(0) @binding(3) var<storage, read> looks: array<Look, 256>;
// every chunk's terrain near and far, drawn or not, one instance each with mask 1 << body (ADR 0022)
@group(0) @binding(5) var acc: acceleration_structure;
// the lamps near the camera (ADR 0022): pos from the camera, w its body; col its light
struct Lamp { pos: vec4f, col: vec4f };
@group(0) @binding(8) var<storage, read> lamps: array<Lamp>;

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

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
    let q = quads[vi / 6u];
    let dir = (q.x >> 28u) & 7u;
    let a = dir / 2u;
    let k = vi % 6u;
    let local = quad_corner(q, k, 0.0);
    // the corner in the face's u, v: as quad_corner lays it out
    var cu = array<u32, 6>(0u, 1u, 1u, 0u, 1u, 0u);
    var cv = array<u32, 6>(0u, 0u, 1u, 0u, 1u, 1u);
    var ku = cu[k];
    var kv = cv[k];
    if ((dir & 1u) == 1u) { ku = cv[k]; kv = cu[k]; }
    let occl = (q.y >> (8u + 2u * (ku + 2u * kv))) & 3u;
    let c = chunks[ii];
    let b = g.bodies[c.info >> 8u];
    let gp = grid(c, local);
    let f = frame_at(c.info & 7u, b, gp.x, gp.y);
    let r = b.r0 * exp(gp.z * b.lnq);
    // the sides of a column are planes through the body's centre; its top is the sphere
    var normal = f.up;
    if (a == 0u) { normal = normalize(f.s - f.n * f.ta); }
    if (a == 1u) { normal = normalize(f.t - f.n * f.tb); }
    if ((dir & 1u) == 1u) { normal = -normal; }

    var o: VOut;
    let rel = brot(b, (f.up * r - b.cam_hi) - b.cam_lo);
    o.clip = g.view_proj * vec4f(rel, 1.0);
    o.rel = rel;
    o.ao = 1.0 - 0.2 * f32(occl);
    o.local = local;
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

@compute @workgroup_size(64)
fn expand(@builtin(global_invocation_id) id: vec3u) {
    let bd = builds[id.y];
    if (id.x >= bd.n) { return; }
    let q = quads[bd.off + id.x];
    let c = Chunk(bd.base, bd.info, bd.off);
    let b = g.bodies[c.info >> 8u];
    for (var k = 0u; k < 6u; k++) {
        let gp = grid(c, quad_corner(q, k, GROW));
        let p = frame_at(c.info & 7u, b, gp.x, gp.y).up * (b.r0 * exp(gp.z * b.lnq));
        let o = (bd.dst + id.x * 6u + k) * 3u;
        verts[o] = p.x;
        verts[o + 1u] = p.y;
        verts[o + 2u] = p.z;
    }
}

// ---- light (ADR 0022) ----
// The sun is the one source. Direct light is what the bodies' spheres leave of its disc (eclipses, and the
// horizon of the body far away) times one ray through the terrain to a point of the disc (soft shadows).
// The moon reflects it by its phase. The sky's light fades with the sun's height over the horizon here.
const PI: f32 = 3.14159265;
const DAY_AMB: vec3f = vec3f(0.33, 0.35, 0.40);
const NIGHT_AMB: vec3f = vec3f(0.018, 0.022, 0.035);

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

// one ray through the terrain of body `body`: 1 if it gets out, or as far as tmax
fn ray(p: vec3f, n: vec3f, d: vec3f, body: u32, tmax: f32) -> f32 {
    var rq: ray_query;
    let o = p + n * (0.02 + length(p) * 2e-5);
    rayQueryInitialize(&rq, acc, RayDesc(RAY_FLAG_TERMINATE_ON_FIRST_HIT, 1u << body, 0.0, tmax, o, d));
    rayQueryProceed(&rq);
    return select(1.0, 0.0, rayQueryGetCommittedIntersection(&rq).kind != RAY_QUERY_INTERSECTION_NONE);
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

// the light on a surface of body `body` at p (from the camera) with normal n
const LAMP_R: f32 = 8.0;
const LAMP_RAYS: u32 = 3u;
const NONE: u32 = 0xffffffffu;

// a lamp's light d2 square metres away: falling off with the square of the distance, to nothing at LAMP_R
fn lamp_light(lp: Lamp, d2: f32) -> vec3f {
    let x = 1.0 - d2 / (LAMP_R * LAMP_R);
    return lp.col.rgb * x * x * 1.5 / (1.0 + d2);
}

// lamp i's direct light on a surface at p facing n, before its shadow
fn lamp_on(i: u32, p: vec3f, n: vec3f) -> vec3f {
    let v = lamps[i].pos.xyz - p;
    let d2 = dot(v, v);
    return lamp_light(lamps[i], d2) * max(dot(n, v) * inverseSqrt(d2), 0.0);
}

fn light(p: vec3f, n: vec3f, body: u32, px: vec2u, ao: f32) -> Light {
    let up = normalize(p - g.bodies[body].ctr.xyz);
    let sd = normalize(g.sun_pos.xyz - p);
    // eclipses: every other body's sphere over the sun's disc
    var ecl = 1.0;
    for (var i = 0u; i < 4u; i++) {
        if (i != body) { ecl *= left_by(i, p, g.sun_pos.xyz, g.sun_pos.w); }
    }
    var o: Light;
    o.day = smoothstep(-0.2, 0.15, dot(up, sd)) * ecl;
    let mi = u32(g.moon_col.w);
    let moon = g.moon_col.rgb * select(1.0, 0.0, body == mi);
    o.l = (mix(NIGHT_AMB, DAY_AMB, o.day) + moon * 0.25) * ao;
    let ndl = dot(n, sd);
    let vis = ecl * left_by(body, p, g.sun_pos.xyz, g.sun_pos.w);
    if (ndl > 0.0 && vis > 0.0) {
        let ar = g.sun_pos.w / length(g.sun_pos.xyz - p);
        o.sun = vis * ray(p, n, in_disc(sd, ar, px), body, 1e5);
        o.l += g.sun_col.rgb * ndl * o.sun * (0.6 + 0.4 * ao);
    }
    // the moon's light where the sun's is weak, through the same terrain
    if (o.day < 0.5 && max(moon.r, moon.b) > 0.001) {
        let mc = g.bodies[mi].ctr;
        let md = normalize(mc.xyz - p);
        let ndm = dot(n, md);
        if (ndm > 0.0) {
            var mv = 1.0;
            for (var i = 0u; i < 4u; i++) {
                if (i != mi) { mv *= left_by(i, p, mc.xyz, mc.w); }
            }
            if (mv > 0.0) { o.l += moon * ndm * mv * ray(p, n, md, body, 1e5); }
        }
    }
    // the lamps: shadow rays to the LAMP_RAYS strongest here; the others get their mean visibility, and every
    // lamp a share without a ray for the light its room bounces back
    var top = array<u32, LAMP_RAYS>(NONE, NONE, NONE);
    var topw = array<f32, LAMP_RAYS>(0.0, 0.0, 0.0);
    var rest = vec3f(0.0);
    for (var i = 0u; i < g.n_lamps.x; i++) {
        let lp = lamps[i];
        let v = lp.pos.xyz - p;
        let d2 = dot(v, v);
        if (u32(lp.pos.w) != body || d2 > LAMP_R * LAMP_R) { continue; }
        o.l += lamp_light(lp, d2) * 0.12 * ao;
        let w = dot(lamp_on(i, p, n), vec3f(0.3, 0.5, 0.2));
        if (w <= 0.0) { continue; }
        // keep the strongest: insert into the short list, the weakest falls to `rest`
        var j = i;
        var wj = w;
        for (var k = 0u; k < LAMP_RAYS; k++) {
            if (wj > topw[k]) {
                let tj = top[k];
                let tw = topw[k];
                top[k] = j;
                topw[k] = wj;
                j = tj;
                wj = tw;
            }
        }
        if (j != NONE) { rest += lamp_on(j, p, n); }
    }
    var seen_n = 0.0;
    var seen_v = 0.0;
    for (var k = 0u; k < LAMP_RAYS; k++) {
        if (top[k] == NONE) { continue; }
        let lp = lamps[top[k]];
        let v = lp.pos.xyz - p;
        let d = length(v);
        // stop short of the lamp's own block: a 0.5 m block's corners are 0.433 m from its middle
        let vis = select(ray(p, n, v / d, body, d - 0.44), 1.0, d < 0.45);
        o.l += lamp_on(top[k], p, n) * vis;
        seen_n += 1.0;
        seen_v += vis;
    }
    if (seen_n > 0.0) { o.l += rest * (seen_v / seen_n); }
    return o;
}

// what a ray from p sees: the terrain it hits, lit, or the sky (a metal's reflection)
fn seen(p: vec3f, d: vec3f, body: u32, px: vec2u) -> vec3f {
    var rq: ray_query;
    rayQueryInitialize(&rq, acc, RayDesc(0u, 1u << body, 0.0, 4000.0, p, d));
    rayQueryProceed(&rq);
    let h = rayQueryGetCommittedIntersection(&rq);
    if (h.kind == RAY_QUERY_INTERSECTION_NONE) { return sky_colour(d); }
    let c = chunks[h.instance_custom_data];
    let q = quads[c.off + h.primitive_index / 2u];
    let dir = (q.x >> 28u) & 7u;
    let a = dir / 2u;
    let b = g.bodies[c.info >> 8u];
    let gp = grid(c, quad_corner(q, 0u, 0.0));
    let f = frame_at(c.info & 7u, b, gp.x, gp.y);
    var hn = f.up;
    if (a == 0u) { hn = normalize(f.s - f.n * f.ta); }
    if (a == 1u) { hn = normalize(f.t - f.n * f.tb); }
    if ((dir & 1u) == 1u) { hn = -hn; }
    let m = q.y & 255u;
    if ((looks[m].info >> 24u) == 1u) { return looks[m].colour; }
    return looks[m].colour * light(p + d * h.t, brot(b, hn), body, px, 1.0).l;
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
    let look = looks[i.mat & 255u];
    if ((look.info >> 24u) == 1u) { return vec4f(col * 1.15, 1.0); } // it gives light: it shows its own
    let n = normalize(i.normal);
    let body = c.info >> 8u;
    let px = vec2u(i.clip.xy);
    let l = light(i.rel, n, body, px, i.ao);
    var out = col * l.l;
    let metal = f32((look.info >> 16u) & 255u) / 255.0;
    if (metal > 0.0) {
        // a metal mirrors what is around it, tinted by its colour, more so at a grazing angle
        let v = -normalize(i.rel);
        let fres = metal + (1.0 - metal) * pow(1.0 - max(dot(n, v), 0.0), 5.0);
        let r = seen(i.rel + n * (0.02 + length(i.rel) * 2e-5), reflect(-v, n), body, px);
        out = mix(out, r * mix(vec3f(1.0), col, metal), fres);
        // and the sun's glint, where the sun reaches
        out += col * g.sun_col.rgb * 3.0 * l.sun * metal * pow(max(dot(reflect(-v, n), normalize(g.sun_pos.xyz - i.rel)), 0.0), 48.0);
    }
    return vec4f(shoulder(out), 1.0);
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
const ASPECT: f32 = 1920.0 / 1080.0;

struct WOut {
    @builtin(position) clip: vec4f,
    @location(0) rel: vec3f, // from the camera
    @location(1) normal: vec3f,
    @location(2) @interpolate(flat) side: u32,
    @location(3) @interpolate(flat) body: u32,
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
    return wout(c.info >> 8u, f.up, layer, f.up, 0u);
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
    if (kind == 5u) {
        // a droplet: a square of cur.w metres facing the camera, at (i + cur.x, j + cur.y, layer cur.z)
        let f = frame_at(face, b, f32(q.i) + q.cur.x, f32(q.j) + q.cur.y);
        var o = wout(bi, f.up, q.cur.z, -normalize(f.up), 2u);
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
    return wout(bi, f.up, z, normal, u32(kind != 0u));
}

@fragment
fn fs_water(i: WOut) -> @location(0) vec4f {
    let v = normalize(-i.rel);
    var n = normalize(i.normal);
    if (dot(n, v) < 0.0) { n = -n; } // seen from under the surface
    let facing = max(dot(n, v), 0.0);
    let fres = 0.08 + 0.92 * pow(1.0 - facing, 5.0);
    let l = light(i.rel, n, i.body, vec2u(i.clip.xy), 1.0);
    var col = vec3f(0.06, 0.24, 0.36) * (l.l + 0.1 * l.day);
    // the sky in it, and the sun's glint where the sun reaches
    col = mix(col, g.sky.rgb + vec3f(0.17, 0.13, 0.01) * l.day, fres * 0.7);
    col += g.sun_col.rgb * 1.5 * l.sun * pow(max(dot(reflect(-g.sun.xyz, n), v), 0.0), 120.0);
    var a = clamp(0.62 + 0.35 * fres, 0.0, 0.96);
    if (i.side == 2u) {
        return vec4f(0.85, 0.93, 1.0, 0.85);
    }
    if (i.side == 1u) {
        // a front or a fall: churned, lighter
        col = mix(col, vec3f(0.85, 0.92, 1.0), 0.25);
        a = max(a, 0.8);
    }
    return vec4f(shoulder(col), a);
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
fn fs_sky(i: SkyOut) -> @location(0) vec4f {
    let w = g.inv_vp * vec4f(i.ndc, 1.0, 1.0);
    return vec4f(sky_colour(normalize(w.xyz / w.w)), 1.0);
}

// the sky in direction d
fn sky_colour(d: vec3f) -> vec3f {
    let s = normalize(g.sun_pos.xyz);
    let rs = g.sun_pos.w / length(g.sun_pos.xyz);
    let ang = atan2(length(cross(d, s)), dot(d, s));
    var c = g.sky.rgb;
    let glow = exp(-ang * 12.0) * 0.35 + exp(-ang * 80.0) * 0.6;
    c += g.sun_col.rgb * glow * g.sky.w;
    if (ang < rs) {
        // limb darkening: the disc dims towards its edge
        c = mix(vec3f(1.0, 0.86, 0.6), vec3f(1.0, 0.98, 0.92), sqrt(1.0 - (ang / rs) * (ang / rs)));
    }
    return c;
}
