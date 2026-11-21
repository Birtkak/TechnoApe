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
    @location(2) glyph: u32,   // 0 solid, 1..10 the digits 0..9, 32..126 an ASCII character of FONT
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
// The globe map (ADR 0077, globe.rs): its head (the view), then Terra's and Luna's pictures (RGBA8, latitude
// rows from the north, longitude from 180 W) and their seen bits, all in `globe::off`'s order.
@group(0) @binding(20) var<storage, read> globe: array<u32>;
fn gf(k: u32) -> f32 { return bitcast<f32>(globe[k]); }

// One pixel of the globe: every px x px block takes the texel under its middle (the game's pixel look), the
// night side darker past the terminator, the unseen bare, a line every 30 degrees. globe.rs `View::pick`
// and prototypes/globe-map `shade` do the same on the CPU.
fn globe_px(frag: vec2f) -> vec4f {
    let scale = f32(g.opt.z) / gf(23u);
    let c = vec2f(gf(0u), gf(1u)) * scale;
    let r = gf(2u) * scale;
    let px = max(gf(3u) * scale, 1.0);
    let q = (floor(frag / px) + 0.5) * px;
    let p = vec2f(q.x - c.x, c.y - q.y) / r;
    let s = dot(p, p);
    if (s > 1.0) { discard; }
    let v = normalize(vec3f(gf(4u), gf(5u), gf(6u)) * p.x + vec3f(gf(8u), gf(9u), gf(10u)) * p.y + vec3f(gf(12u), gf(13u), gf(14u)) * sqrt(1.0 - s));
    let luna = gf(20u) > 0.5;
    let lat = degrees(asin(clamp(v.z, -1.0, 1.0)));
    let lon = fract((degrees(atan2(v.y, v.x) - gf(22u)) + 180.0) / 360.0) * 360.0 - 180.0;
    let w = select(1024u, 512u, luna);
    let h = w / 2u;
    let k = min(u32(max(floor((90.0 - lat) / 180.0 * f32(h)), 0.0)), h - 1u) * w + min(u32(floor((lon + 180.0) / 360.0 * f32(w))), w - 1u);
    let t = 32u + 1024u * 512u;
    let texel = unpack4x8unorm(globe[select(32u, t, luna) + k]);
    let mask = select(t + 512u * 256u, t + 512u * 256u + 1024u * 512u / 32u, luna);
    var rgb = texel.rgb;
    if (texel.a < 0.5 || (gf(21u) < 0.5 && ((globe[mask + k / 32u] >> (k % 32u)) & 1u) == 0u)) {
        rgb = vec3f(0.32, 0.30, 0.27);
    }
    // soft over about 6 degrees, the night a dim blue so the land still reads
    let day = clamp(dot(v, vec3f(gf(16u), gf(17u), gf(18u))) / 0.1 + 0.5, 0.0, 1.0);
    rgb *= vec3f(0.25, 0.28, 0.4) + vec3f(0.75, 0.72, 0.6) * day;
    // the lines a pixel block wide wherever the globe is turned
    let wide = degrees(px / r) * 0.6;
    let near = abs(fract(vec2f(lat, lon) / 30.0 + 0.5) - 0.5) * 30.0;
    if (near.x < wide || (near.y < wide / max(cos(radians(lat)), 0.05) && abs(lat) < 80.0)) {
        rgb = rgb * 0.7 + vec3f(0.26);
    }
    // a little darker to the rim, so it reads as round
    return vec4f(rgb * (0.8 + 0.2 * sqrt(1.0 - s)), 1.0);
}

@fragment
fn ui_fs(i: UiOut) -> @location(0) vec4f {
    if (i.glyph == 200u) {
        return globe_px(i.clip.xy);
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
