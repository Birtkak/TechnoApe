// The shader, part 4 of 6 (gpu.rs joins them in order): the water, its mesh and its look.
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

// the outward direction at grid point (gi, gj) of a face, the face's normal and in-face axes, the tangents, and the
// normals of its column's sides along i and along j (planes through the centre; the warp's surfaces, ADR 0131)
struct Frame { up: vec3f, n: vec3f, s: vec3f, t: vec3f, ta: f32, tb: f32, si: vec3f, sj: vec3f };
// a body's grid as the light's texels take it (`TxGrid`): the warp rides in rot0.w
fn body_grid(b: Body) -> TxGrid {
    return TxGrid(b.theta, b.half_n, b.r0, b.lnq, b.rot0.w);
}
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
    if (b.rot0.w != 0.0) {
        let x = tx_frame(face, vec2f(gi, gj), body_grid(b));
        f.ta = x.ta;
        f.tb = x.tb;
        f.up = x.up;
        f.si = x.si;
        f.sj = x.sj;
        return f;
    }
    f.ta = tan((gi - b.half_n) * b.theta);
    f.tb = tan((gj - b.half_n) * b.theta);
    f.up = normalize(f.n + f.s * f.ta + f.t * f.tb);
    f.si = normalize(f.s - f.n * f.ta);
    f.sj = normalize(f.t - f.n * f.tb);
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
    // the liquid metal sea's quads (kind 6, world/water.rs metal_quads) are laid as a flat river's
    let metal = (lo >> 28u) == 6u;
    let river = (lo >> 28u) == 5u || metal;
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
        var o = wout(bi, f.up, layer, normalize(f.up - tilt * (b.lnq / b.theta)), select(0u, 2u, metal));
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
    let bi = (q.info >> 3u) & 15u;
    let b = g.bodies[bi];
    let kind = (q.info >> 7u) & 7u;
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
        normal = f.si * select(-1.0, 1.0, kind == 2u);
    } else {
        normal = f.sj * select(-1.0, 1.0, kind == 4u);
    }
    // a glowing fluid (lava, water.rs `glow`): drawn as the metal sea is, in its own light (side bits 2 and 4); another
    // fluid than water in its own colour (oil: info bit 31, side bit 8, `tinted`)
    let glow = q.info >> 10u;
    let tint = (glow >> 21u) != 0u;
    var o = wout(bi, f.up, z, normal, u32(kind != 0u) | select(select(0u, 6u, glow != 0u), 8u, tint));
    o.flow = vec4f(vec3f(f32(glow & 127u), f32((glow >> 7u) & 127u), f32((glow >> 14u) & 127u)) * select(4.0 / 127.0, 1.0 / 127.0, tint), 0.0);
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
    let m = quad_id(q.y);
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
    if ((i.side & 2u) != 0u) { return metal_sea(i); }
    if ((i.side & 8u) != 0u) { return tinted(i); }
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

// the liquid metal sea (draft the metal sea): its own white-hot light, opaque and calm, no sun nor shadow on it;
// large slow convection cells a little darker drift over it
const METAL_GLOW: vec3f = vec3f(4.0, 2.6, 0.9);
fn metal_sea(i: WOut) -> vec4f {
    let t = g.water.x * 0.04;
    let p = i.grid.xy / 7.0;
    let m = 0.6 * wnoise(p + vec2f(t, -0.7 * t)) + 0.4 * wnoise(p * 2.3 + vec2f(-0.6 * t, t) + 17.0);
    // (the shoulder rolls 4.0 and 2.6 both to white: the cells must fall to about a third to show at all)
    // stored lava carries its row's light in `flow` (side bit 4, `vs_water`)
    let hot = select(METAL_GLOW, i.flow.xyz, (i.side & 4u) != 0u);
    return water_out(hot * mix(0.3, 1.0, smoothstep(0.2, 0.7, m)), 1.0);
}

fn water_out(col: vec3f, a: f32) -> vec4f {
    var lit = shoulder(col);
    if (g.opt.y != 1000u) { lit = pow(lit, vec3f(1000.0 / f32(g.opt.y))); }
    return vec4f(lit, clamp(a, 0.0, 1.0));
}

// a fluid in its own colour (oil; `flow.xyz`, side bit 8): opaque, lit by the sun and the sky as the old water
// sheet is, a glint and the sky in it by its Fresnel term; no rays of its own
fn tinted(i: WOut) -> vec4f {
    let v = normalize(-i.rel);
    var n = normalize(i.normal);
    if (dot(n, v) < 0.0) { n = -n; }
    let fres = 0.04 + 0.96 * pow(1.0 - max(dot(n, v), 0.0), 5.0);
    let l = light(i.rel, n, i.body);
    var col = i.flow.xyz * (l.l + ambient(l.day) + 0.1 * l.day);
    col = mix(col, g.sky.rgb + vec3f(0.17, 0.13, 0.01) * l.day, fres * 0.6);
    col += g.sun_col.rgb * 1.5 * l.sun * pow(max(dot(reflect(-g.sun.xyz, n), v), 0.0), 80.0);
    return water_out(col, 1.0);
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

