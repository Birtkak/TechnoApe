// The shader, part 3 of 6 (gpu.rs joins them in order): the bounce, block texels and patterns, the terrain's fragments and the G-buffer.
// ---- the bounce (ADR 0022) ----
// One cosine-weighted ray per 2x2 pixels, from one of them in turn over four frames (BOUNCE_AT); what it
// sees is that pixel's light from around, noisy.
// post.wgsl gathers it over frames and smooths it, and the composite multiplies it by the albedo.
@group(1) @binding(0) var gb_t: texture_2d<u32>;
@group(1) @binding(1) var albedo_t: texture_2d<f32>;
@group(1) @binding(2) var noisy: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8)
fn gi_trace(@builtin(global_invocation_id) id: vec3u) {
    let half = textureDimensions(noisy);
    if (id.x >= half.x || id.y >= half.y) { return; }
    let size = textureDimensions(gb_t);
    var at = array<vec2u, 4>(vec2u(0u, 0u), vec2u(1u, 1u), vec2u(1u, 0u), vec2u(0u, 1u));
    let q = min(id.xy * 2u + at[g.n_lamps.y % 4u], size - 1u);
    let ge = textureLoad(gb_t, q, 0).xy;
    let gb = vec4f(gb_normal(ge.x), bitcast<f32>(ge.y));
    if (gb.w <= 0.0) {
        textureStore(noisy, id.xy, vec4f(0.0));
        return;
    }
    let body = u32(round(textureLoad(albedo_t, q, 0).a * 3.0));
    let n = gb.xyz;
    let uv = (vec2f(q) + 0.5) / vec2f(size);
    let w = g.inv_vp * vec4f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 1.0, 1.0);
    let vd = normalize(w.xyz / w.w);
    // the ray leaves from the middle of the texel this pixel shows, with the texel's random numbers (ADR 0108):
    // the G-buffer has no texel, so it is found from the point on the body's grid and the face from the normal
    // (the sides are planes of one column through the centre, the tops spheres of one layer)
    let bb = g.bodies[body];
    let gr = body_grid(bb);
    let p0 = vd * gb.w;
    let tg = tx_grid_of(to_body(bb, p0), gr);
    let nb = n * mat3x3f(bb.rot0.xyz, bb.rot1.xyz, bb.rot2.xyz);
    let tf = tx_frame(tg.face, tg.g.xy, gr);
    let wi = abs(dot(nb, tf.si));
    let wj = abs(dot(nb, tf.sj));
    let wl = abs(dot(nb, tx_point(tg.face, tg.g, gr) / length(tx_point(tg.face, tg.g, gr))));
    var axis = 2u;
    if (wi > wj && wi > wl) { axis = 0u; } else if (wj > wl) { axis = 1u; }
    // a pixel's footprint there, longer on a face seen edge on (a column is about a metre: grid units)
    let w1 = g.inv_vp * vec4f(uv.x * 2.0 - 1.0 + 2.0 / f32(size.x), 1.0 - uv.y * 2.0, 1.0, 1.0);
    // only a block face snaps: a slope, a plant, a model or an effect (a normal off the grid's axes) keeps its pixel's
    var k = 0.0;
    if (max(wi, max(wj, wl)) > 0.99) { k = tx_k(TEXELS, gb.w * length(normalize(w1.xyz / w1.w) - vd) / max(abs(dot(n, vd)), 0.25)); }
    let gs = tx_snap(tg.g, axis, k);
    let p = p0 + brot(bb, tx_point(tg.face, gs, gr) - tx_point(tg.face, tg.g, gr));
    let f = g.n_lamps.y;
    let px = select(q, tx_seed(tg.face, gs, axis, k), k > 0.0) + vec2u(f * 7919u, f * 6271u);
    // a cosine-weighted direction around n
    let t = normalize(cross(n, select(vec3f(0., 1., 0.), vec3f(1., 0., 0.), abs(n.y) > 0.9)));
    let bt = cross(n, t);
    let u1 = hash2(px);
    let ph = 2.0 * PI * hash2(px + vec2u(4513u, 9157u));
    let r = sqrt(u1);
    let d = normalize(t * (r * cos(ph)) + bt * (r * sin(ph)) + n * sqrt(1.0 - u1));
    let o = p + n * (0.02 + length(p) * 2e-5);
    textureStore(noisy, id.xy, vec4f(seen(o, d, body, px, true, 400.0), 1.0));
}


fn hash(p: vec3i) -> f32 {
    var h = u32(p.x) * 73856093u ^ u32(p.y) * 19349663u ^ u32(p.z) * 83492791u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    return f32(h & 1023u) / 1023.0;
}

// Block look: 16 texels per 1 m block (ADR 0033) (ADR 0002, prototypes/block-look/).
// shortcut: texels are hashed here instead of read from 16x16 images, until content brings a texture atlas.
const TEXELS: f32 = 16.0;
const PLAIN: u32 = 0u;
const GRASS: u32 = 1u;
const STONE: u32 = 2u;

// The patterns with parts (ADR 0039), the port of `part` in tools/review/workbench.html: which part texel
// (x, y) of a 16 x 16 face is (0 the colour, 1 colour2, 2 colour3), the part it blends towards, how far, and a
// shade. y counts down from a side's top edge, as on the page. Whole numbers and `hash` only.
fn part(look: Look, pattern: u32, dir: u32, cell: vec3i, x: i32, y: i32) -> vec4f {
    let s = i32(look.size);
    let amt = look.amount;
    let cz = cell.x * 7 + cell.y * 13 + cell.z * 5 + i32(dir);
    let e = min(min(x, y), min(15 - x, 15 - y));
    switch (pattern) {
        case 3u: { // pebbles
            let h = hash(vec3i(x + cz * 16, y, 31));
            if (h < amt * 0.25) { return vec4f(1.0, 1.0, 0.0, 1.0); }
            if (h > 1.0 - amt * 0.1) { return vec4f(2.0, 2.0, 0.0, 1.0); }
        }
        case 4u: { // speckle
            let cx = x / s;
            let cy = y / s;
            if (hash(vec3i(cx + cz, cy, 91)) < amt) {
                let ox = cx * s + i32(floor(hash(vec3i(cx + cz, cy, 17)) * f32(s)));
                let oy = cy * s + i32(floor(hash(vec3i(cx + cz, cy, 23)) * f32(s)));
                let ax = f32(abs(x - ox));
                let ay = f32(abs(y - oy));
                let dist = max(ax, ay) + min(ax, ay) / 2.0;
                let r = f32(s) * 0.42;
                if (dist <= r * 0.4) { return vec4f(2.0, 2.0, 0.0, 1.0); }
                if (dist <= r) { return vec4f(1.0, 1.0, 0.0, 1.0); }
            }
        }
        case 5u: { // frame
            if (e < s) { return vec4f(1.0, 1.0, 0.0, 1.0); }
            let t = min(1.0, f32(e - s) / f32(max(1, 7 - s)));
            // floor(v + 0.5): the page's Math.round, not WGSL's round half to even
            return vec4f(0.0, 2.0, floor(t * amt * 4.0 + 0.5) / 4.0, 1.0);
        }
        case 6u: { // weave
            let off = i32(floor(hash(vec3i(cz, y, 5)) * f32(s)));
            if (hash(vec3i((x + off) / s + cz * 3, y, 9)) < amt) { return vec4f(1.0, 1.0, 0.0, 1.0); }
        }
        case 7u: { // planks
            let board = x / s + cz;
            if (x % s == 0 || y == i32(floor(hash(vec3i(board, 0, 3)) * 16.0))) { return vec4f(1.0, 1.0, 0.0, 1.0); }
            return vec4f(0.0, 1.0, select(0.0, 0.35, hash(vec3i(board, y, 4)) < amt * 0.3), 0.92 + 0.16 * hash(vec3i(board, 1, 4)));
        }
        case 8u: { // tiles
            if (x % s == 0 || y % s == 0) { return vec4f(1.0, 1.0, 0.0, 1.0); }
            if (hash(vec3i(x + cz * 16, y, 41)) < amt * 0.3) { return vec4f(2.0, 2.0, 0.0, 1.0); }
        }
        case 9u: { // bricks
            let h = max(2, s);
            let row = y / h;
            let xo = x + (row % 2) * h;
            if (y % h == 0 || xo % (2 * h) == 0) { return vec4f(1.0, 1.0, 0.0, 1.0); }
            return vec4f(0.0, 2.0, select(0.0, amt, y % h == 1), 0.94 + 0.12 * hash(vec3i(xo / (2 * h) + cz, row, 6)));
        }
        case 10u: { // rings
            if ((e / s) % 2 == 1) { return vec4f(1.0, 1.0, 0.0, 1.0); }
        }
        case 11u: { // leaves (their holes are `leaf_hole`'s)
            let h = hash(vec3i(x / 2 + cz * 8, y / 2, 71));
            if (h < 0.3) { return vec4f(1.0, 1.0, 0.0, 1.0); }
            if (h > 0.9) { return vec4f(2.0, 2.0, 0.0, 1.0); }
        }
        default: {}
    }
    return vec4f(0.0, 0.0, 0.0, 1.0);
}

// The season of the looks at p (from the camera, its frame) on body b, the port of engine `sky::season_at`:
// x the year's phase in quarter turns (0 spring, 1 summer, 2 autumn, 3 winter; the south half a year on), y how
// much of a season the place has, 0 near the equator to 1 from 35 degrees on (the sines of 10 and 35 degrees);
// z the column's own share of its block's stagger, -1..1, 0 until `stagger` sets it.
fn season(b: Body, p: vec3f) -> vec3f {
    return season_in(b, to_body(b, p));
}

// the same at a point of body b's own frame (`to_body`)
fn season_in(b: Body, bp: vec3f) -> vec3f {
    if (b.year < 0.0) { return vec3f(0.0); }
    // the Studio's year slider at a latitude (view.rs `season_year`): that phase, whole, everywhere
    if (b.year >= 8.0) { return vec3f(b.year - 8.0, 1.0, 0.0); }
    let z = normalize(bp).z;
    return vec3f(b.year + select(0.0, 2.0, z < 0.0), smoothstep(0.1736, 0.5736, abs(z)), 0.0);
}
// How far the column `col` (columns of level 0, so a tree keeps its season as its chunk's level changes) of
// face `face` runs ahead of or behind its place's season, -1..1: two thirds by its patch of 4 x 4 columns, a
// third its own, so a tree turns nearly as one and trees turn one by one.
fn stagger(col: vec2i, face: u32) -> f32 {
    let f = i32(face);
    return (hash(vec3i(col.x >> 2u, col.y >> 2u, 77 + f)) * 2.0 + hash(vec3i(col.x, col.y, 87 + f)) - 1.5) / 1.5;
}
// Block m's look in season sn (ADR 0061): the two nearest seasons' rows mixed by the phase, and that with its
// own look by the place's share of a season; sn.z of the block's stagger ahead or behind. A block without
// seasons, or a place without, is its row untouched.
fn look_in(m: u32, sn: vec3f) -> Look {
    var a = looks[m];
    let seasons = a.size >> 8u;
    a.size &= 255u;
    if (seasons == 0u || sn.y == 0.0) { return a; }
    let r = season_rows(seasons, sn);
    let f = r.f;
    let x = looks[r.x + m];
    let y = looks[r.y + m];
    a.colour = mix(a.colour, mix(x.colour, y.colour, f), sn.y);
    a.colour2 = mix(a.colour2, mix(x.colour2, y.colour2, f), sn.y);
    a.colour3 = mix(a.colour3, mix(x.colour3, y.colour3, f), sn.y);
    a.amount = mix(a.amount, mix(x.amount, y.amount, f), sn.y);
    a.contrast = mix(a.contrast, mix(x.contrast, y.contrast, f), sn.y);
    return a;
}

// The two nearest seasons' look rows (their offsets, 256 a season) and how far from the first to the second,
// for a block's `seasons` (`Look::size` from bit 8: its stagger from bit 16) in season sn
struct SeasonRows { x: u32, y: u32, f: f32 };
fn season_rows(seasons: u32, sn: vec3f) -> SeasonRows {
    let q = fract((sn.x + sn.z * f32(seasons >> 8u) / 1024.0) * 0.25) * 4.0;
    let i = min(u32(q), 3u);
    let j = (i + 1u) & 3u;
    // phase 0 is spring's row (1), 1 the block's own (0), then autumn's and winter's
    return SeasonRows(select(i, 1u - i, i < 2u) * 256u, select(j, 1u - j, j < 2u) * 256u, q - f32(i));
}

// look row r's paint of face `dir` (16 bits: layer | variants - 1 << 12 | painted << 15), 0 for none (a
// season's row without paint of its own has the base's, `looks_of`)
fn face_paint(r: u32, dir: u32) -> u32 {
    return (select(looks[r].tex, looks[r].tex2, dir == 5u) >> select(0u, 16u, dir < 4u)) & 65535u;
}

fn part_colour(look: Look, i: f32) -> vec3f {
    return select(select(look.colour, look.colour2, i > 0.5), look.colour3, i > 1.5);
}

// `texel_a`'s colour
fn texel(mat: u32, dir: u32, cell: vec3i, t: vec2f, hgt: f32, sn: vec3f) -> vec3f {
    return texel_a(mat, dir, cell, t, hgt, sn).rgb;
}

// a joining block's painted frame, texels wide (`lamp` in tools/paint_textures.py)
const JOIN_FRAME: i32 = 2;
const LEAVES: u32 = 11u;
fn is_glass(m: u32) -> bool { return ((looks[m & 255u].info >> 26u) & 1u) == 1u; }
// an unpainted glass block's alpha at texel (x, y): a clear middle, a rim one texel wide, so a wall reads as panes
fn glass_pane(x: i32, y: i32) -> f32 {
    return select(0.3, 0.8, x == 0 || y == 0 || x == 15 || y == 15);
}
// floor(a / b) for b > 0, as Rust's div_euclid (the Studio's `pattern::hole`)
fn fdiv(a: i32, b: i32) -> i32 { return (a - select(0, b - 1, a < 0)) / b; }
// a hole of the leaves pattern at texel ti of face dir of the block at cell (ADR 0069, studio `pattern::hole`):
// hashed by the texel's place on the face's plane, so the two faces between neighbouring leaf blocks agree
fn leaf_hole(look: Look, dir: u32, cell: vec3i, ti: vec2i) -> bool {
    // flat colours: one texel a face, never a hole (a face whose middle is a hole would be all hole colour)
    if ((look.info & 255u) != LEAVES || (g.opt.x & 4u) != 0u) { return false; }
    let a = dir / 2u;
    let s = max(i32(look.size & 255u), 1);
    let pu = cell[(a + 1u) % 3u] * 16 + ti.x;
    let pv = cell[(a + 2u) % 3u] * 16 + ti.y;
    let plane = cell[a] + select(0, 1, (dir & 1u) == 0u);
    return hash(vec3i(fdiv(pu, s), fdiv(pv, s), plane * 4 + i32(a) + 61)) < look.amount;
}

// a pixel-art texel on an N x N grid per block face: 3 tones of noise, grass tops, a grass fringe on the
// upper rows of a grass block's sides, dark crack texels in stone, the patterns with parts; a block with a
// `side` takes that block's look on its sides and foot. a: 0 for an empty texel (a painting's alpha 0, a leaves
// hole, ADR 0069), whose colour is what an opaque block or fast leaves show there: the block's colour, darker
fn texel_a(mat: u32, dir: u32, cell: vec3i, t: vec2f, hgt: f32, sn: vec3f) -> vec4f {
    let n = TEXELS;
    var ti = vec2i(floor(t * n));
    // a joining block's sides that meet its own kind (bits 16-19 of the quad, u-, u+, v-, v+; mesh.rs, ADR 0085):
    // the frame there is read from inside, so a wall of lamps glows as one surface with the frame round it.
    // shortcut: an inner corner (both sides joined, the diagonal not) loses its frame texel; fine for flat walls
    let j = (mat >> 16u) & 15u;
    if (j != 0u) {
        let lo = select(vec2i(0), vec2i(JOIN_FRAME), vec2<bool>((j & 1u) != 0u, (j & 4u) != 0u));
        let hi = select(vec2i(15), vec2i(15 - JOIN_FRAME), vec2<bool>((j & 2u) != 0u, (j & 8u) != 0u));
        ti = clamp(ti, lo, hi);
    }
    let h = hash(cell * i32(n) + vec3i(ti.x, ti.y, i32(dir)));
    let tone = 0.82 + 0.12 * floor(h * 3.0);
    let m0 = mat & 255u;
    var m = m0;
    // a painted face: its own file, else the block its side and foot take (ADR 0045)
    var painted = face_paint(m, dir);
    if (painted == 0u && (looks[m].info & 255u) == GRASS && dir != 4u) {
        // a side of a grass block takes its side block's look, with 2-4 rows of grass over the top edge
        // hashed by the texel's column along the side: t is (z, x) on a y face, so ti.x there is the row, and
        // the fringe came out as stray rows of grass down the side (bug 2026-11-11_17-48-16, ADR 0041)
        let along = select(ti.x, ti.y, dir / 2u == 1u);
        let fringe = 2.0 + floor(hash(cell * i32(n) + vec3i(along, 7, i32(dir))) * 3.0);
        if (floor(hgt * n) < n - fringe) { m = (looks[m].info >> 8u) & 255u; }
    } else if (painted == 0u && dir != 4u) {
        // any other block's side is its own id unless it names one (the mask drops metal and light)
        m = (looks[m].info >> 8u) & 255u;
    }
    painted = face_paint(m, dir);
    // on a side, x runs left to right as seen from outside and y down from its top edge (t is (y, z) on an x
    // face, (z, x) on a y face; along +y on +x and along +x on -y read rightwards, the other two are turned)
    let root = (mat >> 20u) & 7u;
    let cutout = ((looks[m0].info >> 25u) & 1u) == 1u;
    let turn = select(0u, root, cutout);
    let xy = face_xy(ti, t, dir, turn, 15);
    let x = xy.x;
    let y = xy.y;
    // the same at 32 texels a metre, for a fine painting (ADR 0090)
    let fxy = face_xy(clamp(vec2i(floor(t * 32.0)), vec2i(0), vec2i(31)), t, dir, turn, 31);
    var c: vec4f;
    if (painted != 0u) {
        c = paint_texel(painted, cell, xy, fxy, -1);
        if (c.a < 0.5 && !is_glass(m)) { c = vec4f(row_seen(m) * 0.6, 0.0); }
    } else {
        let lk = look_in(m, sn);
        if (leaf_hole(lk, dir, cell, ti)) {
            c = vec4f(lk.colour * 0.6, 0.0);
        } else {
            c = vec4f(pattern_texel(lk, dir, cell, x, y, h, tone), select(1.0, glass_pane(x, y), is_glass(m)));
        }
    }
    // a crack along an edge of the face (bits 20-23 of a rock's quad, u-, u+, v-, v+; a cutout block's are its twigs'; mesh.rs, ADR 0094): a dark line a
    // texel wide on the edge, so the two faces beside a crack draw it as one line two texels wide
    let ck = (mat >> 20u) & 15u;
    if (ck != 0u && !cutout) {
        let e = i32(n) - 1;
        if (((ck & 1u) != 0u && ti.x == 0) || ((ck & 2u) != 0u && ti.x == e) || ((ck & 4u) != 0u && ti.y == 0) || ((ck & 8u) != 0u && ti.y == e)) {
            c = vec4f(c.rgb * 0.3, c.a);
        }
    }
    // a bedding plane across a side face (bits 24-31: its texel row at the face's low end + 8, the rows it climbs to
    // the high end + 4 in the top three; `crack::bed`): neighbouring faces share their edges' rows, so the line runs
    // on along the wall and tilts with the fold
    let bd = mat >> 24u;
    if (bd != 0u && dir < 4u && !cutout) {
        let along = (f32(select(ti.x, ti.y, dir / 2u == 1u)) + 0.5) / n;
        let row = floor(f32(i32(bd & 31u) - 8) + f32(i32(bd >> 5u) - 4) * along + 0.5);
        if (floor(hgt * n) == row) {
            c = vec4f(c.rgb * 0.3, c.a);
        }
    }
    // a block a season paints (bit 9 of its size, `looks_of`): the two nearest seasons' rows, each its paint of
    // this face or else the look above, mixed as look_in mixes; a row without paint is the summer look's face,
    // so a grass side a season does not paint stays its side block's
    let size = looks[m0].size;
    if ((size & 512u) == 0u || sn.y == 0.0) { return c; }
    let r = season_rows(size >> 8u, sn);
    var cx = c;
    var cy = c;
    let px = face_paint(r.x + m0, dir);
    let py = face_paint(r.y + m0, dir);
    // a bare crown's winter painting picks its variant by the block's place in the crown (ADR 0090, part D's
    // layout): beside the wood 0 or 1, a step on 2 (a limb through to the next block), two on 3 (tips)
    let steps = (mat >> 23u) & 3u;
    let role = cutout && root != 0u;
    if (px != 0u) { cx = paint_texel(px, cell, xy, fxy, select(-1, i32(steps), role && r.x == 768u)); }
    if (py != 0u) { cy = paint_texel(py, cell, xy, fxy, select(-1, i32(steps), role && r.y == 768u)); }
    let mixed = mix(c, mix(cx, cy, r.f), sn.y);
    var a = select(select(0.0, 1.0, mixed.a >= 0.5), mixed.a, is_glass(m0));
    // a bare crown is its winter painting alone: bark-coloured twigs, no leaf tint mixed in (a crown turns bare
    // at once where winter outweighs the rest); a block with no way to wood within three steps draws no twigs
    if (cutout && bare_of(r, sn) > 0.5) {
        let w = select(cy, cx, r.x == 768u);
        return vec4f(w.rgb, select(0.0, 1.0, w.a >= 0.5 && root != 0u));
    }
    // the empty texels of the season nearest in the mix
    return vec4f(mixed.rgb, a);
}

// how much winter's bare look weighs in a season mix (`season_rows`, row 768 the winter look)
fn bare_of(r: SeasonRows, sn: vec3f) -> f32 {
    return sn.y * (select(0.0, 1.0 - r.f, r.x == 768u) + select(0.0, r.f, r.y == 768u));
}

// whether block m0 at season sn is bare (a cutout block a season paints, winter outweighing the rest)
fn is_bare(m0: u32, sn: vec3f) -> bool {
    let size = looks[m0].size;
    if ((size & 512u) == 0u || sn.y == 0.0 || ((looks[m0].info >> 25u) & 1u) == 0u) { return false; }
    return bare_of(season_rows(size >> 8u, sn), sn) > 0.5;
}

// a face's texel in its painting (`last` 15, or 31 for a fine one): on a side, x runs left to right as seen
// from outside and y down from its top edge (t is (y, z) on an x face, (z, x) on a y face; along +y on +x and
// along +x on -y read rightwards, the other two are turned). A cutout block's twigs grow from its wood (ADR 0090,
// mesh.rs `twig_bits`): its paintings have them rooted on the bottom edge, turned here so that edge lies on the
// side of the face the wood is on (`root` 1..4: u-, u+, v-, v+), or folded about the middle when the wood is
// behind the face (5); 0 turns nothing
fn face_xy(ti: vec2i, t: vec2f, dir: u32, root: u32, last: i32) -> vec2i {
    if (root == 5u) {
        return vec2i(ti.x, last - i32(floor(abs(t.y - 0.5) * f32(2 * last + 2))));
    }
    if (root != 0u) {
        let d = select(ti.y, ti.x, root < 3u);
        return vec2i(select(ti.x, ti.y, root < 3u), select(d, last - d, (root & 1u) == 1u));
    }
    var x = ti.x;
    var y = ti.y;
    if (dir < 4u) {
        let on_y = dir / 2u == 1u;
        x = select(ti.x, ti.y, on_y);
        y = last - select(ti.y, ti.x, on_y);
        if (dir == 1u || dir == 2u) { x = last - x; }
    }
    return vec2i(x, y);
}

// texel (x, y) of a painted face (`face_paint`): one of its variants by the cell's hash, the same on every face;
// a 0 for an empty pixel (a cutout block's, ADR 0069). A fine painting (bit 14, 32 x 32 a variant as four 16 x 16
// quarters in turn, ADR 0090) reads `fxy` instead. `steps` >= 0: a bare crown's winter painting of four variants
// takes its variant by the block's place in the crown (ADR 0090, part D's layout): beside the wood 0 or 1 by the
// hash, a step on 2 (a limb through), two on 3 (tips)
fn paint_texel(painted: u32, cell: vec3i, xy: vec2i, fxy: vec2i, steps: i32) -> vec4f {
    let nv = ((painted >> 12u) & 3u) + 1u;
    let hv = hash(cell + vec3i(0, 0, 4099));
    var v = min(u32(hv * f32(nv)), nv - 1u);
    if (steps > 0 && nv == 4u) { v = u32(steps) + 1u; } else if (steps == 0 && nv == 4u) { v = min(u32(hv * 2.0), 1u); }
    if (((painted >> 14u) & 1u) == 1u) {
        let f = vec2u(clamp(fxy, vec2i(0), vec2i(31)));
        let q = (f.y >> 4u) * 2u + (f.x >> 4u);
        return unpack4x8unorm(tex[((painted & 4095u) + v * 4u + q) * 256u + (f.y & 15u) * 16u + (f.x & 15u)]);
    }
    let p = vec2u(clamp(xy, vec2i(0), vec2i(15)));
    return unpack4x8unorm(tex[((painted & 4095u) + v) * 256u + p.y * 16u + p.x]);
}

// a look's pattern texel (x, y) of face `dir`; h the texel's hash, tone its noise
fn pattern_texel(look: Look, dir: u32, cell: vec3i, x: i32, y: i32, h: f32, tone: f32) -> vec3f {
    let pattern = look.info & 255u;
    // contrast 1 is exactly the look before ADR 0039
    let k = select(1.0 + look.contrast * (tone - 1.0), tone, look.contrast == 1.0);
    var c = look.colour * k;
    if (pattern == STONE && h > 0.94) { c *= 0.6; }
    if (pattern == GRASS && h > 0.9) { c *= 1.15; }
    if (pattern > STONE) {
        let q = part(look, pattern, dir, cell, x, y);
        c = mix(part_colour(look, q.x), part_colour(look, q.y), q.z) * (k * q.w);
    }
    return c;
}

// the terrain: opaque quads (`fs`), with early depth; the cutout blocks' quads of level 0 (`fs_cut`, ADR 0069),
// which leave an empty texel out. A cutout block on a coarse level is its one colour (no stretched holes), and
// through `fs` (fast leaves) an empty texel shows the block's colour, darker
@fragment
fn fs(i: VOut) -> GOut {
    return terrain_lit(i, ground_tint(i, terrain_texel(i).rgb), foot(i.rel));
}

// a grass block's top takes the climate's tint (ADR 0091), as the plants on it do: its green texels by the
// chunk's climate at the place, no season (its paintings have theirs)
fn ground_tint(i: VOut, c: vec3f) -> vec3f {
    if (i.dir > 4u || (looks[i.mat & 255u].info & 255u) != GRASS) { return c; }
    let ch = chunks[i.ci];
    let cl = chunk_climate(ch, i.local.xy / 32.0);
    let tint = climate_tint(cl.x, cl.y, 2u, 0.0);
    // where the plants have thinned out (from half their far distance) the ground takes the meadow ground's
    // painting (the textures session's three tiles, one by its column's hash): tall-grass greens, a few
    // flowers, so from a hill there is no line where plants end
    let gp = grid(ch, i.local);
    let d = length(i.rel);
    let far = smoothstep(still_far() * 0.5, still_far(), d);
    // past 150 m a pixel spans many texels: the painting fades to its mean colour, no crawling noise
    let blur = smoothstep(150.0, 300.0, d);
    if (i.dir == 4u) {
        if (!leafy(c)) { return c; }
        if (far <= 0.0) { return c * tint; }
        let m = mix(meadow_texel(gp.xy), meadow_mean(), blur);
        return mix(c * tint, m * select(vec3f(1.0), tint, leafy(m)), far);
    }
    // a side: the meadow hangs over the step's top third as a fringe, reaching further down with distance until
    // a far step (and a coarse chunk's, all of whose steps are far) is a grassy ledge, not a band of dirt, so
    // terraces seen from above read as meadow
    let low = mix(0.55, 0.0, smoothstep(still_far(), 3.0 * still_far(), d));
    let t = smoothstep(low, low + 0.3, fract(i.local.z)) * far;
    if (t <= 0.0) { return c; }
    let m = mix(meadow_texel(vec2f(gp.x + gp.y, gp.z)), meadow_mean(), blur);
    return mix(c, m * select(vec3f(1.0), tint, leafy(m)), t);
}

fn meadow_mean() -> vec3f {
    return unpack4x8unorm(sfaces[0].mean).rgb;
}

// const: face 0's flag when it is the meadow ground (engine clutter::MEADOW)
const MEADOW: u32 = 4u;
// the meadow ground's texel at grid point p (metres across, 32 px a metre), the tile picked per column
fn meadow_texel(p: vec2f) -> vec3f {
    let f = sfaces[0];
    let w = f.wh & 65535u;
    let h = f.wh >> 16u;
    let tiles = max(w / 32u, 1u);
    let col = vec2i(floor(p)) + vec2i(1 << 20);
    let tile = min(u32(hash2(vec2u(col)) * f32(tiles)), tiles - 1u);
    let px = min(vec2u(fract(p) * 32.0), vec2u(min(w, 32u) - 1u, h - 1u)) + vec2u(tile * 32u, 0u);
    // its painting's top row is the far side: y down
    let t = unpack4x8unorm(spaint[f.base + (h - 1u - min(px.y, h - 1u)) * f.pw + px.x]);
    return select(vec3f(0.3, 0.58, 0.2), t.rgb, (f.flags & MEADOW) != 0u);
}

@fragment
fn fs_cut(i: VOut) -> GOut {
    // (before the discard: the derivatives need every pixel of the quad)
    let fpx = foot(i.rel);
    let c = terrain_texel(i);
    if (c.a < 0.5) {
        discard;
    }
    return terrain_lit(i, c.rgb, fpx);
}

// the cutout blocks' depth pass (ADR 0090): the same holes as `fs_cut`, nothing written but the depth
@fragment
fn fs_cut_depth(i: VOut) -> GOut {
    if (terrain_texel(i).a < 0.5) {
        discard;
    }
    // the targets are masked off: what is returned is never written
    return GOut(vec4f(0.0), vec4f(0.0), vec2u(0u));
}

// their G-buffer after the opaque terrain, where the depth pass left theirs (`Equal`): no discard, early depth
@fragment
fn fs_cut_lit(i: VOut) -> GOut {
    return terrain_lit(i, terrain_texel(i).rgb, foot(i.rel));
}

// the fluff (`vs_fluff`, ADR 0099): the card keeps the face (its corners rounded) and, past its edges, lobes of
// leaves up to its overhang (`i.plane.z`) from a smooth field of the place about 0.3 m across, in whole texels with
// a ragged rim, so a crown's outline is bumpy leaves, not a square
fn fluff_in(i: VOut) -> bool {
    let tx = (floor(i.plane.xy * 32.0) + 0.5) / 32.0;
    let c = chunks[i.ci];
    // in the chunk's own cells (a coarse level's are larger), as `terrain_texel`, so the field has no seam at its edge
    let cell = vec3i(floor(i.local * 32.0)) + (c.base >> vec3u((c.info >> 3u) & 31u)) * 32;
    let h = hash(cell + vec3i(3, 9, 27 + i32(i.dir)));
    let out = length(max(abs(tx) - 0.38, vec2f(0.0))) - 0.12;
    let lobe = vnoise(vec3f(cell) / 32.0 * 3.3 + vec3f(f32(i.dir & 6u) * 5.1));
    return out < i.plane.z * smoothstep(0.25, 0.75, lobe) * 1.15 + 0.035 * (h - 0.5);
}

@fragment
fn fs_fluff_depth(i: VOut) -> GOut {
    if (!fluff_in(i) || terrain_texel(i).a < 0.5) {
        discard;
    }
    return GOut(vec4f(0.0), vec4f(0.0), vec2u(0u));
}

@fragment
fn fs_fluff_lit(i: VOut) -> GOut {
    return terrain_lit(i, terrain_texel(i).rgb, foot(i.rel));
}

fn terrain_texel(i: VOut) -> vec4f {
    let a = i.dir / 2u;
    let sgn = select(1.0, -1.0, (i.dir & 1u) == 1u);
    let c = chunks[i.ci];
    let cell = vec3i(floor(i.local - axis(a) * sgn * 0.5)) + (c.base >> vec3u((c.info >> 3u) & 31u)) + vec3i(i32(c.info & 7u) * 977);
    let f = fract(i.local);
    // flat colours: every pixel of a face takes its middle texel
    let t = select(vec2f(f[(a + 1u) % 3u], f[(a + 2u) % 3u]), vec2f(0.5), (g.opt.x & 4u) != 0u);
    let body = (c.info >> 8u) & 255u;
    var sn = season(g.bodies[body], i.rel);
    let level = (c.info >> 3u) & 31u;
    sn.z = stagger((vec2i(floor(i.local.xy - axis(a).xy * sgn * 0.5)) + (c.base.xy >> vec2u(level))) << vec2u(level), c.info & 7u);
    if (level > 0u && ((looks[i.mat & 255u].info >> 25u) & 1u) == 1u) { return vec4f(seen_colour(i.mat & 255u, sn), 1.0); }
    // far glass: its one colour, half clear (ADR 0081)
    if (level > 0u && is_glass(i.mat)) { return vec4f(seen_colour(i.mat & 255u, sn), 0.45); }
    let col = texel_a(i.mat, i.dir, cell, t, f.z, sn);
    // flat colours: one texel a face, never a hole; glass keeps its clearness
    return vec4f(col.rgb, select(col.a, 1.0, (g.opt.x & 4u) != 0u && !is_glass(i.mat)));
}

// a pixel's footprint on the surface (`terrain_lit`'s `fpx`): taken where every pixel of the quad is still there
fn foot(p: vec3f) -> f32 { return max(length(dpdx(p)), length(dpdy(p))); }

// `fpx` the pixel's footprint (`foot`), or below 0 for a pixel lit where it is: a still's `local` and `dir` are its
// painting's and its face's index, not the grid's
fn terrain_lit(i: VOut, col_in: vec3f, fpx: f32) -> GOut {
    let c = chunks[i.ci];
    let body = (c.info >> 8u) & 255u;
    let look = looks[i.mat & 255u];
    let sloped = i.slope.w > 0.5;
    let n = normalize(select(i.normal, i.slope.xyz, sloped));
    // where its light is worked out (ADR 0108): the middle of the painting's texel this pixel shows, found on the
    // grid from `local` (the painting's own point: a swaying crown's light rides with its texels) and moved there
    // from the pixel's point along the face (`sun_seen` and `lamp_seen` take no random numbers). A coarse level's texels are as large
    // as it draws them; far away, where a texel is smaller than a pixel, the light's are larger (`tx_k`). A slope
    // (its face is not the grid's) and a still keep the pixel's point.
    let b = g.bodies[body];
    var lit_at = i.rel;
    if (fpx >= 0.0 && !sloped) {
        let gr = body_grid(b);
        let gp = grid(c, i.local);
        let k = tx_k(TEXELS / f32(1u << ((c.info >> 3u) & 31u)), fpx);
        let gs = tx_snap(gp, i.dir / 2u, k);
        lit_at = i.rel + brot(b, tx_point(c.info & 7u, gs, gr) - tx_point(c.info & 7u, gp, gr));
    }
    var col = col_in;
    // the planet's lying snow (ADR 0076, the air grid's per column): white over what faces up, the face's own
    // pattern showing through where it lies thin
    if (body == 0u && g.cloud.w > 0.0) {
        let b0 = g.bodies[0];
        let up = normalize(i.rel - b0.ctr.xyz);
        let flat = smoothstep(0.55, 0.85, dot(n, up));
        var lie = 0.0;
        if (flat > 0.0) { lie = cl_snow(up * mat3x3f(b0.rot0.xyz, b0.rot1.xyz, b0.rot2.xyz), g.cloud) * flat; }
        if (lie > 0.0) {
            let lum = dot(col, vec3f(0.3, 0.5, 0.2));
            col = mix(col, vec3f(0.9, 0.92, 0.96), clamp(lie * 1.6 - (0.5 - lum) * 0.6, 0.0, 1.0));
        }
    }
    var o: GOut;
    o.gbuf = gb_pack(n, length(i.rel));
    if (((look.info >> 24u) & 1u) == 1u) {
        // it gives light: it shows its own, and takes none from around
        o.direct = vec4f(col * 1.15, 1.0);
        o.albedo = vec4f(0.0, 0.0, 0.0, f32(body) / 3.0);
        return o;
    }
    let px = vec2u(i.clip.xy);
    // a plant lit as one mass (ADR 0090): the direct light takes its normal bent half way toward up, so the sun
    // side is not lime and the far side not black (its volume is its shade, `ao`); the G-buffer keeps the true
    // normal for the bounce and the denoiser
    var ln = n;
    if (((look.info >> 25u) & 1u) == 1u) { ln = normalize(n + normalize(i.rel - g.bodies[body].ctr.xyz) * 0.5); }
    let l = light(lit_at, ln, body);
    var out = col * l.l;
    // its own shade darkens its sunlit side too, so a plane facing the sun keeps the painting's greens
    if (((look.info >> 25u) & 1u) == 1u) { out *= i.ao; }
    var diffuse = 1.0;
    let metal = f32((look.info >> 16u) & 255u) / 255.0;
    if (metal > 0.0 && (g.opt.x & 2u) == 0u) {
        // a metal mirrors what is around it, tinted by its colour, more so at a grazing angle
        let v = -normalize(i.rel);
        let fres = metal + (1.0 - metal) * pow(1.0 - max(dot(n, v), 0.0), 5.0);
        let r = seen(i.rel + n * (0.02 + length(i.rel) * 2e-5), reflect(-v, n), body, px, false, 4000.0);
        out = mix(out, r * mix(vec3f(1.0), col, metal), fres);
        diffuse = 1.0 - fres;
        // and the sun's glint, where the sun reaches
        out += col * g.sun_col.rgb * 3.0 * l.sun * metal * pow(max(dot(reflect(-v, n), normalize(g.sun_pos.xyz - i.rel)), 0.0), 48.0);
    }
    o.direct = vec4f(out, 1.0);
    // the corner occlusion darkens the light from around a little more, in detail the bounce smooths away
    o.albedo = vec4f(col * diffuse * mix(1.0, i.ao, 0.5), f32(body) / 3.0);
    return o;
}

// glass (ADR 0081): after the light, blended over the frame as the water is, lit by the same direct light and
// the flat light from around; the sky in it at a grazing angle, the sun's glint
@fragment
fn fs_glass(i: VOut) -> @location(0) vec4f {
    let t = terrain_texel(i);
    if (t.a < 0.01) { discard; }
    let body = (chunks[i.ci].info >> 8u) & 255u;
    let v = normalize(-i.rel);
    let n = normalize(i.normal);
    let l = light(i.rel, n, body);
    let fres = 0.04 + 0.96 * pow(1.0 - max(dot(n, v), 0.0), 5.0);
    var col = t.rgb * (l.l + ambient(l.day) + 0.1 * l.day);
    col = mix(col, g.sky.rgb + vec3f(0.17, 0.13, 0.01) * l.day, fres * 0.6);
    col += g.sun_col.rgb * 1.5 * l.sun * pow(max(dot(reflect(-g.sun.xyz, n), v), 0.0), 200.0);
    let a = clamp(t.a + fres * 0.5 * (1.0 - t.a), 0.0, 0.97);
    var lit = shoulder(col);
    if (g.opt.y != 1000u) { lit = pow(lit, vec3f(1000.0 / f32(g.opt.y))); }
    return vec4f(lit, a);
}

// the G-buffer: direct light; albedo, a the body / 3; normal (camera frame) and distance from the camera,
// 0 for the sky, packed by `gb_pack`
struct GOut {
    @location(0) direct: vec4f,
    @location(1) albedo: vec4f,
    @location(2) gbuf: vec2u,
};

// the normal folded onto an octahedron as two 16-bit numbers, and the distance's f32 bits: 8 bytes a pixel
// instead of 16 (ADR 0025, J2)
fn gb_pack(n: vec3f, d: f32) -> vec2u {
    var o = n.xy / (abs(n.x) + abs(n.y) + abs(n.z));
    if (n.z < 0.0) { o = (1.0 - abs(o.yx)) * select(vec2f(-1.0), vec2f(1.0), o >= vec2f(0.0)); }
    return vec2u(pack2x16snorm(o), bitcast<u32>(d));
}

// the G-buffer's normal, folded onto an octahedron as two 16-bit numbers (ADR 0025, J2; `gb_pack` in shader.wgsl)
fn gb_normal(e: u32) -> vec3f {
    let o = unpack2x16snorm(e);
    var n = vec3f(o, 1.0 - abs(o.x) - abs(o.y));
    let t = max(-n.z, 0.0);
    n.x += select(t, -t, n.x >= 0.0);
    n.y += select(t, -t, n.y >= 0.0);
    return normalize(n);
}

// bright light rolls off above 0.8 instead of clipping; everything under it is left as it is
fn shoulder(c: vec3f) -> vec3f {
    let k = 0.8;
    return select(c, k + (1.0 - k) * (1.0 - exp(-(c - k) / (1.0 - k))), c > vec3f(k));
}

