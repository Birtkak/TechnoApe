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
    _p0: f32,
    _p1: f32,
};
struct Globals { view_proj: mat4x4f, sun: vec4f, bodies: array<Body, 4> };
struct Chunk { base: vec3i, info: u32 }; // base in columns and layers; info = face | level << 3 | body << 8

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var<storage, read> quads: array<vec2u>;
@group(0) @binding(2) var<storage, read> chunks: array<Chunk>;

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

@fragment
fn fs(i: VOut) -> @location(0) vec4f {
    let a = i.dir / 2u;
    let sgn = select(1.0, -1.0, (i.dir & 1u) == 1u);
    var base = vec3f(0.5, 0.5, 0.52);
    if (i.mat == 1u) { base = vec3f(0.30, 0.58, 0.20); }
    if (i.mat == 2u) { base = vec3f(0.47, 0.33, 0.20); }
    let c = chunks[i.ci];
    let cell = vec3i(floor(i.local - axis(a) * sgn * 0.5)) + (c.base >> vec3u((c.info >> 3u) & 31u)) + vec3i(i32(c.info & 7u) * 977);
    let f = fract(i.local);
    let t = vec2f(f[(a + 1u) % 3u], f[(a + 2u) % 3u]);
    let edge = min(min(t.x, 1.0 - t.x), min(t.y, 1.0 - t.y));
    let shade = (0.9 + 0.2 * hash(cell)) * select(1.0, 0.8, edge < 0.05);
    let light = 0.35 + 0.65 * max(dot(normalize(i.normal), g.sun.xyz), 0.0);
    return vec4f(base * shade * light, 1.0);
}
