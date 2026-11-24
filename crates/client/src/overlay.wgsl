// Overlays drawn after the terrain (ADR 0017): the ghost of the shape a dig or place will cover, and the
// 2D layer (crosshair, hotbar, counts).
struct Body {
    cam_hi: vec3f,
    r0: f32,
    cam_lo: vec3f,
    lnq: f32,
    theta: f32,
    half_n: f32,
    _p0: f32,
    _p1: f32,
    // the body's frame turned into the camera's (ADR 0022): columns of the rotation
    rot0: vec4f,
    rot1: vec4f,
    rot2: vec4f,
    ctr: vec4f,
};
// shader.wgsl's Globals to `opt`, field for field (without `ctr` the bodies after the first were misread)
struct Globals {
    view_proj: mat4x4f,
    sun: vec4f,
    bodies: array<Body, 16>, // shader_vertex.wgsl MAX_BODIES
    inv_vp: mat4x4f,
    sun_pos: vec4f,
    sun_col: vec4f,
    moon_col: vec4f,
    sky: vec4f,
    n_lamps: vec4u,
    stars: array<vec4f, 3>,
    // z, w: the frame's width and height in pixels
    opt: vec4u,
};
@group(0) @binding(0) var<uniform> g: Globals;

// The ghost: lines along every edge of the shape's outer cells, positioned relative to their body's centre
// (f32 holds 0.1 mm at 1 km) and seen through the ground, so a dig shows what it takes.
struct GhostOut {
    @builtin(position) clip: vec4f,
    @location(0) colour: vec4f,
    // where along the shape this is, in turns of the colour wheel: the idle outline's colours run along it
    @location(1) along: f32,
};

@vertex
fn ghost_vs(@builtin(instance_index) inst: u32, @location(0) pos: vec3f, @location(1) body: u32, @location(2) colour: vec4f) -> GhostOut {
    let b = g.bodies[body];
    var o: GhostOut;
    o.clip = g.view_proj * vec4f(mat3x3f(b.rot0.xyz, b.rot1.xyz, b.rot2.xyz) * ((pos - b.cam_hi) - b.cam_lo), 1.0);
    // a line is one pixel wide: drawn nine times, a pixel apart in a 3 x 3 square, it is three (on the real
    // size: spaced for 1080 lines, a 1440 line screen showed gaps down the middle of each line, bug
    // 2026-11-11_17-38-47)
    let px = 2.0 / vec2f(f32(g.opt.z), f32(g.opt.w));
    o.clip += vec4f((f32(inst % 3u) - 1.0) * px.x, (f32(inst / 3u) - 1.0) * px.y, 0.0, 0.0) * o.clip.w;
    o.colour = colour;
    o.along = dot(pos, vec3f(0.31, 0.37, 0.43));
    return o;
}

@fragment
fn ghost_fs(i: GhostOut) -> @location(0) vec4f {
    // the outline of what is aimed at (white from the client, play.rs) is a keyboard's rainbow wave: the hues
    // run along its edges, a whole turn of the wheel every metre, and flow along them a turn every 3 s (it
    // was 12 s and paler, which read as standing still: bug 2026-11-11_17-38-47); a held dig stays red and a
    // held place green
    if (i.colour.a < 0.7 && min(i.colour.r, min(i.colour.g, i.colour.b)) > 0.99) {
        // the Accessibility page's still outline colours (bit 16, ADR 0073) keep the hues where they are
        let h = fract(i.along - select(g.sun.w / 3.0, 0.0, (g.opt.x & 65536u) != 0u));
        let rgb = clamp(abs(fract(vec3f(h) + vec3f(0.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0) - 1.0, vec3f(0.0), vec3f(1.0));
        return vec4f(mix(vec3f(1.0), rgb, 0.92), 0.95);
    }
    return i.colour;
}

// The 2D layer: rectangles in clip space, each solid or one digit of a 3 x 5 pixel font.
struct Rect {
    @location(0) xy: vec4f,    // x0, y0, x1, y1 in clip space
    @location(1) colour: vec4f,
    @location(2) glyph: u32,   // 0 solid, 1..10 the digits 0..9, 32..126 an ASCII character of FONT, 127 the degree sign
};
struct UiOut {
    @builtin(position) clip: vec4f,
    @location(0) uv: vec2f,
    @location(1) colour: vec4f,
    @location(2) @interpolate(flat) glyph: u32,
};

@vertex
fn ui_vs(@builtin(vertex_index) vi: u32, r: Rect) -> UiOut {
    var cu = array<f32, 6>(0., 1., 1., 0., 1., 0.);
    var cv = array<f32, 6>(0., 0., 1., 0., 1., 1.);
    let uv = vec2f(cu[vi], cv[vi]);
    var o: UiOut;
    o.clip = vec4f(mix(r.xy.x, r.xy.z, uv.x), mix(r.xy.y, r.xy.w, uv.y), 0.0, 1.0);
    o.uv = uv;
    o.colour = r.colour;
    o.glyph = r.glyph;
    return o;
}

// bit 14 is the top-left pixel, rows of three from the top
const DIGITS = array<u32, 10>(31599u, 11415u, 29671u, 29647u, 23497u, 31183u, 31215u, 29257u, 31727u, 31695u);

// FONT, the 8 x 8 font (ADR 0023), is appended by the client from `hud::font_wgsl`
// The globe map (ADR 0077, globe.rs): its head (the view), then every body's picture (RGBA8, latitude rows from
// the north, longitude from 180 W) and their seen bits, in `globe::layout`'s order; the head names the focused one's.
@group(0) @binding(20) var<storage, read> globe: array<u32>;
fn gf(k: u32) -> f32 { return bitcast<f32>(globe[k]); }

// The focused body's texel in direction v (the body's frame): the picture's width (twice its height) is word 25.
fn globe_k(v: vec3f) -> u32 {
    let lat = degrees(asin(clamp(v.z, -1.0, 1.0)));
    let lon = fract((degrees(atan2(v.y, v.x) - gf(22u)) + 180.0) / 360.0) * 360.0 - 180.0;
    let w = globe[25u];
    let h = w / 2u;
    return min(u32(max(floor((90.0 - lat) / 180.0 * f32(h)), 0.0)), h - 1u) * w + min(u32(floor((lon + 180.0) / 360.0 * f32(w))), w - 1u);
}
// How far out the ground in direction v stands, a share of the radius (globe.rs `lift`: the texel's top byte).
fn globe_lift(v: vec3f) -> f32 {
    let rel = gf(27u);
    return 1.0 - rel + rel * (f32(globe[globe[24u] + globe_k(v)] >> 24u) - 1.0) / 254.0;
}

// One pixel of the globe, a small copy of the body (draft "The map zooms out to the system"): every px x px block
// follows the ray under its middle (the game's pixel look) into the ground, which stands out by the body's relief
// (so mountains show at the limb), lit by the sun where it really is with a floor so the night still reads, a rim
// of sky on a body with air, the unseen bare, a line every 30 degrees. globe.rs `State::pick` (the same march)
// and `State::lift` find the same places on the CPU.
fn globe_px(frag: vec2f) -> vec4f {
    let scale = f32(g.opt.z) / gf(23u);
    let c = vec2f(gf(0u), gf(1u)) * scale;
    let r = gf(2u) * scale;
    let px = max(gf(3u) * scale, 1.0);
    let q = (floor(frag / px) + 0.5) * px;
    let p = vec2f(q.x - c.x, c.y - q.y) / r;
    let s = dot(p, p);
    if (s > 1.0) { discard; }
    let e = vec3f(gf(4u), gf(5u), gf(6u));
    let n = vec3f(gf(8u), gf(9u), gf(10u));
    let d = vec3f(gf(12u), gf(13u), gf(14u));
    let sun = vec3f(gf(16u), gf(17u), gf(18u));
    // march from where the ray enters the highest ground's sphere to where it meets the lowest's (or leaves): the
    // first step under the ground, then halved down to the surface
    let rel = gf(27u);
    let t0 = sqrt(1.0 - s);
    let inner = (1.0 - rel) * (1.0 - rel);
    let t1 = select(-t0, sqrt(max(inner - s, 0.0)), s < inner);
    var above = t0;
    var below = t0;
    var hit = false;
    // two steps a texel of the picture, so a one-texel ridge at the limb is never stepped over (globe.rs `march_steps`)
    let steps = i32(clamp(ceil((t0 - t1) * f32(globe[25u]) / 3.14159265), 8.0, 192.0));
    for (var i = 0; i <= steps; i++) {
        let t = t0 + (t1 - t0) * f32(i) / f32(steps);
        let x = e * p.x + n * p.y + d * t;
        if (length(x) <= globe_lift(normalize(x))) {
            below = t;
            hit = true;
            break;
        }
        above = t;
    }
    // (a ray that reaches the lowest ground's sphere meets it there: the sea's level, which rounding can miss)
    if (!hit && s >= inner) { discard; }
    if (!hit) { below = t1; above = t1; }
    for (var i = 0; i < 6; i++) {
        let t = 0.5 * (above + below);
        let x = e * p.x + n * p.y + d * t;
        if (length(x) <= globe_lift(normalize(x))) { below = t; } else { above = t; }
    }
    let v = normalize(e * p.x + n * p.y + d * below);
    let lat = degrees(asin(clamp(v.z, -1.0, 1.0)));
    let lon = fract((degrees(atan2(v.y, v.x) - gf(22u)) + 180.0) / 360.0) * 360.0 - 180.0;
    let k = globe_k(v);
    let texel = unpack4x8unorm(globe[globe[24u] + k]);
    let mask = globe[26u];
    var rgb = texel.rgb;
    if (gf(21u) < 0.5 && ((globe[mask + k / 32u] >> (k % 32u)) & 1u) == 0u) {
        rgb = vec3f(0.32, 0.30, 0.27);
    }
    // the ground's slope from its neighbours a texel and a half away, east-west and north-south
    var te = cross(vec3f(0.0, 0.0, 1.0), v);
    te = select(vec3f(1.0, 0.0, 0.0), normalize(te), length(te) > 1e-4);
    let tn = cross(v, te);
    let st = 1.5 * 6.2831853 / f32(globe[25u]);
    let de = (globe_lift(normalize(v + te * st)) - globe_lift(normalize(v - te * st))) / (2.0 * st);
    let dn = (globe_lift(normalize(v + tn * st)) - globe_lift(normalize(v - tn * st))) / (2.0 * st);
    // (the slopes steepened three times for the light, so a range reads at globe size; the picture's own shade from
    // the north-west stays under it)
    let nrm = normalize(v - (te * de + tn * dn) * 3.0);
    // the day side by the terminator (soft over about 6 degrees), the slopes by the sun on them; the night a dim blue
    let day = clamp(dot(v, sun) / 0.1 + 0.5, 0.0, 1.0) * (0.45 + 0.7 * clamp(dot(nrm, sun), 0.0, 1.0));
    rgb *= vec3f(0.22, 0.25, 0.36) + vec3f(0.78, 0.75, 0.64) * day;
    // a thin rim of sky where the light grazes the air
    if (gf(20u) > 0.5) {
        let rim = pow(1.0 - t0, 3.0) * clamp(dot(v, sun) * 2.0 + 0.6, 0.0, 1.0);
        rgb = mix(rgb, vec3f(0.5, 0.7, 1.0), rim * 0.7);
    }
    // the lines a pixel block wide wherever the globe is turned
    let wide = degrees(px / r) * 0.6;
    let near = abs(fract(vec2f(lat, lon) / 30.0 + 0.5) - 0.5) * 30.0;
    if (near.x < wide || (near.y < wide / max(cos(radians(lat)), 0.05) && abs(lat) < 80.0)) {
        rgb = rgb * 0.75 + vec3f(0.2);
    }
    return vec4f(rgb, 1.0);
}

@fragment
fn ui_fs(i: UiOut) -> @location(0) vec4f {
    if (i.glyph == 200u) {
        return globe_px(i.clip.xy);
    }
    // a body with no picture on the map (globe_ui.rs `GLYPH_DISC`): a ball in its colour, darker to the rim
    if (i.glyph == 201u) {
        let p = i.uv * 2.0 - 1.0;
        let s = dot(p, p);
        if (s > 1.0) { discard; }
        return vec4f(i.colour.rgb * (0.45 + 0.55 * sqrt(1.0 - s)), i.colour.a);
    }
    if (i.glyph >= 32u) {
        let x = u32(clamp(i.uv.x * 8.0, 0.0, 7.0));
        let y = u32(clamp((1.0 - i.uv.y) * 8.0, 0.0, 7.0));
        let word = FONT[(i.glyph - 32u) * 2u + y / 4u];
        if (((word >> ((y % 4u) * 8u + x)) & 1u) == 0u) { discard; }
    } else if (i.glyph > 0u) {
        let x = u32(clamp(i.uv.x * 3.0, 0.0, 2.0));
        let y = u32(clamp((1.0 - i.uv.y) * 5.0, 0.0, 4.0));
        if (((DIGITS[i.glyph - 1u] >> (14u - (y * 3u + x))) & 1u) == 0u) { discard; }
    }
    return i.colour;
}
