// The shader, part 2 of 6 (gpu.rs joins them in order): light, the light's texels, the traced rays and the lamps.
// ---- light (ADR 0022) ----
// The sun is the one source. Direct light is what the bodies' spheres leave of its disc (eclipses, and the
// horizon of the body far away) times one ray through the terrain to a point of the disc (soft shadows).
// The moon reflects it by its phase; lamps add theirs. `light` is the direct light only: the light of the
// sky and of everything around, bounced, comes from the bounce pass (`gi_trace`, post.wgsl).
const PI: f32 = 3.14159265;
const DAY_AMB: vec3f = vec3f(0.33, 0.35, 0.40);
const NIGHT_AMB: vec3f = vec3f(0.018, 0.022, 0.035);
// the night sky's own faint light (`sky_colour`)
const NIGHT_GLOW: vec3f = vec3f(0.02, 0.03, 0.07);

fn hash2(p: vec2u) -> f32 {
    var h = p.x * 73856093u ^ p.y * 19349663u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    h = h ^ (h >> 16u);
    return f32(h & 65535u) / 65535.0;
}

// the share of a disc of angular radius rs that a disc of angular radius rb, d away, covers
fn cover(rs: f32, rb: f32, d: f32) -> f32 {
    if (d >= rs + rb) { return 0.0; }
    if (d <= rb - rs) { return 1.0; }
    if (d <= rs - rb) { return (rb / rs) * (rb / rs); }
    if (rb > 20.0 * rs) {
        // a horizon: the body's edge is a straight line across the disc, x radii from its centre
        let x = clamp((d - rb) / rs, -1.0, 1.0);
        return (acos(x) - x * sqrt(1.0 - x * x)) / PI;
    }
    // two discs in the small-angle plane: the lens they share
    let l1 = rs * rs * acos(clamp((d * d + rs * rs - rb * rb) / (2.0 * d * rs), -1.0, 1.0));
    let l2 = rb * rb * acos(clamp((d * d + rb * rb - rs * rs) / (2.0 * d * rb), -1.0, 1.0));
    let k = sqrt(max((-d + rs + rb) * (d + rs - rb) * (d - rs + rb) * (d + rs + rb), 0.0)) * 0.5;
    return clamp((l1 + l2 - k) / (PI * rs * rs), 0.0, 1.0);
}

// what body i's sphere leaves of a light disc at lp of radius lr, seen from p; a point inside the sphere
// (under its nominal surface) leaves it to the terrain's rays
fn left_by(i: u32, p: vec3f, lp: vec3f, lr: f32) -> f32 {
    let bc = g.bodies[i].ctr;
    let v = bc.xyz - p;
    let dist = length(v);
    let ld = lp - p;
    if (bc.w <= 0.0 || dist <= bc.w || dist - bc.w > length(ld)) { return 1.0; }
    let rs = asin(min(lr / length(ld), 1.0));
    let rb = asin(bc.w / dist);
    return 1.0 - cover(rs, rb, atan2(length(cross(ld, v)), dot(ld, v)));
}

// where a ray from surface point p facing n starts: off the surface, more so far from the camera
fn ray_origin(p: vec3f, n: vec3f) -> vec3f {
    return p + n * (0.02 + length(p) * 2e-5);
}

// a point from the camera into body b's frame, relative to its centre; a direction d turns as `d * mat3x3f(rot)`
fn to_body(b: Body, p: vec3f) -> vec3f {
    return b.cam_hi + (b.cam_lo + p * mat3x3f(b.rot0.xyz, b.rot1.xyz, b.rot2.xyz));
}

// one ray through the terrain of body `body`: 1 if it gets out, or as far as tmax from `ray_origin`
fn ray(p: vec3f, n: vec3f, d: vec3f, body: u32, tmax: f32) -> f32 {
    let b = g.bodies[body];
    let o = to_body(b, ray_origin(p, n));
    let db = d * mat3x3f(b.rot0.xyz, b.rot1.xyz, b.rot2.xyz);
    return select(1.0, 0.0, trace(o, db, body, tmax, RAY_FLAG_TERMINATE_ON_FIRST_HIT).kind != RAY_QUERY_INTERSECTION_NONE);
}

// A ray through the terrain (ADR 0069, light option 3): the opaque geometry commits its hits by itself; a cutout
// quad (a BLAS's second geometry, not flagged opaque) is a candidate, confirmed unless its texel at the hit is
// empty, so the sun, the lamps and the bounce pass through a crown's holes. Shadow, lamp and bounce rays all
// come here. `flags`: a shadow ray takes the first hit, a ray that looks takes the nearest.
// Glass (ADR 0081) is in the same geometry and never confirmed: each glass candidate multiplies `ray_tint` by what
// its texel lets through, so light through a green pane comes out green. (shortcut: candidates come unordered, so
// a ray that looks also takes the tint of glass beyond what it hits; exact only for shadow rays)
var<private> ray_tint: vec3f;
// set while `seen` lights what a ray hit: one shadow ray to the sun and to each lamp is enough there (ADR 0108)
var<private> one_ray: bool = false;
// added to a chunk's index for its stills' instance (rt.rs STILL_INST)
const STILL_INST: u32 = 8388608u;
// added to a player's place in the list for its body's instance (rt.rs PROXY_INST, ADR 0119): shadow and lamp
// rays only; `skip_proxy` the one they pass through (a lamp's holder, the camera's own for its hand), else NONE
const PROXY_INST: u32 = 4194304u;
var<private> skip_proxy: u32 = 0xffffffffu;
fn trace(o: vec3f, db: vec3f, body: u32, tmax: f32, flags: u32) -> RayIntersection {
    var rq: ray_query;
    ray_tint = vec3f(1.0);
    // the stills are instances of their own (rt.rs): only shadow and lamp rays, which they stop, ask for them
    let mask = select(1u << body, 17u << body, (flags & RAY_FLAG_TERMINATE_ON_FIRST_HIT) != 0u);
    rayQueryInitialize(&rq, acc, RayDesc(flags, mask, 0.0, tmax, o, db));
    // the season where the ray starts: a bare crown lets it through between its twigs (ADR 0090)
    let sn = season_in(g.bodies[body], o);
    while (rayQueryProceed(&rq)) {
        let c = rayQueryGetCandidateIntersection(&rq);
        if (c.instance_custom_data >= PROXY_INST && c.instance_custom_data < STILL_INST) {
            if (c.instance_custom_data != skip_proxy) { rayQueryConfirmIntersection(&rq); }
            continue;
        }
        let q = hit_quad(c.instance_custom_data, c.geometry_index, c.primitive_index);
        if (((q.x >> 28u) & 7u) == 7u) {
            // a still model or plant (ADR 0091) shades shadow and lamp rays only: a plant only while it stands
            // whole (`still_reach`), by its painting, and past `still_skip` from the ray's start, so a swaying
            // blade does not shade itself
            let b = g.bodies[body];
            // (nearer than 0.44 of the far distance every plant reaches, `plant_reach` times 0.8, a shrub's too
            // while still_far() <= 120 (frame.rs PLANTS_FAR): its hash is not needed)
            let e = length((o + db * c.t - b.cam_hi) - b.cam_lo);
            if ((flags & RAY_FLAG_TERMINATE_ON_FIRST_HIT) != 0u && c.t > still_skip
                && (e < 0.44 * still_far() || e < still_reach(c.instance_custom_data & (STILL_INST - 1u), q))
                && still_texel(q, c.primitive_index, c.barycentrics).a >= 0.5) {
                rayQueryConfirmIntersection(&rq);
            }
        } else if (is_glass(q.y)) {
            // far glass is drawn in its one colour, half clear: its shadow the same
            var t = vec4f(seen_colour(q.y & 255u, vec3f(0.0)), 0.45);
            if (((chunks[c.instance_custom_data].info >> 3u) & 31u) == 0u) { t = hit_texel(c.instance_custom_data, q, c.primitive_index, c.barycentrics, sn); }
            ray_tint *= mix(vec3f(1.0), t.rgb, t.a);
        } else if (!hit_empty(c.instance_custom_data, q, c.primitive_index, c.barycentrics, hit_season(c.instance_custom_data, q, sn))) {
            // a shadow ray through a face of fast leaves takes its share of the light (`fluff.light`, the block row)
            // and goes on: a soft green shade under a crown, not a whole block's (ADR 0090); nearly none left stops it.
            // shortcut: a fast crown is a shell, so any crown passes light squared whatever its thickness; weigh by
            // the distance from the entry face if thin and thick crowns should differ
            if (flags == RAY_FLAG_TERMINATE_ON_FIRST_HIT && ((chunks[c.instance_custom_data].info >> 18u) & 1u) == 1u) {
                ray_tint *= looks[q.y & 255u].through * vec3f(0.9, 1.0, 0.75);
                if (max(ray_tint.r, max(ray_tint.g, ray_tint.b)) < 0.02) { rayQueryConfirmIntersection(&rq); }
            } else {
                rayQueryConfirmIntersection(&rq);
            }
        }
    }
    return rayQueryGetCommittedIntersection(&rq);
}

// the quad a ray hit: instance id (a chunk-table entry, plus STILL_INST for its stills' instance), geometry
// geo (0 the opaque quads, 1 the cutout ones), triangle prim
fn hit_quad(id: u32, geo: u32, prim: u32) -> vec2u {
    let c = chunks[id & (STILL_INST - 1u)];
    if (id >= STILL_INST) { return quads[c.off + c.sto + prim / 2u]; }
    return quads[c.off + select(0u, c.opq, geo == 1u) + prim / 2u];
}

// whether a candidate hit at barycentrics bc is on an empty texel of its cutout block (level 0 only meshes them
// so), in season sn. Fast leaves (the chunk's info bit 18, frame.rs) are whole blocks to a ray (ADR 0069) unless
// bare: then the ray takes the twigs' painting, so light falls through a winter crown and its twigs are lit like
// the bark (ADR 0090); fancy ones take the season's texel
fn hit_empty(ci: u32, q: vec2u, prim: u32, bc: vec2f, sn: vec3f) -> bool {
    // flat colours draw a cutout block whole: the light takes it whole too
    if ((g.opt.x & 4u) != 0u) { return false; }
    if (((chunks[ci].info >> 18u) & 1u) == 1u && !is_bare(q.y & 255u, sn)) { return false; }
    return hit_texel(ci, q, prim, bc, sn).a < 0.5;
}

// season sn at quad q of chunk-table entry ci of level 0: with its column's stagger, as `terrain_texel` draws it
// (a cutout quad never merges, so its corner names its cell)
fn hit_season(ci: u32, q: vec2u, sn: vec3f) -> vec3f {
    let c = chunks[ci];
    let a = ((q.x >> 28u) & 7u) / 2u;
    let sgn = select(1.0, -1.0, ((q.x >> 28u) & 1u) == 1u);
    let at = quad_corner(q, 0u, 0.0);
    return vec3f(sn.xy, stagger(vec2i(floor(at.xy - axis(a).xy * sgn * 0.5)) + c.base.xy, c.info & 7u));
}

// the texel of quad q of chunk-table entry ci at a candidate hit's barycentrics bc on its triangle prim
fn hit_texel(ci: u32, q: vec2u, prim: u32, bc: vec2f, sn: vec3f) -> vec4f {
    let c = chunks[ci];
    let tri = (prim & 1u) * 3u;
    // the BLAS holds the quads grown by GROW; the same barycentrics on the ungrown corners map a hit in the lip
    // onto the quad's edge texel, as `vs` keeps `local` ungrown (a grown lip's fract wraps to the far edge)
    var local = quad_corner(q, tri, 0.0) * (1.0 - bc.x - bc.y) + quad_corner(q, tri + 1u, 0.0) * bc.x + quad_corner(q, tri + 2u, 0.0) * bc.y;
    let dir = (q.x >> 28u) & 7u;
    // a slope's painting lies flat, as `vs` lays it (ADR 0101)
    if ((q.x >> 31u) == 1u && dir == 4u) { local.z = f32((q.x >> 12u) & 63u); }
    let a = dir / 2u;
    let sgn = select(1.0, -1.0, (dir & 1u) == 1u);
    let cell = vec3i(floor(local - axis(a) * sgn * 0.5)) + c.base + vec3i(i32(c.info & 7u) * 977);
    let f = fract(local);
    return texel_a(quad_mat(q), dir, cell, vec2f(f[(a + 1u) % 3u], f[(a + 2u) % 3u]), f.z, sn);
}

// what the terrain leaves to p of the sun's disc, angular radius ar around direction sd (ADR 0108): rays to two
// opposite points of the disc, and two across them where those two disagree, so a penumbra is flat steps of a
// quarter with no noise in it (it was one ray a pixel to a dithered point, bug 2026-11-14_01-59-35); one ray to
// its middle with sharp shadows, and at a bounce's hit (`one_ray`). `ray_tint` is left the mean tint of the rays
// that got out. The pairs lie on the disc's diagonals, in a frame of the sun's path over this place (across it and
// along it, from the local up), so no edge's way is favoured and nothing flips as the sun goes over.
fn sun_seen(p: vec3f, n: vec3f, sd: vec3f, ar: f32, body: u32) -> f32 {
    if ((g.opt.x & 1u) != 0u || one_ray) { return ray(p, n, sd, body, 1e5); }
    let up = normalize(p - g.bodies[body].ctr.xyz);
    let across = cross(sd, up);
    // (the sun straight overhead: any way across)
    let t = normalize(select(across, cross(sd, select(vec3f(0., 1., 0.), vec3f(1., 0., 0.), abs(sd.y) > 0.9)), dot(across, across) < 1e-6));
    let bt = cross(sd, t);
    // (0.7 of the radius: about the middles of the disc's quarters by area)
    let r = ar * 0.7 * 0.70710678;
    var v = 0.0;
    var k = 0.0;
    var tint = vec3f(0.0);
    for (var i = 0u; i < 4u; i++) {
        if (i == 2u && (v == 0.0 || v == 2.0)) { break; }
        let s = ray(p, n, normalize(sd + (t + bt * select(1.0, -1.0, i >= 2u)) * select(r, -r, (i & 1u) == 1u)), body, 1e5);
        v += s;
        k += 1.0;
        tint += ray_tint * s;
    }
    ray_tint = select(vec3f(1.0), tint / v, v > 0.0);
    return v / k;
}

// ---- the light's texels (ADR 0108) ----
// A block face's light is worked out once a texel of its painting: shadow, lamp and bounce rays leave from the
// texel's middle and take their random numbers from the texel, so every pixel of one texel has one light, a
// shadow's edge runs along texel borders and no pixel's grain is left (bug 2026-11-14_01-59-35). Free of the
// globals: gi.rs's test runs this part alone on the GPU.

// a cube face's normal and in-face axes (frame_at's), by bits as clouds.wgsl's `cl_face`: s 1 2 2 0 0 1, t 2 1 0 2 1 0
fn tx_axis(a: u32) -> vec3f { return vec3f(f32(a == 0u), f32(a == 1u), f32(a == 2u)); }
fn tx_n(f: u32) -> vec3f { return tx_axis(f >> 1u) * select(1.0, -1.0, (f & 1u) == 1u); }
fn tx_s(f: u32) -> vec3f { return tx_axis((1065u >> (2u * f)) & 3u); }
fn tx_t(f: u32) -> vec3f { return tx_axis((390u >> (2u * f)) & 3u); }

// a body's grid: an angle a column, half the columns a face edge, layer 0's radius, the log of a layer's ratio, and
// the block grid's warp towards the conformal grid (ADR 0131; 0 the equi-angular grid of ADR 0003, to the bit)
struct TxGrid { theta: f32, half_n: f32, r0: f32, lnq: f32, warp: f32 };

// the warp's table (engine warp.rs, the same bytes): 193 x 193 texels of (da, db), x along b and y along a. Bilinear by
// hand (rg32float is not filterable), on |a| and |b| with the signs put back as the engine does, so the two faces of a
// seam find its points from the same texels. `ab` the face point in -1..1; the warped point and its derivatives
@group(0) @binding(18) var warp_t: texture_2d<f32>;
struct Warp { ab: vec2f, da: vec2f, db: vec2f };
fn warp_at(ab: vec2f, w: f32) -> Warp {
    let sg = select(vec2f(1.0), vec2f(-1.0), ab < vec2f(0.0));
    let x = (min(abs(ab), vec2f(1.0)) + 1.0) * 96.0;
    let i0 = min(vec2i(floor(x)), vec2i(191));
    let fr = x - vec2f(i0);
    let t00 = textureLoad(warp_t, vec2i(i0.y, i0.x), 0).xy;
    let t10 = textureLoad(warp_t, vec2i(i0.y, i0.x + 1), 0).xy;
    let t01 = textureLoad(warp_t, vec2i(i0.y + 1, i0.x), 0).xy;
    let t11 = textureLoad(warp_t, vec2i(i0.y + 1, i0.x + 1), 0).xy;
    let v = mix(mix(t00, t10, fr.x), mix(t01, t11, fr.x), fr.y);
    // (constant past the face's edge)
    let k = select(vec2f(96.0), vec2f(0.0), abs(ab) >= vec2f(1.0));
    let dva = mix(t10 - t00, t11 - t01, fr.y) * k.x;
    let dvb = mix(t01 - t00, t11 - t10, fr.x) * k.y;
    let s = sg.x * sg.y;
    return Warp(ab + w * sg * v, vec2f(1.0, 0.0) + w * vec2f(dva.x, s * dva.y), vec2f(0.0, 1.0) + w * vec2f(s * dvb.x, dvb.y));
}

// the two tangents of grid point g's direction on its cube face
fn tx_tan(g: vec2f, gr: TxGrid) -> vec2f {
    if (gr.warp == 0.0) { return tan((g - gr.half_n) * gr.theta); }
    return tan(warp_at((g - gr.half_n) / gr.half_n, gr.warp).ab * 0.78539816);
}

// the warp undone: the grid point whose direction the equi-angular grid has at `ij` (ADR 0131). One fixed-point step,
// then Newton on the bilinear cell: three steps leave under 0.005 of a column anywhere on the face, a twelfth of a
// light texel (engine warp.rs `the_shaders_three_newton_steps_suffice`; two leave 1.4 columns near a corner)
const WARP_STEPS: u32 = 3u;
fn tx_unwarp(ij: vec2f, gr: TxGrid) -> vec2f {
    let t = (ij - gr.half_n) / gr.half_n;
    var x = 2.0 * t - warp_at(t, gr.warp).ab;
    for (var k = 0u; k < WARP_STEPS; k++) {
        let wp = warp_at(x, gr.warp);
        let r = wp.ab - t;
        let det = wp.da.x * wp.db.y - wp.db.x * wp.da.y;
        x -= vec2f(wp.db.y * r.x - wp.db.x * r.y, wp.da.x * r.y - wp.da.y * r.x) / det;
    }
    return x * gr.half_n + gr.half_n;
}

// grid point g's direction, its columns' two sides' normals (each pointing on along its column index: a side is the
// surface through the centre along the grid's other line) and its tangents; with no warp the planes of ADR 0003
struct TxFrame { up: vec3f, si: vec3f, sj: vec3f, ta: f32, tb: f32 };
fn tx_frame(f: u32, g: vec2f, gr: TxGrid) -> TxFrame {
    let n = tx_n(f);
    let s = tx_s(f);
    let t = tx_t(f);
    if (gr.warp == 0.0) {
        let ta = tan((g - gr.half_n) * gr.theta);
        return TxFrame(normalize(n + s * ta.x + t * ta.y), normalize(s - n * ta.x), normalize(t - n * ta.y), ta.x, ta.y);
    }
    let wp = warp_at((g - gr.half_n) / gr.half_n, gr.warp);
    let ta = tan(wp.ab * 0.78539816);
    let p = n + s * ta.x + t * ta.y;
    // the cube plane's point moved a column along i and along j (common factors left out)
    let sec = 1.0 + ta * ta;
    let pi = s * (sec.x * wp.da.x) + t * (sec.y * wp.da.y);
    let pj = s * (sec.x * wp.db.x) + t * (sec.y * wp.db.y);
    let ni = normalize(cross(p, pj));
    let nj = normalize(cross(pi, p));
    return TxFrame(normalize(p), ni * select(-1.0, 1.0, dot(ni, pi) >= 0.0), nj * select(-1.0, 1.0, dot(nj, pj) >= 0.0), ta.x, ta.y);
}

// body-frame point q as face f's grid coordinates: column i, column j, layer (q need not be over face f)
fn tx_grid_on(q: vec3f, f: u32, gr: TxGrid) -> vec3f {
    let dn = dot(q, tx_n(f));
    var ij = atan(vec2f(dot(q, tx_s(f)), dot(q, tx_t(f))) / dn) / gr.theta + gr.half_n;
    if (gr.warp != 0.0) { ij = tx_unwarp(ij, gr); }
    return vec3f(ij, log(length(q) / gr.r0) / gr.lnq);
}

// body-frame point q (from the body's centre) as its face and grid coordinates: column i, column j, layer
struct TxAt { face: u32, g: vec3f };
fn tx_grid_of(q: vec3f, gr: TxGrid) -> TxAt {
    let a = abs(q);
    var f = 4u + select(0u, 1u, q.z < 0.0);
    if (a.x >= a.y && a.x >= a.z) { f = select(0u, 1u, q.x < 0.0); } else if (a.y >= a.z) { f = 2u + select(0u, 1u, q.y < 0.0); }
    return TxAt(f, tx_grid_on(q, f, gr));
}

// grid point g of face f in the body's frame (the cube-sphere's own, as `frame_at` and `place` lay it)
fn tx_point(f: u32, g: vec3f, gr: TxGrid) -> vec3f {
    let ta = tx_tan(g.xy, gr);
    return normalize(tx_n(f) + tx_s(f) * ta.x + tx_t(f) * ta.y) * (gr.r0 * exp(g.z * gr.lnq));
}

// grid point g moved to the middle of its texel, k texels a grid unit, along the two axes other than `axis` (the
// face's normal: 0 column i, 1 column j, 2 the layer), which stays as it is; k 0 leaves the point
fn tx_snap(g: vec3f, axis: u32, k: f32) -> vec3f {
    if (k <= 0.0) { return g; }
    return select((floor(g * k) + 0.5) / k, g, vec3<bool>(axis == 0u, axis == 1u, axis == 2u));
}

// the random numbers' seed of the texel whose middle is gs: its place, its cube face and its normal's axis
fn tx_seed(f: u32, gs: vec3f, axis: u32, k: f32) -> vec2u {
    var c = vec3i(floor(gs * k));
    // (the normal's axis: the face's plane. The two faces of one plane, looking either way, share a seed: only one
    // of them is ever drawn, since a face is there only between a block and an open cell)
    c[axis] = i32(round(gs[axis])) * 8 + i32(axis) + 3 * i32(k);
    var v = vec3u(c) * 1664525u + vec3u(1013904223u + f * 977u);
    v.x += v.y * v.z; v.y += v.z * v.x; v.z += v.x * v.y;
    v ^= v >> vec3u(16u);
    v.x += v.y * v.z; v.y += v.z * v.x;
    return v.xy;
}

// the texels a grid unit of the light at a place whose painting has `k0` a grid unit and where one screen pixel
// spans `px` grid units of the surface: the painting's while a texel spans a pixel or more; past that, halved
// until one does again, so far away the light is still one a texel of a coarser grid of whole texels and never
// finer than the pixels, which would only alias and shimmer as the camera moves. Never larger than a block (nor a
// coarse level's painted texel): a light texel of many blocks would put its point in a terrace step or the air,
// so past that it is 0, the pixel's own point (`tx_snap` leaves it)
fn tx_k(k0: f32, px: f32) -> f32 {
    let e = max(ceil(log2(max(px * k0, 1e-6))), 0.0);
    return select(k0 / exp2(e), 0.0, e > max(log2(k0), 0.0));
}
// ---- end of the light's texels ----

struct Light { l: vec3f, sun: f32, day: f32 };

// the sky's light over point p of body `body`, for bounce rays that leave: blue by day where the body has
// air (a sea), fading through twilight; no sun's disc
const DAY_SKY: vec3f = vec3f(0.32, 0.46, 0.66);
fn sky_light(p: vec3f, body: u32) -> vec3f {
    let b = g.bodies[body];
    let day = smoothstep(-0.2, 0.15, dot(normalize(p - b.ctr.xyz), normalize(g.sun_pos.xyz - p)));
    return DAY_SKY * day * select(0.0, 1.0, b.sea >= 0.0) + NIGHT_AMB;
}

// the share of the sun's direct light the air lets through near the horizon, where the body has air (a sea):
// it fades over the last few degrees, so the line where the sun sets behind the body's sphere is a gradient
// seen from far away, not a hard edge (ADR 0038 C); 1 on a body without air
fn sunset(up: vec3f, sd: vec3f, body: u32) -> f32 {
    return select(1.0, smoothstep(-0.03, 0.12, dot(up, sd)), g.bodies[body].sea >= 0.0);
}

// a flat stand-in for the light from around, where no bounce ray is traced: water, and what rays hit
fn ambient(day: f32) -> vec3f {
    return mix(NIGHT_AMB, DAY_AMB, day);
}

// the light on a surface of body `body` at p (from the camera) with normal n
const LAMP_R: f32 = 24.0;
const LAMP_RAYS: u32 = 3u;
// a shadow ray goes to the lamp's block, not its middle (`lamp_target`), and stops this far short of it, so
// the lamp never shadows itself
const LAMP_STOP: f32 = 0.01;
const NONE: u32 = 0xffffffffu;

// a lamp's light d2 square metres away: falling off with the square of the distance from 2 m, to nothing at
// LAMP_R. It was 1.5 / (1 + d2) to 8 m, a seventh at 3 m and nothing past 6; the developer asked for "much more
// light" (bug 2026-11-11_17-43-39, ADR 0041): now 7 times that at 3 m, 20 at 6, a glow to 10. Then twice the
// reach (ADR 0072): about as bright within 3 m (stretching the whole curve washed a lamp's neighbours out by
// day), twice as bright at 6 m, a tenth at 10 m and a glow to 20, so one lamp lights a 15 to 20 m room
fn lamp_light(lp: Lamp, d2: f32) -> vec3f {
    let x = 1.0 - d2 / (LAMP_R * LAMP_R);
    return lp.col.rgb * x * x * 2.0 / (1.0 + 0.1 * d2);
}

// the sunlight on body `body`: through the air as the camera sees the sun on its own body; white on
// another, which has its own sky (bug 2026-11-11_13-17-26: the moon took the camera's sunset red every night)
fn sun_on(body: u32) -> vec3f {
    return select(vec3f(0.65), g.sun_col.rgb, body == u32(g.sun_col.w));
}
// the sunlight left in a planet's full shadow, bent round it through its air: dim copper, even over the moon
const COPPER: vec3f = vec3f(0.65, 0.2, 0.06) * 0.35;
// how much brighter the moon is drawn in the night sky of a body with air (ADR 0070, the developer's to tune):
// the eye adapts to the night, and the moon, lit by the full sun, is far the brightest thing in it; drawn at
// the ground's exposure it read grey. 1 draws it as it is.
const MOON_NIGHT_GAIN: f32 = 2.6;
// the gain on the sunlight on `body` as the camera sees it: MOON_NIGHT_GAIN on the moon from inside another
// body's air at night, faded in with the dusk; 1 elsewhere (by day, from space, standing on the moon)
fn moon_gain(body: u32) -> f32 {
    if (body != u32(g.moon_col.w) || body == u32(g.sun_col.w)) { return 1.0; }
    let night = 1.0 - smoothstep(-0.1, 0.05, g.stars[1].w);
    return mix(1.0, MOON_NIGHT_GAIN, night * g.stars[0].w);
}

// where a shadow ray from o aims at lamp lp: the point of its block nearest o, 2 cm out from the faces o is
// beyond (out along the others too, or in, would put a flat ray's end inside the blocks beside it). Aimed at
// the middle, the ray to a lamp set in a floor or wall passed through the blocks beside it wherever it came
// in flatter than 30 degrees, and the lamp's light ended there in a hard cone over the walls; a lamp on the
// ground cast spikes from its own corners (bugs 2026-11-11_13-00-53, 2026-11-10_19-45-23; ADR 0037).
// Worked in the grid's own coordinates (the face's two angles and the log of the radius, its face the up's
// largest axis), where the block is a box: o clamped into the lamp's cell and turned back. Moved by j (-1 to 1 an
// axis, in parts of the box) before the clamp (ADR 0072), so the rays end on points spread over the sides o sees:
// the lamp is a 1 m light, not a point, and the shadows it casts are soft (`lamp_seen`, ADR 0108).
fn lamp_target(lp: Lamp, o: vec3f, j: vec3f) -> vec3f {
    let b = g.bodies[u32(lp.pos.w)];
    let m = mat3x3f(b.rot0.xyz, b.rot1.xyz, b.rot2.xyz);
    let up = normalize(lp.pos.xyz - b.ctr.xyz);
    let ub = up * m;
    let a = abs(ub);
    var axis = 2u;
    if (a.x >= a.y && a.x >= a.z) { axis = 0u; } else if (a.y >= a.z) { axis = 1u; }
    let f = frame_at(axis * 2u + select(0u, 1u, ub[axis] < 0.0), b, 0.0, 0.0);
    let r = length(lp.pos.xyz - b.ctr.xyz);
    if (b.rot0.w != 0.0) {
        // the warped grid (ADR 0131): its angles are not the block's box, its own coordinates are
        let fc = axis * 2u + select(0u, 1u, ub[axis] < 0.0);
        let gr = body_grid(b);
        let ga = tx_grid_on(ub * r, fc, gr);
        let gq = tx_grid_on((o - b.ctr.xyz) * m, fc, gr);
        // half a cell each way and 2 cm (at a column's mean width: near a cube corner, where they are a third of
        // that, rather less)
        let gh = vec3f(0.5 + 0.02 / (r * b.theta), 0.5 + 0.02 / (r * b.theta), 0.5 + 0.02 / (r * b.lnq));
        let gin = vec3f(0.45);
        let gc = select(clamp(gq, ga - gh, ga + gh), clamp(gq + j * gin, ga - gin, ga + gin), abs(gq - ga) < gin);
        return m * tx_point(fc, gc, gr) + b.ctr.xyz;
    }
    let at = vec3f(atan(dot(ub, f.s) / dot(ub, f.n)), atan(dot(ub, f.t) / dot(ub, f.n)), log(r));
    let ob = (o - b.ctr.xyz) * m;
    let ro = length(ob);
    let q = vec3f(atan(dot(ob, f.s) / dot(ob, f.n)), atan(dot(ob, f.t) / dot(ob, f.n)), log(ro));
    // half a cell each way (an angle's metres are about the radius times it), and 2 cm
    let half = vec3f(0.5 * b.theta, 0.5 * b.theta, 0.5 * b.lnq) + 0.02 / r;
    // the jitter only along the axes o lies within the box on, inset 5 cm: along the others the end stays 2 cm out
    // from the face o sees, and a ray from the floor beside a lamp never ends under that floor
    let inner = vec3f(0.45 * b.theta, 0.45 * b.theta, 0.45 * b.lnq);
    let c = select(clamp(q, at - half, at + half), clamp(q + j * inner, at - inner, at + inner), abs(q - at) < inner);
    let tb = normalize(f.n + f.s * tan(c.x) + f.t * tan(c.y)) * exp(c.z);
    return m * tb + b.ctr.xyz;
}

// what the terrain leaves to p of lamp lp, 0 to 1, as `sun_seen` the sun: rays to two opposite points of the
// sides p sees, two more across them where those disagree; flat steps of a quarter, no noise (ADR 0108; it was one
// ray a pixel to a random point). `ray_tint` is left the mean tint of the rays that got out. The jitter moves the
// end only along the axes p lies within the lamp's box on, so ends can fall together: a point already cast takes
// that ray's answer, not a ray of its own. At a bounce's hit (`one_ray`) one ray, to the nearest point.
fn lamp_seen(p: vec3f, n: vec3f, lp: Lamp, body: u32) -> f32 {
    let ro = ray_origin(p, n);
    var js = array<vec3f, 4>(vec3f(0.6), vec3f(-0.6), vec3f(0.6, -0.6, 0.0), vec3f(-0.6, 0.6, 0.0));
    if (one_ray) { js[0] = vec3f(0.0); }
    var ends = array<vec3f, 4>(vec3f(0.0), vec3f(0.0), vec3f(0.0), vec3f(0.0));
    var got = array<f32, 4>(0.0, 0.0, 0.0, 0.0);
    var tints = array<vec3f, 4>(vec3f(0.0), vec3f(0.0), vec3f(0.0), vec3f(0.0));
    var v = 0.0;
    var k = 0.0;
    var tint = vec3f(0.0);
    for (var i = 0u; i < 4u; i++) {
        if ((i == 1u && one_ray) || (i == 2u && (v == 0.0 || v == 2.0))) { break; }
        // aimed from where the ray starts: from p, the 2 cm offset carried its end into the lamp by its corners
        // (bug 2026-11-10_19-45-23)
        ends[i] = lamp_target(lp, ro, js[i]);
        var s = 1.0;
        ray_tint = vec3f(1.0);
        var fresh = true;
        for (var m = 0u; m < i; m++) {
            if (fresh && all(ends[m] == ends[i])) { s = got[m]; ray_tint = tints[m]; fresh = false; }
        }
        let w = ends[i] - ro;
        let d = length(w);
        if (fresh && d >= LAMP_STOP) { s = ray(p, n, w / d, body, d - LAMP_STOP); }
        got[i] = s;
        tints[i] = ray_tint;
        v += s;
        k += 1.0;
        tint += ray_tint * s;
    }
    ray_tint = select(vec3f(1.0), tint / v, v > 0.0);
    return v / k;
}

fn light(p: vec3f, n: vec3f, body: u32) -> Light {
    let up = normalize(p - g.bodies[body].ctr.xyz);
    let sd = normalize(g.sun_pos.xyz - p);
    // eclipses: every other body's sphere over the sun's disc; what a body with air hides of it comes round
    // its rim bent and reddened (a lunar eclipse: the whole moon copper, ADR 0037)
    var ecl = 1.0;
    var copper = 0.0;
    for (var i = 0u; i < 4u; i++) {
        if (i != body && ((g.n_lamps.z >> i) & 1u) == 1u) {
            let l = left_by(i, p, g.sun_pos.xyz, g.sun_pos.w);
            ecl *= l;
            if (g.bodies[i].sea >= 0.0) { copper = max(copper, 1.0 - l); }
        }
    }
    var o: Light;
    o.day = smoothstep(-0.2, 0.15, dot(up, sd)) * ecl;
    let mi = u32(g.moon_col.w);
    let moon = g.moon_col.rgb * select(1.0, 0.0, body == mi);
    o.l = vec3f(0.0);
    let ndl = dot(n, sd);
    // (the sun's light fades as it nears the horizon on a body with air, ADR 0038 C)
    let occ = left_by(body, p, g.sun_pos.xyz, g.sun_pos.w) * sunset(up, sd, body);
    if (ndl > 0.0 && occ > 0.0 && max(ecl, copper) > 0.0) {
        var seen = occ * sun_seen(p, n, sd, g.sun_pos.w / length(g.sun_pos.xyz - p), body);
        // the planet's clouds (ADR 0076), where the sun is not already hidden
        if (body == 0u && seen > 0.0 && g.cloud.y > 0.0) {
            let b0 = g.bodies[0];
            let m0 = mat3x3f(b0.rot0.xyz, b0.rot1.xyz, b0.rot2.xyz);
            seen *= cl_shadow((p - b0.ctr.xyz) * m0, sd * m0, g.cloud.z, g.cloud);
        }
        // (the glint behind a pane takes its share of the tint)
        o.sun = ecl * seen * dot(ray_tint, vec3f(1.0 / 3.0));
        o.l += (sun_on(body) * ecl + COPPER * copper) * ndl * seen * moon_gain(body) * ray_tint;
    }
    // the moon's light where the sun's is weak, through the same terrain
    if (o.day < 0.5 && max(moon.r, moon.b) > 0.001) {
        let mc = g.bodies[mi].ctr;
        let md = normalize(mc.xyz - p);
        let ndm = dot(n, md);
        if (ndm > 0.0) {
            var mv = 1.0;
            for (var i = 0u; i < 4u; i++) {
                if (i != mi && ((g.n_lamps.w >> i) & 1u) == 1u) { mv *= left_by(i, p, mc.xyz, mc.w); }
            }
            // faded in as the sun's ambient fades out, not switched on at once (ADR 0038 C)
            mv *= 1.0 - smoothstep(0.3, 0.5, o.day);
            if (mv > 0.0) {
                let mr = ray(p, n, md, body, 1e5);
                o.l += moon * ndm * mv * mr * ray_tint;
            }
        }
    }
    // the lamps: shadow rays to the LAMP_RAYS strongest here; the others get their mean visibility
    var top = array<u32, LAMP_RAYS>(NONE, NONE, NONE);
    var topw = array<f32, LAMP_RAYS>(0.0, 0.0, 0.0);
    var topl = array<vec3f, LAMP_RAYS>(vec3f(0.0), vec3f(0.0), vec3f(0.0));
    var rest = vec3f(0.0);
    var e0 = 0u;
    var e1 = 0u;
    let gc = vec3i(floor(p / LAMP_CELL)) + LAMP_G / 2;
    if (g.n_lamps.x > 0u && all(gc >= vec3i(0)) && all(gc < vec3i(LAMP_G))) {
        let ci = u32((gc.z * LAMP_G + gc.y) * LAMP_G + gc.x);
        e0 = lamp_cells[ci];
        e1 = lamp_cells[ci + 1u];
    }
    for (var e = e0; e < e1; e++) {
        let i = lamp_cells[e];
        let lp = lamps[i];
        let v = lp.pos.xyz - p;
        let d2 = dot(v, v);
        if (u32(lp.pos.w) != body || d2 > LAMP_R * LAMP_R) { continue; }
        let ndv = dot(n, v);
        if (ndv <= 0.0) { continue; }
        // its direct light here before its shadow, worked out once (ADR 0025, H)
        var lj = lamp_light(lp, d2) * (ndv * inverseSqrt(d2));
        var wj = dot(lj, vec3f(0.3, 0.5, 0.2));
        if (wj <= 0.0) { continue; }
        // weaker than the short list's weakest (it is sorted, strongest first): straight to `rest`
        if (wj <= topw[LAMP_RAYS - 1u]) {
            rest += lj;
            continue;
        }
        // keep the strongest: insert into the short list, the weakest falls to `rest`
        var j = i;
        for (var k = 0u; k < LAMP_RAYS; k++) {
            if (wj > topw[k]) {
                let tj = top[k];
                let tw = topw[k];
                let tl = topl[k];
                top[k] = j;
                topw[k] = wj;
                topl[k] = lj;
                j = tj;
                wj = tw;
                lj = tl;
            }
        }
        if (j != NONE) { rest += lj; }
    }
    var seen_n = 0.0;
    var seen_v = 0.0;
    for (var k = 0u; k < LAMP_RAYS; k++) {
        if (top[k] == NONE) { continue; }
        // a held lamp's rays pass through its holder's body (ADR 0119)
        let keep = skip_proxy;
        let lw = lamps[top[k]].col.w;
        if (lw > 0.0) { skip_proxy = PROXY_INST + u32(lw) - 1u; }
        let vis = lamp_seen(p, n, lamps[top[k]], body);
        skip_proxy = keep;
        o.l += topl[k] * vis * ray_tint;
        seen_n += 1.0;
        seen_v += vis;
    }
    if (seen_n > 0.0) { o.l += rest * (seen_v / seen_n); }
    return o;
}

// what a ray from p sees: the terrain it hits, lit, or the sky. A metal's reflection (bounce false) sees the
// sun's disc and lamps' faces; a bounce ray sees the sky's light only, and lamps' faces as nothing, since a
// lamp's light is already direct light
fn seen(p: vec3f, d: vec3f, body: u32, px: vec2u, bounce: bool, tmax: f32) -> vec3f {
    let bb = g.bodies[body];
    let db = d * mat3x3f(bb.rot0.xyz, bb.rot1.xyz, bb.rot2.xyz);
    let h = trace(to_body(bb, p), db, body, tmax, 0u);
    // what the glass on the way lets through, before `light` traces again
    let tint = ray_tint;
    if (h.kind == RAY_QUERY_INTERSECTION_NONE) { return select(sky_colour(d), sky_light(p, body), bounce) * tint; }
    let c = chunks[h.instance_custom_data];
    let q = hit_quad(h.instance_custom_data, h.geometry_index, h.primitive_index);
    let dir = (q.x >> 28u) & 7u;
    let a = dir / 2u;
    let b = g.bodies[(c.info >> 8u) & 255u];
    let gp = grid(c, quad_corner(q, 0u, 0.0));
    let f = frame_at(c.info & 7u, b, gp.x, gp.y);
    var hn = f.up;
    if (a == 0u) { hn = f.si; }
    if (a == 1u) { hn = f.sj; }
    if ((dir & 1u) == 1u) { hn = -hn; }
    if ((q.x >> 31u) == 1u && dir == 4u) { hn = slope_normal(c, b, q, h.primitive_index & 1u); }
    let m = q.y & 255u;
    if (((looks[m].info >> 24u) & 1u) == 1u) { return select(seen_colour(m, vec3f(0.0)), vec3f(0.0), bounce) * tint; }
    one_ray = true;
    let l = light(p + d * h.t, brot(b, hn), body);
    one_ray = false;
    // the light from around at the hit: a flat share, the second bounce
    return seen_colour(m, season(b, p + d * h.t)) * (l.l + ambient(l.day) * select(1.0, 0.35, bounce)) * tint;
}

