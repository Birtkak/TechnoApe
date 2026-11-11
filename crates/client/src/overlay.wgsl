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
};
struct Globals { view_proj: mat4x4f, sun: vec4f, bodies: array<Body, 4> };
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
    // a line is one pixel wide: drawn four times, a pixel right, down and both, it is two (a pixel of a
    // 1080 line high screen; shortcut: not read from the real size, so a little under two on a larger one)
    o.clip += vec4f(f32(inst & 1u) / 960.0, -f32(inst >> 1u) / 540.0, 0.0, 0.0) * o.clip.w;
    o.colour = colour;
    o.along = dot(pos, vec3f(0.31, 0.37, 0.43));
    return o;
}

@fragment
fn ghost_fs(i: GhostOut) -> @location(0) vec4f {
    // the outline of what is aimed at (white from the client, play.rs) turns slowly through the colours, a
    // turn every 12 s, the hues running along its edges; a held dig stays red and a held place green
    if (i.colour.a < 0.7 && min(i.colour.r, min(i.colour.g, i.colour.b)) > 0.99) {
        let h = fract(g.sun.w / 12.0 + i.along);
        let rgb = clamp(abs(fract(vec3f(h) + vec3f(0.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0) - 1.0, vec3f(0.0), vec3f(1.0));
        return vec4f(mix(vec3f(1.0), rgb, 0.8), 0.95);
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
@fragment
fn ui_fs(i: UiOut) -> @location(0) vec4f {
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
