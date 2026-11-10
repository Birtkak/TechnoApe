enable wgpu_ray_query;

struct Cam {
    f: vec4f,
    r: vec4f,
    u: vec4f,
    sun: vec4f, // xyz: direction to the sun, w: cos of its angular radius
    size: vec4u, // w, h, frame, mode
}

@group(0) @binding(0) var acc: acceleration_structure;
@group(0) @binding(1) var out: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var<uniform> c: Cam;
@group(0) @binding(3) var<storage, read> verts: array<vec4f>;

fn trace(o: vec3f, d: vec3f, flags: u32) -> RayIntersection {
    var rq: ray_query;
    rayQueryInitialize(&rq, acc, RayDesc(flags, 0xFFu, 0.001, 20000.0, o, d));
    rayQueryProceed(&rq);
    return rayQueryGetCommittedIntersection(&rq);
}

fn normal_of(h: RayIntersection, d: vec3f) -> vec3f {
    let i = h.instance_custom_data + h.primitive_index * 3u;
    let a = verts[i].xyz;
    let n = normalize(cross(verts[i + 1u].xyz - a, verts[i + 2u].xyz - a));
    return select(n, -n, dot(n, d) > 0.0);
}

fn hash(p: vec3u) -> f32 {
    var h = p.x * 73856093u ^ p.y * 19349663u ^ p.z * 83492791u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    return f32(h & 65535u) / 65535.0;
}

// a direction within the sun's disc: soft shadows and, later, partial eclipses
fn sun_sample(seed: vec3u) -> vec3f {
    let s = c.sun.xyz;
    let t = normalize(cross(s, select(vec3f(0., 1., 0.), vec3f(1., 0., 0.), abs(s.y) > 0.9)));
    let b = cross(s, t);
    let u = hash(seed);
    let v = hash(seed + vec3u(17u, 31u, 7u));
    let ct = mix(1.0, c.sun.w, u);
    let st = sqrt(1.0 - ct * ct);
    let ph = 6.2831853 * v;
    return normalize(s * ct + (t * cos(ph) + b * sin(ph)) * st);
}

fn shade(o: vec3f, n: vec3f, seed: vec3u) -> f32 {
    var k = 1u;
    if (c.size.w == 2u) { k = 4u; }
    var lit = 0.0;
    for (var i = 0u; i < k; i++) {
        let s = trace(o + n * 0.05, sun_sample(seed + vec3u(0u, 0u, i * 101u)), RAY_FLAG_TERMINATE_ON_FIRST_HIT);
        lit += select(1.0, 0.0, s.kind != RAY_QUERY_INTERSECTION_NONE);
    }
    return lit / f32(k) * max(dot(n, c.sun.xyz), 0.0);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= c.size.x || id.y >= c.size.y) { return; }
    let uv = (vec2f(id.xy) + 0.5) / vec2f(c.size.xy) * 2.0 - 1.0;
    let d = normalize(c.f.xyz + c.r.xyz * uv.x - c.u.xyz * uv.y);
    let sky = vec3f(0.45, 0.62, 0.9);
    let h = trace(vec3f(0.0), d, 0u);
    var col = sky;
    if (h.kind != RAY_QUERY_INTERSECTION_NONE) {
        let p = d * h.t;
        let n = normal_of(h, d);
        let seed = vec3u(id.xy, c.size.z);
        col = vec3f(0.35, 0.55, 0.25) * (0.25 + 0.75 * shade(p, n, seed));
        if (c.size.w >= 1u) {
            // every surface a mirror-ish metal: one reflection ray, shaded with its own shadow ray
            let rd = reflect(d, n);
            let r = trace(p + n * 0.01, rd, 0u);
            var rc = sky;
            if (r.kind != RAY_QUERY_INTERSECTION_NONE) {
                let rp = p + rd * r.t;
                let rn = normal_of(r, rd);
                rc = vec3f(0.35, 0.55, 0.25) * (0.25 + 0.75 * shade(rp, rn, seed + vec3u(5u)));
            }
            col = mix(col, rc, 0.5);
        }
    }
    textureStore(out, id.xy, vec4f(col, 1.0));
}
