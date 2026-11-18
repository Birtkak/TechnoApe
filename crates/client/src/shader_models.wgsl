// The shader, part 6 of 6 (gpu.rs joins them in order): models and still models (clutter).
// ---- models (ADR 0047, crates/client/src/models.rs) ----
// One draw per part of a model, one instance per piece: the part's quads (corner x | y << 6 | z << 12 | dir << 18
// in its voxels; w | h << 6; rgba8) placed by the piece's transform straight into the camera's frame. Lit by
// `light()` where they stand, as the terrain and the effects are; not in the traced scene, so they cast no
// shadow.
// ---- still models (ADR 0068, 0081, 0082: clutter) ----
// A painted box face put in a level-0 chunk's mesh as one quad (`clutter::quad`): the cell x6 y6 z6 and its place
// in the cell u4 v4 in lo, the face's index in `sfaces` and its turn in sixteenths in hi. Drawn into the G-buffer
// as the terrain is and lit the same; not traced (no shadow).
// flags: 1 a plant's plane (ADR 0091: both sides drawn, swayed, tinted, faded far away; clutter::PLANT)
// mean: face 0's (the meadow ground's) mean colour, RGBA8
struct SFace { o: vec3f, base: u32, eu: vec3f, pw: u32, ev: vec3f, xy: u32, wh: u32, flags: u32, mean: u32, _c: u32 };
const PLANT: u32 = 1u;
// and 2 a shrub's: wide, lopsided, seen twice as far (clutter::SHRUB)
const SHRUB: u32 = 2u;

// The climate's tint of a green pixel (ADR 0091; engine clutter::tint, line for line): heat and wetness 0 to 1,
// the part of the year (0..8, `year_part`) and how much of a season the place has (0 for the grass block, whose
// paintings have theirs). Yellow-green where it is dry and warm, deep green where it is wet; a plant also fresh
// in spring, yellowing from late summer, dull in winter. Leaves can take it the same way.
fn climate_tint(heat: f32, wet: f32, part: u32, season: f32) -> vec3f {
    var shift = array<f32, 8>(-0.2, -0.1, 0.0, 0.15, 0.35, 0.5, 0.4, 0.3);
    let dry = clamp((1.0 - wet) * (0.35 + 0.65 * heat) + shift[part & 7u] * season, 0.0, 1.0);
    return mix(vec3f(0.62, 0.86, 0.56), vec3f(1.4, 0.86, 0.36), dry * dry) * (1.0 - select(0.0, 0.18 * season, part >= 6u));
}
// heat and wetness at `f` (0..1 across the chunk's footprint, x and y) from its corners' bytes: one field over
// every chunk, whose neighbours share their corners
fn chunk_climate(c: Chunk, f: vec2f) -> vec2f {
    let k = clamp(f, vec2f(0.0), vec2f(1.0));
    var v: array<vec2f, 4>;
    for (var n = 0u; n < 4u; n++) {
        let b = (c.clim >> (8u * n)) & 255u;
        v[n] = vec2f(f32(b & 15u), f32(b >> 4u)) / 15.0;
    }
    return mix(mix(v[0], v[1], k.x), mix(v[2], v[3], k.x), k.y);
}
// the part of the year (engine clutter::part): early or late half of `season`'s x
fn year_part(sn: vec3f) -> u32 {
    return u32(floor((fract(sn.x * 0.25) * 4.0 + 0.5) * 2.0)) & 7u;
}
// a painted pixel the tint takes: a green one (flowers, berries and seed heads keep their colour)
fn leafy(c: vec3f) -> bool { return c.g > c.r && c.g > c.b; }

// A plant's sway (ADR 0091): the wind `w` (camera frame, m/s) bends a point `hm` metres up by its square, in gusts
// that roll across the land with `place` (columns) and run on the sky's clock, so every client and every shot of
// the same time sees the same. Leaves can take it the same way. With `[sway]` (draft-game-reads-maps) the gusts
// are a Studio map's: its push along the wind and across it (a quarter turn about `up`, camera frame); else the
// two sines.
@group(0) @binding(19) var sway_t: texture_3d<f32>;
@group(0) @binding(21) var sway_s: sampler;
fn plant_sway(w: vec3f, hm: f32, place: vec2f, seed: f32, up: vec3f) -> vec3f {
    let speed = length(w);
    if (speed < 0.01) { return vec3f(0.0); }
    let t = g.water.x;
    let amp = min(speed * 0.025, 0.3) * min(hm * hm, 1.5);
    if (g.sway.z > 0.5) {
        // frame k's middle at k + 0.5 of the third axis, as engine `Field::at` has frame k at its loop's k-th step
        let at = vec3f(place * g.sway.x, fract(t * g.sway.y / 3600.0) + 0.5 / f32(textureDimensions(sway_t).z));
        let m = textureSampleLevel(sway_t, sway_s, at, 0.0).xy;
        let dir = w / speed;
        return amp * (dir * (0.4 + 0.8 * m.x) + normalize(cross(up, dir)) * 0.8 * m.y);
    }
    let ph = dot(place, vec2f(0.31, 0.23)) + seed * 1.5;
    // periods that divide the clock's hour, so it has no jump
    let gust = 0.4 + 0.6 * sin(t * 6.2831853 * 0.6 + ph) + 0.2 * sin(t * 6.2831853 * 1.5 + ph * 1.7);
    return w / speed * min(speed * 0.025, 0.3) * min(hm * hm, 1.5) * gust;
}
@group(0) @binding(16) var<storage, read> sfaces: array<SFace>;
@group(0) @binding(17) var<storage, read> spaint: array<u32>;

// a model direction (x, y up, z) turned by the quad's sixteenths about y, into the chunk's axes: model y is the
// chunk's z (up), model z its -y, a turn and not a mirror
fn still_dir(q: vec2u, d: vec3f) -> vec3f {
    let a = f32((q.y >> 16u) & 15u) * (6.2831853 / 16.0);
    let r = vec2f(d.x * cos(a) - d.z * sin(a), d.x * sin(a) + d.z * cos(a));
    return vec3f(r.x, -r.y, d.y);
}

// where a still quad's corner uv lies in its chunk's cells (ADR 0068, 0091): its face's model point, a plant
// sized by its column's hash and pressed into the ground by `fade` (1 whole), turned and set in its cell; a
// model's placing is repeated by hand in clutter.rs `thing_faces` (the aim at things on the ground): change both
fn still_local(q: vec2u, uv: vec2f, c: Chunk, fade: f32) -> vec3f {
    let f = sfaces[q.y & 65535u];
    let lo = q.x;
    let cell = vec3f(f32(lo & 63u), f32((lo >> 6u) & 63u), f32((lo >> 12u) & 63u));
    // its place in the cell: up to a quarter metre off the middle
    let off = (vec2f(f32((lo >> 18u) & 15u), f32((lo >> 22u) & 15u)) / 15.0 - 0.5) * 0.5;
    let c0 = vec3f(0.5, 0.0, 0.5);
    let t = plant_turn(f, c, cell);
    var m = turned(t, f.o + f.eu * uv.x + f.ev * uv.y - c0);
    var at = off;
    if ((f.flags & PLANT) != 0u) {
        m.y *= fade;
        at = t.at;
    }
    return cell + vec3f(0.5 + at.x, 0.5 + at.y, 0.0) + still_dir(q, m);
}

// a plant's turn and size (`still_local`): its height (h.x) and width (k.z) its own, turned to any angle (cos and
// sin in `cs`) and set up to a third of a block off the middle (`at`, k), so no row or grid shows from any side; a
// model's is none
struct Turn { cs: vec2f, sc: vec3f, at: vec2f };
fn plant_turn(f: SFace, c: Chunk, cell: vec3f) -> Turn {
    if ((f.flags & PLANT) == 0u) { return Turn(vec2f(1.0, 0.0), vec3f(1.0), vec2f(0.0)); }
    let col = c.base.xy + vec2i(cell.xy);
    let h = hash4(vec3i(col, i32(c.info & 7u)));
    let k = hash4(vec3i(col, i32(c.info & 7u) + 8));
    let a = k.w * 6.2831853;
    var sc = vec3f(0.85 + 0.3 * k.z, 0.8 + 0.4 * h.x, 0.85 + 0.3 * k.z);
    // a shrub: wider than tall and lopsided, one side fuller than the other
    if ((f.flags & SHRUB) != 0u) { sc = vec3f(1.1 + 0.4 * k.z, 0.7 + 0.25 * h.x, 0.85 + 0.4 * fract(k.x * 7.31)); }
    return Turn(vec2f(cos(a), sin(a)), sc, (k.xy - 0.5) * 0.66);
}
// a model direction turned about y and sized by t
fn turned(t: Turn, m: vec3f) -> vec3f {
    return vec3f(m.x * t.cs.x - m.z * t.cs.y, m.y, m.x * t.cs.y + m.z * t.cs.x) * t.sc;
}

// how far from the eye plants stand (`vs_still` fades them by then)
fn still_far() -> f32 { return g.plants.w; }

// how far one plant reaches, by its hash y: 55 to 100 % of the far distance, so the plants thin in a hashed
// order and no ring shows; a shrub twice as far, within the level-0 chunks it is meshed in (about 128 m)
fn plant_reach(flags: u32, hy: f32) -> f32 {
    // the cap first, then the hash, so shrubs too thin in a hashed order and do not all end on one ring
    let far = select(still_far(), min(still_far() * 2.0, 120.0), (flags & SHRUB) != 0u);
    return far * (0.55 + 0.45 * hy);
}

// how far from the eye still quad q of chunk-table entry ci casts a shadow: a plant until it starts to shrink
// (its own reach, as `vs_still`), a model as far as it is drawn
fn still_reach(ci: u32, q: vec2u) -> f32 {
    let fl = sfaces[q.y & 65535u].flags;
    if ((fl & PLANT) == 0u) { return 1e9; }
    let c = chunks[ci];
    let h = hash4(vec3i(c.base.xy + vec2i(i32(q.x & 63u), i32((q.x >> 6u) & 63u)), i32(c.info & 7u)));
    return plant_reach(fl, h.y) * 0.8;
}

// a shadow ray skips still quads this near its start: a few cm, or for a plant's own pixel (`fs_still`) the
// sway's reach, as its drawn blade sways off its still copy in the BLAS
var<private> still_skip: f32 = 0.04;

// the painting's texel of still quad q at a candidate hit's barycentrics bc on its triangle prim
fn still_texel(q: vec2u, prim: u32, bc: vec2f) -> vec4f {
    let f = sfaces[q.y & 65535u];
    // the corners' uv, (0,0) (1,0) (1,1) then (0,0) (1,1) (0,1), weighed by bc: written out, as an array indexed
    // at run time spills to slow memory, and this runs for every blade a shadow ray passes (ADR 0104)
    let uv = select(vec2f(bc.x, bc.x + bc.y), vec2f(bc.x + bc.y, bc.y), (prim & 1u) == 0u);
    let wh = vec2u(f.wh & 65535u, f.wh >> 16u);
    let px = min(vec2u(uv * vec2f(wh)), wh - 1u) + vec2u(f.xy & 65535u, f.xy >> 16u);
    return unpack4x8unorm(spaint[f.base + px.y * f.pw + px.x]);
}

@vertex
fn vs_still(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
    let q = quads[vi / 4u];
    let fi = q.y & 65535u;
    let f = sfaces[fi];
    var cu = array<f32, 6>(0., 1., 1., 0., 1., 0.);
    var cv = array<f32, 6>(0., 0., 1., 0., 1., 1.);
    let uv = vec2f(cu[corner_k(vi, false)], cv[corner_k(vi, false)]);
    let lo = q.x;
    let cell = vec3f(f32(lo & 63u), f32((lo >> 6u) & 63u), f32((lo >> 12u) & 63u));
    let c = chunks[ii];
    let b = g.bodies[(c.info >> 8u) & 255u];
    let plant = (f.flags & PLANT) != 0u;
    var bend = vec3f(0.0);
    var fade = 1.0;
    var h = vec4f(0.0);
    let col = c.base.xy + vec2i(cell.xy);
    if (plant) {
        // how far it reaches and its gusts' phase, from its column: the same on every client. It shrinks into
        // the ground over the last fifth of its own reach (55 to 100 % of the far one): the plants thin, none pops
        h = hash4(vec3i(col, i32(c.info & 7u)));
        let d = length(brot(b, (place(c, b, cell + vec3f(0.5, 0.5, 0.0), cell) - b.cam_hi) - b.cam_lo));
        let d0 = plant_reach(f.flags, h.y);
        fade = 1.0 - smoothstep(d0 * 0.8, d0, d);
        // past its reach it has shrunk into the ground: nothing more to place (ADR 0103)
        if (fade <= 0.0) {
            var o: VOut;
            o.clip = vec4f(0.0, 0.0, 0.0, 1.0);
            return o;
        }
    }
    let local = still_local(q, uv, c, fade);
    let gp = grid(c, local);
    let fr = frame_at(c.info & 7u, b, gp.x, gp.y);
    if (plant && ((c.info >> 8u) & 255u) == 0u) { bend = plant_sway(brot(b, g.plants.xyz), local.z - cell.z, vec2f(col), h.z, brot(b, fr.up)); }
    // a plant's plane turns, leans and stretches by its place's hash (`still_local`): its normal from its turned
    // edges (the placed corners' differences), so the sun side is the drawn plane's
    var n = still_dir(q, cross(f.ev, f.eu));
    if (plant) {
        let t = plant_turn(f, c, cell);
        n = cross(still_dir(q, turned(t, f.ev)), still_dir(q, turned(t, f.eu)));
    }
    // a plant's planes are lit as the ground under them is, from either side
    let normal = select(normalize(fr.si * n.x + fr.sj * n.y + fr.up * n.z), normalize(fr.up), plant);
    var o: VOut;
    let rel = brot(b, (place(c, b, local, cell) - b.cam_hi) - b.cam_lo) + bend;
    o.clip = g.view_proj * vec4f(rel, 1.0);
    o.rel = rel;
    // a plant's plane: how squarely it faces the sun (either side), for `fs_still`'s sun side; 1 for models
    let np = normalize(fr.si * n.x + fr.sj * n.y + fr.up * n.z);
    o.ao = select(1.0, abs(dot(brot(b, np), normalize(g.sun_pos.xyz - rel))), plant);
    // the painting's pixel, in `local.xy`; the face in `dir`; no block's look (0, air: no light of its own, no metal)
    // and in `local.z` its cell's x | y << 6, for a plant's climate
    o.local = vec3f(vec2f(f32(f.xy & 65535u), f32(f.xy >> 16u)) + uv * vec2f(f32(f.wh & 65535u), f32(f.wh >> 16u)), f32(lo & 4095u));
    o.dir = fi;
    o.mat = 0u;
    o.ci = ii;
    o.normal = brot(b, normal);
    return o;
}

// a still quad's painted texel; clear where it is not drawn: a model's plane is two faces back to back, the one
// turned away goes (the pipeline culls nothing); a plant's is one face seen from both sides
fn still_paint(i: VOut) -> vec4f {
    let f = sfaces[i.dir];
    if ((f.flags & PLANT) == 0u && dot(i.normal, i.rel) > 0.0) { return vec4f(0.0); }
    let x0 = vec2u(f.xy & 65535u, f.xy >> 16u);
    let px = clamp(vec2u(max(floor(i.local.xy), vec2f(0.0))), x0, x0 + vec2u(f.wh & 65535u, f.wh >> 16u) - 1u);
    return unpack4x8unorm(spaint[f.base + px.y * f.pw + px.x]);
}

// the still quads' depth pass (ADR 0103), as the cutout blocks' (`fs_cut_depth`): `fs_still` then shades where
// this left its depth (`Equal`), each pixel once, and the terrain under a plant is not shaded at all
@fragment
fn fs_still_depth(i: VOut) -> GOut {
    if (still_paint(i).a < 0.5) {
        discard;
    }
    return GOut(vec4f(0.0), vec4f(0.0), vec2u(0u));
}

@fragment
fn fs_still(i: VOut) -> GOut {
    let f = sfaces[i.dir];
    let plant = (f.flags & PLANT) != 0u;
    // (the back-to-back faces of a model meet the depth pass's alike: the turned-away one still goes)
    let t = still_paint(i);
    if (t.a < 0.5) {
        discard;
    }
    if (plant) {
        // its ground's tint and season, and darker to the root, as grass is in its own shade
        var col = t.rgb;
        let c = chunks[i.ci];
        let cell = u32(i.local.z + 0.5);
        let cxy = vec2f(f32(cell & 63u), f32((cell >> 6u) & 63u));
        if (leafy(col)) {
            let cl = chunk_climate(c, (cxy + 0.5) / 32.0);
            let sn = season(g.bodies[(c.info >> 8u) & 255u], i.rel);
            col *= climate_tint(cl.x, cl.y, year_part(sn), sn.y);
            // each plant a little off its neighbours: warmer or cooler, lighter or darker (its column's hash)
            let h = hash4(vec3i(c.base.xy + vec2i(cxy), i32(c.info & 7u)));
            col *= mix(vec3f(0.94, 1.0, 1.06), vec3f(1.06, 1.0, 0.92), h.w) * (0.9 + 0.2 * h.z);
        }
        // depth: dark at the root, lit at the tips; the planes of a cross a shade apart by how they are turned
        let up = 1.0 - (i.local.y - f32(f.xy >> 16u)) / f32(f.wh >> 16u);
        let turn = abs(f.eu.x) / max(length(f.eu), 1e-4);
        still_skip = 0.35;
        // the planes facing the sun lighter, those edge-on to it darker, so a tuft has a sun side
        var j = i;
        j.ao = 1.0;
        return terrain_lit(j, col * mix(0.42, 1.0, clamp(up * 1.3, 0.0, 1.0)) * mix(0.82, 1.0, turn) * mix(0.8, 1.08, i.ao), -1.0);
    }
    return terrain_lit(i, t.rgb, -1.0);
}

struct MPiece { x0: vec4f, x1: vec4f, x2: vec4f, q0: u32, body: u32, _a: u32, _b: u32 };
@group(0) @binding(12) var<storage, read> mquads: array<vec4u>;
@group(0) @binding(13) var<storage, read> mpieces: array<MPiece>;

@vertex
fn vs_model(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> FxOut {
    let m = mpieces[ii];
    let q = mquads[m.q0 + vi / 6u];
    let dir = (q.x >> 18u) & 7u;
    let a = dir / 2u;
    var ext = vec3f(1.0);
    ext[(a + 1u) % 3u] = f32(q.y & 63u);
    ext[(a + 2u) % 3u] = f32((q.y >> 6u) & 63u);
    let corner = vec3f(f32(q.x & 63u), f32((q.x >> 6u) & 63u), f32((q.x >> 12u) & 63u));
    let v = vec4f(corner + cube_corner(dir, vi % 6u) * ext, 1.0);
    let n = vec4f(axis(a) * select(1.0, -1.0, (dir & 1u) == 1u), 0.0);
    var o: FxOut;
    o.rel = vec3f(dot(m.x0, v), dot(m.x1, v), dot(m.x2, v));
    o.clip = g.view_proj * vec4f(o.rel, 1.0);
    o.normal = vec3f(dot(m.x0, n), dot(m.x1, n), dot(m.x2, n));
    o.info = m.body << 8u;
    o.b = q.z;
    // in hand (models.rs): the body it passes through, plus one (ADR 0119)
    o.a = m._a;
    return o;
}

// a contact shadow under a thing lying in a pile (ADR 0114; piles::shade): a square flat on its cell's floor,
// `size` its half width in metres, the floor's first axis in `a`; a centimetre nearer in depth only, as a cell
// look, so it passes where the floor shows and fails behind grass blades, the thing itself and walls
@vertex
fn vs_shade(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> FxOut {
    let q = fxp[ii];
    let b = g.bodies[(q.info >> 8u) & 3u];
    var cx = array<f32, 6>(-1., 1., 1., -1., 1., -1.);
    var cy = array<f32, 6>(-1., -1., 1., -1., 1., 1.);
    let k = vi % 6u;
    let up = normalize(q.p);
    let s = normalize(vec3f(f32(q.a & 1023u), f32((q.a >> 10u) & 1023u), f32((q.a >> 20u) & 1023u)) / 511.5 - 1.0);
    let t = normalize(cross(up, s));
    let pb = q.p + (s * cx[k] + t * cy[k]) * q.size;
    var o: FxOut;
    let rel = brot(b, (pb - b.cam_hi) - b.cam_lo);
    o.rel = rel;
    o.clip = g.view_proj * vec4f(rel, 1.0);
    let near = g.view_proj * vec4f(rel * max(1.0 - 0.01 / length(rel), 0.0), 1.0);
    o.clip.z = near.z / near.w * o.clip.w;
    o.normal = brot(b, up);
    o.uvw = vec3f(cx[k], cy[k], 0.0) * q.size;
    o.info = q.info;
    o.k = q.size;
    return o;
}

// the shadow's darkness, multiplied into the albedo only (gpu.rs `shade_pipeline`: the other targets are masked
// off), in rings of whole ground texels (32 a metre, as ADR 0108's shadows): half the light at its middle, none
// at its edge
@fragment
fn fs_shade(i: FxOut) -> GOut {
    let l = (floor(i.uvw.xy * 32.0) + 0.5) / 32.0;
    let r = length(l) / i.k;
    if (r >= 1.0) { discard; }
    let d = 1.0 - 0.5 * (1.0 - smoothstep(0.25, 1.0, r));
    var o: GOut;
    o.albedo = vec4f(d, d, d, 1.0);
    return o;
}

@fragment
fn fs_model(i: FxOut) -> GOut {
    if (i.a > 0u) { skip_proxy = PROXY_INST + i.a - 1u; }
    let l = light(i.rel, normalize(i.normal), (i.info >> 8u) & 3u);
    return fx_out(i, unpack4x8unorm(i.b).rgb, l.l);
}
