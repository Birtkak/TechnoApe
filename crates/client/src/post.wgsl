// The bounce light's denoiser and the composite (ADR 0022, at half resolution since ADR 0025 C). `gi_trace`
// (shader.wgsl) leaves one noisy sample per 2x2 pixels, traced from one of them in turn (`guide`). `temporal`
// adds it to that half-resolution pixel's history, found where the same point was last frame (checked by
// normal and distance); `atrous` smooths the result three times with a 3x3 kernel, 1, 2 and 4 half-pixels
// apart, across edges only where the guides' normals and planes agree; `fs_composite` takes the four nearest
// half-resolution values, weighted the same way against each full pixel, times albedo, onto the direct light.

struct Post {
    prev_vp: mat4x4f,
    rays: mat4x4f, // x: the view ray through the screen's top-left corner, y and z its change across x and y
    delta: vec4f,  // the camera's move since last frame, in the camera's frame; w 1 to drop the history
    cfg: vec4u,    // x the frames a history holds at most, y the smoothing step, z the frame number,
                   // w 1 when the bounce light is off
    tune: vec4f,   // rgb the flat fill that stands in for the bounce then; w the screen's brightness
    air: vec4f,    // rgb the air's colour at the horizon; w how fast the ground fades into it, per metre
    amb: vec4f,    // x how much stronger the bounce light is drawn (the sky's light in the shadows: it
                   // reaches only where the bounce rays did, so a cave stays dark); w the body the camera
                   // is on, whose ground alone fades into the air
    // the clouds (ADR 0076, clouds.wgsl): the planet's centre from the camera (w its sea's radius, 0 for no
    // clouds), its frame in the camera's (columns), the sun's direction (w the daylight at the camera), its
    // light (w the steps a ray takes at most), the sky's light on them (w a lightning flash), `Globals::cloud`
    ctr: vec4f,
    rot0: vec4f,
    rot1: vec4f,
    rot2: vec4f,
    sun: vec4f,
    sun_col: vec4f,
    sky_amb: vec4f,
    cloud: vec4f,
};

@group(0) @binding(0) var<uniform> u: Post;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var gbuf: texture_2d<u32>;
@group(0) @binding(3) var gbuf_prev: texture_2d<u32>;
@group(0) @binding(4) var hist_prev: texture_2d<f32>;
@group(0) @binding(5) var dst: texture_storage_2d<rgba16float, write>;

// the G-buffer's normal, folded onto an octahedron as two 16-bit numbers (ADR 0025, J2; `gb_pack` in shader.wgsl)
fn gb_normal(e: u32) -> vec3f {
    let o = unpack2x16snorm(e);
    var n = vec3f(o, 1.0 - abs(o.x) - abs(o.y));
    let t = max(-n.z, 0.0);
    n.x += select(t, -t, n.x >= 0.0);
    n.y += select(t, -t, n.y >= 0.0);
    return normalize(n);
}

// a G-buffer pixel as normal (xyz) and distance (w, 0 for the sky)
fn gload(t: texture_2d<u32>, q: vec2i) -> vec4f {
    let e = textureLoad(t, q, 0).xy;
    return vec4f(gb_normal(e.x), bitcast<f32>(e.y));
}

// the point a full-resolution pixel shows, from the camera: the view ray is affine across the screen
fn point_at(px: vec2i, dist: f32) -> vec3f {
    let uv = (vec2f(px) + 0.5) / vec2f(textureDimensions(gbuf));
    return normalize(u.rays[0].xyz + uv.x * u.rays[1].xyz + uv.y * u.rays[2].xyz) * dist;
}

// the full-resolution pixel a half-resolution one was traced from this frame (gi_trace's order)
fn guide(hp: vec2i) -> vec2i {
    var at = array<vec2i, 4>(vec2i(0, 0), vec2i(1, 1), vec2i(1, 0), vec2i(0, 1));
    return min(hp * 2 + at[u.cfg.z % 4u], vec2i(textureDimensions(gbuf)) - 1);
}

// how much a guide at q with G-buffer gq counts for the point p0 with normal n0 at distance d0
fn edge_w(n0: vec3f, p0: vec3f, d0: f32, q: vec2i, gq: vec4f) -> f32 {
    let wn = pow(max(dot(n0, gq.xyz), 0.0), 32.0);
    let plane = abs(dot(n0, point_at(q, gq.w) - p0));
    return wn * exp(-plane / (0.03 + 0.004 * d0));
}

@compute @workgroup_size(8, 8)
fn temporal(@builtin(global_invocation_id) id: vec3u) {
    let hs = vec2i(textureDimensions(src));
    let size = vec2i(textureDimensions(gbuf));
    let hp = vec2i(id.xy);
    if (hp.x >= hs.x || hp.y >= hs.y) { return; }
    let s = guide(hp);
    let gb = gload(gbuf, s);
    if (gb.w <= 0.0) {
        textureStore(dst, hp, vec4f(0.0));
        return;
    }
    let ns = textureLoad(src, hp, 0);
    let cur = ns.rgb;
    let usable = ns.a > 0.0;
    // where this point was on screen last frame
    let was = point_at(s, gb.w) + u.delta.xyz;
    let c = u.prev_vp * vec4f(was, 1.0);
    var len = 1.0;
    var out = cur;
    if (u.delta.w == 0.0 && c.w > 0.0) {
        let uv = c.xy / c.w;
        let q = vec2i(vec2f((uv.x + 1.0) * 0.5, (1.0 - uv.y) * 0.5) * vec2f(size));
        if (all(q >= vec2i(0)) && all(q < size)) {
            let pg = gload(gbuf_prev, q);
            let same = abs(pg.w - length(was)) < 0.04 * pg.w + 0.05 && dot(pg.xyz, gb.xyz) > 0.9;
            if (same) {
                let h = textureLoad(hist_prev, min(q / 2, hs - 1), 0);
                if (usable) {
                    len = min(h.a + 1.0, f32(u.cfg.x));
                    out = mix(h.rgb, cur, 1.0 / len);
                } else {
                    len = h.a;
                    out = h.rgb;
                }
            }
        }
    }
    textureStore(dst, hp, vec4f(out, len));
}

@compute @workgroup_size(8, 8)
fn atrous(@builtin(global_invocation_id) id: vec3u) {
    let hs = vec2i(textureDimensions(src));
    let hp = vec2i(id.xy);
    if (hp.x >= hs.x || hp.y >= hs.y) { return; }
    let s0 = guide(hp);
    let g0 = gload(gbuf, s0);
    let c0 = textureLoad(src, hp, 0);
    if (g0.w <= 0.0) {
        textureStore(dst, hp, c0);
        return;
    }
    let p0 = point_at(s0, g0.w);
    let step = i32(u.cfg.y);
    var k = array<f32, 3>(0.25, 0.5, 0.25);
    var sum = vec3f(0.0);
    var wsum = 0.0;
    // a young history is noisier: it is smoothed across larger differences of brightness
    let l0 = dot(c0.rgb, vec3f(0.3, 0.5, 0.2));
    let sl = 0.15 + 2.0 / sqrt(max(c0.a, 1.0));
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let hq = clamp(hp + vec2i(x, y) * step, vec2i(0), hs - 1);
            let q = guide(hq);
            let gq = gload(gbuf, q);
            if (gq.w <= 0.0) { continue; }
            let cq = textureLoad(src, hq, 0);
            let wl = exp(-abs(dot(cq.rgb, vec3f(0.3, 0.5, 0.2)) - l0) / (sl * (l0 + 0.02)));
            let w = k[x + 1] * k[y + 1] * edge_w(g0.xyz, p0, g0.w, q, gq) * wl;
            sum += cq.rgb * w;
            wsum += w;
        }
    }
    textureStore(dst, hp, vec4f(sum / max(wsum, 1e-6), c0.a));
}

// ---- the composite: a triangle over the screen ----
@group(0) @binding(6) var direct_t: texture_2d<f32>;
@group(0) @binding(7) var albedo_t: texture_2d<f32>;
@group(0) @binding(8) var bounce_t: texture_2d<f32>;
@group(0) @binding(9) var cl_tex: texture_2d_array<f32>;
@group(0) @binding(10) var cl_samp: sampler;

// the clouds between the camera and what a pixel shows `dist` metres away along view ray `dir` (0: the sky),
// premultiplied (rgb) with the share of what is behind them they hide (a): the slab's one or two pieces in
// front of it, marched in steps of about 8 m (fewer far away), from a point jittered per pixel and frame
fn clouds(px: vec2i, dir: vec3f, dist: f32) -> vec4f {
    let sea = u.ctr.w;
    let steps = u.sun_col.w;
    if (sea <= 0.0 || steps <= 0.0 || u.cloud.w <= 0.0) { return vec4f(0.0); }
    let m = mat3x3f(u.rot0.xyz, u.rot1.xyz, u.rot2.xyz);
    let o = (-u.ctr.xyz) * m;
    // shortcut: under the sea level the water hides them (the G-buffer has no water: from under it they would
    // show on its surface); a camera in a cave under the sea level loses them too, until the water is in the G-buffer
    if (dot(o, o) < sea * sea) { return vec4f(0.0); }
    let d = dir * m;
    let sd = u.sun.xyz * m;
    let far = select(1e9, dist, dist > 0.0);
    let r1 = sea + CL_BASE + CL_THICK;
    let r0 = sea + CL_BASE;
    let b = dot(o, d);
    let c1 = dot(o, o) - r1 * r1;
    let h1 = b * b - c1;
    if (h1 <= 0.0) { return vec4f(0.0); }
    let out_t = -b + sqrt(h1);
    let in_t = max(-b - sqrt(h1), 0.0);
    if (out_t <= 0.0 || in_t >= far) { return vec4f(0.0); }
    // the inner sphere cuts the way through the slab in two where the ray dips under the clouds' base
    let h0 = b * b - (dot(o, o) - r0 * r0);
    var seg = array<vec2f, 2>(vec2f(in_t, min(out_t, far)), vec2f(0.0));
    if (h0 > 0.0) {
        let a0 = -b - sqrt(h0);
        let a1 = -b + sqrt(h0);
        if (a1 > in_t) {
            seg[0] = vec2f(in_t, min(max(a0, in_t), far));
            seg[1] = vec2f(max(a1, in_t), min(out_t, far));
        }
    }
    // (most ground pixels: the slab is behind what they show)
    if (seg[0].y <= seg[0].x && seg[1].y <= seg[1].x) { return vec4f(0.0); }
    let wind = cl_wind(normalize(o + d * max(seg[0].x, select(seg[1].x, seg[0].x, seg[0].y > seg[0].x))), u.cloud.w);
    // by the pixel alone: nothing accumulates the clouds over frames, so a jitter by the frame would only shimmer
    let jit = fract(sin(dot(vec2f(px), vec2f(12.9898, 78.233))) * 43758.5453);
    var tr = 1.0;
    var col = vec3f(0.0);
    var first = -1.0;
    for (var k = 0; k < 2; k++) {
        let s = seg[k];
        let len = s.y - s.x;
        if (len <= 0.0 || tr < 0.02) { continue; }
        let n = clamp(ceil(len / max(8.0, s.x * 0.004)), 1.0, steps);
        let dt = len / n;
        for (var i = 0.0; i < n; i += 1.0) {
            let t = s.x + (i + jit) * dt;
            let q = o + d * t;
            let cs = cl_sample(q, sea, wind, u.cloud);
            if (cs.d <= 0.0) { continue; }
            if (first < 0.0) { first = t; }
            let r = length(q);
            let up = q / r;
            let hf = clamp((r - r0) / CL_THICK, 0.0, 1.0);
            // lit from above by the sun, darker underneath; rain clouds grey to slate
            // (by the sun's height where the cloud is: from orbit the night side's clouds are dark)
            let mu = dot(up, sd);
            let dayk = smoothstep(-0.2, 0.15, mu);
            let sun = u.sun_col.rgb * u.sun.w * clamp(mu * 1.5 + 0.6, 0.0, 1.0) * dayk * (0.45 + 0.55 * hf);
            let albedo = mix(0.95, 0.32, smoothstep(0.15, 0.75, cs.rain));
            let amb = u.sky_amb.rgb * (0.06 + 0.94 * dayk) * (0.55 + 0.45 * hf);
            let lit = albedo * (sun + amb + vec3f(0.9, 0.92, 1.0) * u.sky_amb.w);
            let a = 1.0 - exp(-CL_K * dt * min(cs.d, 1.0));
            col += tr * a * lit;
            tr *= 1.0 - a;
            if (tr < 0.02) { break; }
        }
    }
    if (first > 0.0 && u.air.w > 0.0) {
        // far clouds fade into the air as far ground does
        col = mix(col, u.air.rgb * (1.0 - tr), 1.0 - exp(-first * u.air.w));
    }
    return vec4f(col, 1.0 - tr);
}

@vertex
fn vs_composite(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4f {
    let uv = vec2f(f32((vi << 1u) & 2u), f32(vi & 2u)) * 2.0 - 1.0;
    return vec4f(uv, 0.0, 1.0);
}

@fragment
fn fs_composite(@builtin(position) pos: vec4f) -> @location(0) vec4f {
    let px = vec2i(pos.xy);
    var c = textureLoad(direct_t, px, 0).rgb;
    let gb = gload(gbuf, px);
    if (gb.w > 0.0 && u.cfg.w == 1u) {
        c += textureLoad(albedo_t, px, 0).rgb * u.tune.rgb;
    } else if (gb.w > 0.0) {
        // the four half-resolution values around this pixel, bilinear and by normal and plane against it; the
        // nearest one where none fits
        let hs = vec2i(textureDimensions(bounce_t));
        let f = (vec2f(px) - 0.5) * 0.5;
        let h0 = vec2i(floor(f));
        let t = fract(f);
        let p0 = point_at(px, gb.w);
        var sum = vec3f(0.0);
        var ws = 0.0;
        for (var k = 0; k < 4; k++) {
            let o = vec2i(k & 1, k >> 1u);
            let hq = clamp(h0 + o, vec2i(0), hs - 1);
            let q = guide(hq);
            let gq = gload(gbuf, q);
            if (gq.w <= 0.0) { continue; }
            let bl = select(1.0 - t.x, t.x, o.x == 1) * select(1.0 - t.y, t.y, o.y == 1);
            let w = bl * edge_w(gb.xyz, p0, gb.w, q, gq);
            sum += textureLoad(bounce_t, hq, 0).rgb * w;
            ws += w;
        }
        let bounce = select(textureLoad(bounce_t, min(px / 2, hs - 1), 0).rgb, sum / ws, ws > 1e-4);
        c += textureLoad(albedo_t, px, 0).rgb * bounce * (1.0 + u.amb.x);
    }
    // a lightning flash (ADR 0076) lights everything at once, the sky a little
    if (u.sky_amb.w > 0.0) {
        c += select(vec3f(0.25), textureLoad(albedo_t, px, 0).rgb, gb.w > 0.0) * vec3f(0.8, 0.85, 1.0) * u.sky_amb.w;
    }
    if (gb.w > 0.0 && u.air.w > 0.0 && round(textureLoad(albedo_t, px, 0).a * 3.0) == u.amb.w) {
        // far ground fades into the air (ADR 0030). shortcut: by distance alone, so a long tunnel fades
        // too; the air's own grid (W3) will know where air is lit
        c = mix(c, u.air.rgb, 1.0 - exp(-gb.w * u.air.w));
    }
    c = shoulder(c);
    // the settings' brightness: a curve over the finished picture, 1 leaves it as it is
    if (u.tune.w != 1.0) { c = pow(c, vec3f(1.0 / u.tune.w)); }
    return vec4f(c, 1.0);
}

// the clouds over the screen after the water (which is under them: they are over the sea level), blended as
// premultiplied; a pixel's distance is the G-buffer's, the water left out of it
@fragment
fn fs_clouds(@builtin(position) pos: vec4f) -> @location(0) vec4f {
    let px = vec2i(pos.xy);
    let cl = clouds(px, normalize(point_at(px, 1.0)), gload(gbuf, px).w);
    if (cl.a <= 0.0) { discard; }
    var c = shoulder(cl.rgb / cl.a);
    if (u.tune.w != 1.0) { c = pow(c, vec3f(1.0 / u.tune.w)); }
    return vec4f(c * cl.a, cl.a);
}

// bright light rolls off above 0.8 instead of clipping; everything under it is left as it is
fn shoulder(c: vec3f) -> vec3f {
    let k = 0.8;
    return select(c, k + (1.0 - k) * (1.0 - exp(-(c - k) / (1.0 - k))), c > vec3f(k));
}
