// The shader, part 5 of 6 (gpu.rs joins them in order): the sky, the stars and the effects.
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
        if (a == 0u) { nb = f.si; }
        if (a == 1u) { nb = f.sj; }
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

