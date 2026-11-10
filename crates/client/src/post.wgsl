// The bounce light's denoiser and the composite (ADR 0022). `gi_trace` (shader.wgsl) leaves one noisy sample
// per pixel. `temporal` adds it to the pixel's history, found where the same point was last frame (checked by
// normal and distance); `atrous` smooths the result four times with a 3x3 kernel, 1, 2, 4 and 8 pixels apart, across edges only
// where normals and planes agree; `fs_composite` adds albedo times that light to the direct light.

struct Post {
    prev_vp: mat4x4f,
    inv_vp: mat4x4f,
    delta: vec4f, // the camera's move since last frame, in the camera's frame; w 1 to drop the history
    cfg: vec4u,   // x the frames a history holds at most, y the smoothing step, z the frame number
};

@group(0) @binding(0) var<uniform> u: Post;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var gbuf: texture_2d<f32>;
@group(0) @binding(3) var gbuf_prev: texture_2d<f32>;
@group(0) @binding(4) var hist_prev: texture_2d<f32>;
@group(0) @binding(5) var dst: texture_storage_2d<rgba16float, write>;

// the point a pixel shows, from the camera
fn point_at(px: vec2i, dist: f32) -> vec3f {
    let size = vec2f(textureDimensions(gbuf));
    let uv = (vec2f(px) + 0.5) / size;
    let w = u.inv_vp * vec4f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 1.0, 1.0);
    return normalize(w.xyz / w.w) * dist;
}

@compute @workgroup_size(8, 8)
fn temporal(@builtin(global_invocation_id) id: vec3u) {
    let size = vec2i(textureDimensions(gbuf));
    let px = vec2i(id.xy);
    if (px.x >= size.x || px.y >= size.y) { return; }
    let gb = textureLoad(gbuf, px, 0);
    if (gb.w <= 0.0) {
        textureStore(dst, px, vec4f(0.0));
        return;
    }
    // this frame's sample is from one pixel of each 2x2 (gi_trace): it counts for the others on the same plane
    var at = array<vec2i, 4>(vec2i(0, 0), vec2i(1, 1), vec2i(1, 0), vec2i(0, 1));
    let s = min((px / 2) * 2 + at[u.cfg.z % 4u], size - 1);
    let ns = textureLoad(src, px / 2, 0);
    let gs = textureLoad(gbuf, s, 0);
    let cur = ns.rgb;
    let usable = ns.a > 0.0 && dot(gs.xyz, gb.xyz) > 0.9
        && abs(dot(gb.xyz, point_at(s, gs.w) - point_at(px, gb.w))) < 0.05 + 0.005 * gb.w;
    // where this point was on screen last frame
    let was = point_at(px, gb.w) + u.delta.xyz;
    let c = u.prev_vp * vec4f(was, 1.0);
    var len = 1.0;
    var out = cur;
    if (u.delta.w == 0.0 && c.w > 0.0) {
        let uv = c.xy / c.w;
        let q = vec2i(vec2f((uv.x + 1.0) * 0.5, (1.0 - uv.y) * 0.5) * vec2f(size));
        if (all(q >= vec2i(0)) && all(q < size)) {
            let pg = textureLoad(gbuf_prev, q, 0);
            let same = abs(pg.w - length(was)) < 0.04 * pg.w + 0.05 && dot(pg.xyz, gb.xyz) > 0.9;
            if (same) {
                let h = textureLoad(hist_prev, q, 0);
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
    textureStore(dst, px, vec4f(out, len));
}

@compute @workgroup_size(8, 8)
fn atrous(@builtin(global_invocation_id) id: vec3u) {
    let size = vec2i(textureDimensions(gbuf));
    let px = vec2i(id.xy);
    if (px.x >= size.x || px.y >= size.y) { return; }
    let g0 = textureLoad(gbuf, px, 0);
    let c0 = textureLoad(src, px, 0);
    if (g0.w <= 0.0) {
        textureStore(dst, px, c0);
        return;
    }
    let p0 = point_at(px, g0.w);
    let step = i32(u.cfg.y);
    var k = array<f32, 3>(0.25, 0.5, 0.25);
    var sum = vec3f(0.0);
    var wsum = 0.0;
    // a young history is noisier: it is smoothed across larger differences of brightness
    let l0 = dot(c0.rgb, vec3f(0.3, 0.5, 0.2));
    let sl = 0.15 + 2.0 / sqrt(max(c0.a, 1.0));
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let q = clamp(px + vec2i(x, y) * step, vec2i(0), size - 1);
            let gq = textureLoad(gbuf, q, 0);
            if (gq.w <= 0.0) { continue; }
            let cq = textureLoad(src, q, 0);
            let wn = pow(max(dot(g0.xyz, gq.xyz), 0.0), 32.0);
            let plane = abs(dot(g0.xyz, point_at(q, gq.w) - p0));
            let wp = exp(-plane / (0.03 + 0.004 * g0.w));
            let wl = exp(-abs(dot(cq.rgb, vec3f(0.3, 0.5, 0.2)) - l0) / (sl * (l0 + 0.02)));
            let w = k[x + 1] * k[y + 1] * wn * wp * wl;
            sum += cq.rgb * w;
            wsum += w;
        }
    }
    textureStore(dst, px, vec4f(sum / max(wsum, 1e-6), c0.a));
}

// ---- the composite: a triangle over the screen ----
@group(0) @binding(6) var direct_t: texture_2d<f32>;
@group(0) @binding(7) var albedo_t: texture_2d<f32>;
@group(0) @binding(8) var bounce_t: texture_2d<f32>;

@vertex
fn vs_composite(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4f {
    let uv = vec2f(f32((vi << 1u) & 2u), f32(vi & 2u)) * 2.0 - 1.0;
    return vec4f(uv, 0.0, 1.0);
}

@fragment
fn fs_composite(@builtin(position) pos: vec4f) -> @location(0) vec4f {
    let px = vec2i(pos.xy);
    let c = textureLoad(direct_t, px, 0).rgb + textureLoad(albedo_t, px, 0).rgb * textureLoad(bounce_t, px, 0).rgb;
    return vec4f(shoulder(c), 1.0);
}

// bright light rolls off above 0.8 instead of clipping; everything under it is left as it is
fn shoulder(c: vec3f) -> vec3f {
    let k = 0.8;
    return select(c, k + (1.0 - k) * (1.0 - exp(-(c - k) / (1.0 - k))), c > vec3f(k));
}
