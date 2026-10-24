#include "fine_gen.h"

#include <godot_cpp/core/class_db.hpp>

#include <algorithm>
#include <cmath>
#include <cstring>
#include <vector>

using namespace godot;

namespace {
constexpr int FS = FineGen::FS;
// Soil under the top voxel: about a metre of the biome's under material.
constexpr int SOIL_FINE = 8;
// Ground relief on dry land (fine voxels): swells, hummocks and roughness the 0.5 m fields cannot
// make, faded out toward water, rivers and ruins (MacroPt::gw).
constexpr float GROUND_AMP = 3.5f;    // ~8 m swells
constexpr float HUMP_AMP = 1.5f;      // ~2.5 m hummocks
constexpr float GROUND_AMP2 = 0.6f;   // ~1 m roughness
// At lod 1 caves are carved only this close to the surface (cave mouths, ADR 0020).
constexpr int CAVE_MOUTH_DEPTH = 24 * FS;

inline int floor_div(int a, int b) { return a >= 0 ? a / b : -((-a + b - 1) / b); }
inline float smoothstep(float a, float b, float x) {
    const float t = std::clamp((x - a) / (b - a), 0.0f, 1.0f);
    return t * t * (3.0f - 2.0f * t);
}
inline float bilerp(float a00, float a10, float a01, float a11, float tx, float tz) {
    return (a00 * (1.0f - tx) + a10 * tx) * (1.0f - tz) + (a01 * (1.0f - tx) + a11 * tx) * tz;
}
inline uint32_t h3(int a, int b, int c) { return WorldGen::hash3(a, b, c); }
inline float seg_dist2(float px, float py, float pz, float ax, float ay, float az, float bx, float by, float bz) {
    const float vx = bx - ax, vy = by - ay, vz = bz - az;
    const float wx = px - ax, wy = py - ay, wz = pz - az;
    const float l2 = vx * vx + vy * vy + vz * vz;
    const float t = l2 > 0.0f ? std::clamp((wx * vx + wy * vy + wz * vz) / l2, 0.0f, 1.0f) : 0.0f;
    const float dx = wx - vx * t, dy = wy - vy * t, dz = wz - vz * t;
    return dx * dx + dy * dy + dz * dz;
}
bool is_flower(uint16_t m) {
    return m == WorldGen::FLOWER || m == WorldGen::FLOWER_RED || m == WorldGen::CORNFLOWER || m == WorldGen::OXEYE_DAISY || m == WorldGen::ALLIUM;
}
// Stone pickups on their bare 0.5 m patch: loose stones, flint, ore-bearing stones. Sticks are
// stamps (FineGen::STICK).
bool is_pickup(uint16_t m) {
    return m == WorldGen::PEBBLES || m == WorldGen::FLINT_STONES
        || (m >= WorldGen::COPPER_BEARING_STONES && m <= WorldGen::IRON_BEARING_STONES);
}
// A stone pickup's shape (lx, lz in 0..FS-1): voxels tall at this column, 0 = none. Vintage Story's
// chunks in voxels: a slab filling the patch, two voxels thick with chipped corners, mirrored and
// turned per patch; loose stones are a size smaller with a chip beside them. The voxels touch, so the player picks the whole
// chunk up at once (player.gd _dig_selection).
int pickup_shape(uint16_t kind, int lx, int lz, uint32_t s) {
    if (s & 1) lx = FS - 1 - lx;
    if ((s >> 1) & 1) std::swap(lx, lz);
    static const int CHUNK[FS][FS] = {{1, 2, 2, 1}, {2, 2, 2, 2}, {2, 2, 2, 1}, {1, 2, 1, 0}};   // [z][x]
    static const int STONES[FS][FS] = {{0, 0, 0, 0}, {1, 2, 1, 0}, {1, 1, 1, 0}, {0, 0, 0, 1}};
    const int v = (kind == WorldGen::PEBBLES ? STONES : CHUNK)[lz][lx];
    // one of the thick cells sits a voxel lower, so chunks are not all the same block
    if (v == 2 && ((s >> 2) & 3) == (uint32_t)((lx + lz) & 3)) return 1;
    return v;
}
} // namespace

FineGen::FineGen() {
    g_.instantiate();
    configure(1337, 256 * FS, 768 * FS);
}

void FineGen::_bind_methods() {
    ClassDB::bind_method(D_METHOD("configure", "seed", "sea_level", "world_height"), &FineGen::configure);
    ClassDB::bind_method(D_METHOD("get_seed"), &FineGen::get_seed);
    ClassDB::bind_method(D_METHOD("height_at", "x", "z"), &FineGen::height_at);
    ClassDB::bind_method(D_METHOD("biome_at", "x", "z"), &FineGen::biome_at);
    ClassDB::bind_method(D_METHOD("column_info", "x", "z"), &FineGen::column_info);
    ClassDB::bind_method(D_METHOD("generate_block", "origin", "size", "lod", "bytes_per_voxel"), &FineGen::generate_block, DEFVAL(2));
    ClassDB::bind_method(D_METHOD("map_image", "center_x", "center_z", "radius", "step"), &FineGen::map_image);
    ClassDB::bind_method(D_METHOD("climate_map", "x0", "z0", "n", "step"), &FineGen::climate_map);
    ClassDB::bind_method(D_METHOD("water_floor_map", "x0", "z0", "n", "step"), &FineGen::water_floor_map);
    ClassDB::bind_method(D_METHOD("find_nearest", "kind", "x", "z", "max_radius"), &FineGen::find_nearest);
    ClassDB::bind_method(D_METHOD("waterfalls_near", "x", "z", "radius"), &FineGen::waterfalls_near);
    ClassDB::bind_method(D_METHOD("hydrology_stats", "x", "z"), &FineGen::hydrology_stats);
    ClassDB::bind_method(D_METHOD("vein_at", "x", "z"), &FineGen::vein_at);
    ClassDB::bind_method(D_METHOD("get_macro"), &FineGen::get_macro);
    ClassDB::bind_integer_constant(get_class_static(), StringName(), "FS", FS);
}

static void setup_noise(FastNoiseLite &n, int seed, float freq, int octaves) {
    n.SetSeed(seed);
    n.SetNoiseType(FastNoiseLite::NoiseType_OpenSimplex2);
    n.SetFrequency(freq);
    n.SetFractalType(octaves > 1 ? FastNoiseLite::FractalType_FBm : FastNoiseLite::FractalType_None);
    n.SetFractalOctaves(octaves);
}

void FineGen::configure(int p_seed, int p_sea_level, int p_world_height) {
    g_->configure(p_seed, p_sea_level / FS, p_world_height / FS);
    setup_noise(n_ground_, p_seed + 101, 1.0f / 64.0f, 2);   // 8 m swells (fine voxels)
    setup_noise(n_ground2_, p_seed + 102, 1.0f / 9.0f, 1);   // ~1 m roughness
    setup_noise(n_hump_, p_seed + 104, 1.0f / 20.0f, 1);     // 2.5 m hummocks
    setup_noise(n_leaf_, p_seed + 103, 1.0f / 6.0f, 1);      // lumps on leaf clumps
    setup_noise(n_litter_, p_seed + 105, 1.0f / 14.0f, 2);   // leaf litter vs grass under trees
    setup_noise(n_meadow_, p_seed + 106, 1.0f / 48.0f, 2);   // thick and thin grass, flower patches
    setup_noise(n_pool_, p_seed + 107, 1.0f / 88.0f, 1);     // ~11 m blobs: where cave floors hold their biome's pool
}

// ---------- surface ----------

FineGen::MacroPt FineGen::macro_point(int mx, int mz, const WorldGen::Ruin *ruins, int nruins) const {
    const WorldGen &g = *g_.ptr();
    MacroPt m;
    m.col = g.column(mx, mz);
    if (m.col.biome == technosim_biome::BADLANDS && m.col.slope == 0.0f) {
        const int hx = g.column_base(mx + 2, mz, false).height, hz = g.column_base(mx, mz + 2, false).height;
        m.col.slope = std::max(std::abs(hx - m.col.height), std::abs(hz - m.col.height)) * 0.5f;
    }
    float h = m.col.hf;
    m.hy = g.hydro_at(mx, mz, m.col.height);
    m.water_level = g.sea_;
    if (m.hy.river > 0.0f) {
        const float target = (float)m.hy.bed;
        if (h > target) h += (target - h) * m.hy.river;
        m.riverbed = m.hy.river > 0.6f;
    }
    if (m.hy.water_level > m.water_level) m.water_level = m.hy.water_level;
    if (m.hy.lake && h < (float)m.hy.water_level) m.lakebed = true;
    if (m.hy.river > 0.0f && h < (float)m.hy.water_level) m.riverbed = true;
    if (m.hy.marsh && m.hy.river < 0.3f) {
        h = (float)WorldGen::marsh_floor((int)std::lround(h), m.hy);
        m.marshbed = m.hy.puddle;
    }
    for (int i = 0; i < nruins; i++)
        if (WorldGen::in_ruin(ruins[i], mx, mz, 2)) { m.ruin = i; break; }
    if (m.ruin >= 0) {
        const float ground = (float)ruins[m.ruin].base;
        if (WorldGen::in_ruin(ruins[m.ruin], mx, mz, 0)) h = ground;
        else if (h > ground + 1.0f) h = ground + 1.0f;
    }
    if (technosim_biome::DECOR_SPIRE[m.col.biome] > 0.0f && m.ruin < 0 && !m.lakebed && !m.riverbed && !m.marshbed && h > (float)m.water_level) {
        m.spike = g.spire_at(m.col.biome, mx, mz);
        h += (float)m.spike;
    }
    m.hc = h;
    float gw = (m.lakebed || m.riverbed || m.marshbed || m.spike > 0 || m.ruin >= 0) ? 0.0f : 1.0f;
    gw *= 1.0f - std::clamp(m.hy.river * 4.0f, 0.0f, 1.0f);
    gw *= smoothstep(0.5f, 6.0f, h - (float)m.water_level);
    if (m.hy.marsh) gw *= 0.3f;
    for (int i = 0; i < nruins && gw > 0.0f; i++)
        if (WorldGen::in_ruin(ruins[i], mx, mz, 6)) gw *= 0.4f;
    m.gw = gw;
    return m;
}

float FineGen::ground_noise(int x, int z) const {
    const float fx = (float)x, fz = (float)z;
    return n_ground_.GetNoise(fx, fz) * GROUND_AMP + n_hump_.GetNoise(fx, fz) * HUMP_AMP + n_ground2_.GetNoise(fx, fz) * GROUND_AMP2;
}

float FineGen::surface_f(int x, int z) const {
    const int mx = floor_div(x, FS), mz = floor_div(z, FS);
    WorldGen::Ruin ruins[4];
    const int nr = g_->ruins_near(mx - 1, mz - 1, mx + 2, mz + 2, ruins, 4);
    const MacroPt a = macro_point(mx, mz, ruins, nr), b = macro_point(mx + 1, mz, ruins, nr);
    const MacroPt c = macro_point(mx, mz + 1, ruins, nr), d = macro_point(mx + 1, mz + 1, ruins, nr);
    const float tx = (float)(x - mx * FS) / FS, tz = (float)(z - mz * FS) / FS;
    return bilerp(a.hc, b.hc, c.hc, d.hc, tx, tz) * FS + ground_noise(x, z) * bilerp(a.gw, b.gw, c.gw, d.gw, tx, tz);
}

int FineGen::height_at(int x, int z) const {
    return std::clamp((int)std::floor(surface_f(x, z) + 0.5f), 8 * FS, g_->world_height_ * FS - 8 * FS);
}

int FineGen::biome_at(int x, int z) const { return g_->biome_at(floor_div(x, FS), floor_div(z, FS)); }

Dictionary FineGen::column_info(int x, int z) const {
    Dictionary d = g_->column_info(floor_div(x, FS), floor_div(z, FS));
    d["height"] = height_at(x, z);
    for (const char *k : {"water_level", "river_bed", "fall_top", "vein_y"}) {
        const int v = (int)d[k];
        if (v >= 0) d[k] = v * FS;
    }
    return d;
}

// ---------- trees and decorations at fine resolution ----------

// How far along its life a canopy tree is (0 sapling .. 1 ancient): old growth is mostly old,
// forests mix every age, trees near the tree line stay young.
float FineGen::tree_age(const WorldGen::Tree &t, uint32_t seed) const {
    const float u = (float)((seed >> 12) & 1023) / 1023.0f;
    if (t.stunted) return 0.15f + 0.1f * u;
    if (t.biome == technosim_biome::OLD_GROWTH_FOREST) return 0.55f + 0.45f * u;
    if (technosim_biome::TREE_DENSITY[t.biome] >= 0.3f) return 0.3f + 0.7f * u;
    return 0.25f + 0.6f * u;
}

// One tree at fine resolution. Oaks and jungle trees: a trunk that thickens with age, flares into
// roots, and splits into 4-8 limbs that bend upward and end in clusters of leaf clumps, so the crown
// is lumpy and far reaching instead of one ball. Birches: slender, small clumps up the top half.
// Spruces: a layered cone. Acacias: a fork carrying flat plates.
void FineGen::build_tree(int kind, float fx, float fz, int trunk_h, int radius, float a, bool old_growth, uint32_t seed, FTree &o) const {
    const int sd = g_->seed_;
    o.seed = seed;
    o.kind = kind;
    o.x = fx;
    o.z = fz;
    o.ground = (int)std::floor(surface_f((int)std::floor(fx), (int)std::floor(fz)) + 0.5f);
    o.base = o.ground - 6;
    o.log = WorldGen::tree_log(kind);
    o.bark = WorldGen::tree_bark(kind);
    o.leaves = WorldGen::tree_leaves(kind);
    o.nb = o.ns = 0;
    o.cone_r = 0.0f;
    auto rnd = [&](int k) { return (float)(h3((int)seed, k, sd + 79) & 1023) / 1023.0f; };
    const float H = (float)(trunk_h * FS) * (0.55f + 0.65f * a) * (old_growth ? 1.15f : 1.0f);
    o.top = o.ground + std::max(6, (int)H);
    const float top = (float)o.top;
    auto blob = [&](float x, float y, float z, float r, float flat) {
        if (o.nb < MAX_BLOBS) o.blobs[o.nb++] = {x, y, z, r, r * flat, r};
    };
    auto seg = [&](float ax, float ay, float az, float bx, float by, float bz, float r0, float r1) {
        if (o.ns < MAX_SEGS) o.segs[o.ns++] = {ax, ay, az, bx, by, bz, r0, r1};
    };
    auto roots = [&](int n, float len, float rr) {
        const float a0 = rnd(50) * 6.2832f;
        for (int i = 0; i < n; i++) {
            const float an = a0 + (float)i * 6.2832f / (float)n + (rnd(51 + i) - 0.5f) * 0.7f;
            const float l = len * (0.7f + 0.5f * rnd(60 + i));
            seg(o.x, (float)o.ground + o.r * 0.7f, o.z, o.x + std::cos(an) * l, (float)o.ground - 2.5f, o.z + std::sin(an) * l, rr, 0.6f);
        }
    };
    switch (kind) {
        case WorldGen::TREE_SPRUCE: {
            o.r = 1.6f + 4.0f * a + (old_growth ? 1.2f : 0.0f);
            o.r_top = 0.9f;
            o.cone_r = (float)(radius * FS) * (0.55f + 0.75f * a) + 2.0f;
            o.cone_lo = o.ground + (int)(H * 0.15f) + 2;
            o.cone_hi = o.top + 2 + (int)(a * 4.0f);
            if (a > 0.45f) roots(4, o.r + 3.0f + 4.0f * a, o.r * 0.45f);
            break;
        }
        case WorldGen::TREE_BIRCH: {
            o.r = 1.2f + 2.2f * a;
            o.r_top = 0.7f;
            const int n = 4 + (int)(3.0f * a);
            for (int i = 0; i < n; i++) {
                const float y = top - H * 0.45f + (float)i * H * 0.45f / (float)n + rnd(70 + i) * 2.0f;
                const float an = rnd(80 + i) * 6.2832f;
                const float d = 1.5f + (2.0f + 3.0f * a) * rnd(90 + i);
                const float bx = o.x + std::cos(an) * d, bz = o.z + std::sin(an) * d;
                seg(o.x, y - 2.0f, o.z, bx, y, bz, 0.7f, 0.5f);
                blob(bx, y + 1.0f, bz, (3.0f + 4.0f * a) * (0.75f + 0.3f * rnd(100 + i)), 0.85f);
            }
            blob(o.x, top + 1.0f, o.z, 3.0f + 3.0f * a, 1.0f);
            break;
        }
        case WorldGen::TREE_ACACIA: {
            o.r = 1.5f + 2.6f * a;
            o.r_top = o.r * 0.6f;
            const float R = (float)(radius * FS) * (0.7f + 0.6f * a) + 3.0f;
            const float a0 = rnd(1) * 6.2832f;
            for (int i = 0; i < 3; i++) {
                const float an = a0 + (float)i * 2.1f + (rnd(10 + i) - 0.5f) * 0.6f;
                const float d = R * (0.45f + 0.25f * rnd(20 + i));
                const float bx = o.x + std::cos(an) * d, bz = o.z + std::sin(an) * d, by = top + 2.0f + rnd(30 + i) * 3.0f;
                seg(o.x, top - H * 0.3f, o.z, bx, by, bz, o.r * 0.6f, 0.8f);
                blob(bx, by + 1.5f, bz, R * (0.55f + 0.2f * rnd(40 + i)), 0.25f);
            }
            break;
        }
        default: {
            const bool jungle = kind == WorldGen::TREE_JUNGLE;
            // trunk radius ~0.25 m young .. 1 m ancient (2 m across), 0.5 m at the crown
            o.r = (1.8f + 6.2f * std::pow(a, 1.2f) + (old_growth ? 1.5f : 0.0f)) * (jungle ? 1.15f : 1.0f);
            o.r_top = std::max(1.0f, o.r * 0.5f);
            const float Rc = (10.0f + 30.0f * a + 6.0f * (float)(radius - 2)) * (jungle ? 0.9f : 1.0f) * (old_growth ? 1.1f : 1.0f);
            const int n = std::min(8, 4 + (int)(3.0f * a + rnd(3) * 1.5f));
            const float a0 = rnd(2) * 6.2832f;
            for (int i = 0; i < n; i++) {
                const float an = a0 + (float)i * 6.2832f / (float)n + (rnd(10 + i) - 0.5f) * 0.7f;
                const float ca = std::cos(an), sa = std::sin(an);
                const float sy = top - H * (jungle ? 0.12f : 0.4f) * (0.3f + 0.7f * rnd(20 + i));
                const float reach = Rc * (0.75f + 0.35f * rnd(30 + i));
                const float mx = o.x + ca * reach * 0.5f, my = sy + Rc * (0.2f + 0.25f * rnd(40 + i)), mz = o.z + sa * reach * 0.5f;
                const float ex = o.x + ca * reach * 0.88f, ey = my + Rc * (0.12f + 0.2f * rnd(45 + i)), ez = o.z + sa * reach * 0.88f;
                const float lr = std::max(1.0f, o.r * 0.55f * (0.8f + 0.3f * rnd(50 + i)));
                seg(o.x, sy, o.z, mx, my, mz, lr, lr * 0.75f);
                seg(mx, my, mz, ex, ey, ez, lr * 0.75f, std::max(0.7f, lr * 0.45f));
                // a cluster of clumps at the end of the limb and one over its middle
                const float cr = Rc * 0.36f + 2.5f;
                blob(ex, ey + cr * 0.25f, ez, cr, 0.7f);
                blob((mx + ex) * 0.5f, (my + ey) * 0.5f + cr * 0.55f, (mz + ez) * 0.5f, cr * 0.8f, 0.7f);
                const float side = (rnd(55 + i) - 0.5f) * 1.8f * cr;
                blob(ex - sa * side + ca * cr * 0.4f, ey - cr * 0.1f, ez + ca * side + sa * cr * 0.4f, cr * 0.65f, 0.65f);
            }
            blob(o.x, top + Rc * 0.18f, o.z, Rc * 0.5f + 2.0f, 0.6f);
            for (int i = 0; i < 2; i++) {
                const float an = rnd(110 + i) * 6.2832f;
                blob(o.x + std::cos(an) * Rc * 0.3f, top + Rc * 0.32f, o.z + std::sin(an) * Rc * 0.3f, Rc * 0.33f + 1.5f, 0.7f);
            }
            if (a > 0.3f) roots(4 + (int)(2.0f * a), o.r + 5.0f + 8.0f * a, o.r * 0.5f);
            break;
        }
    }
    float reach = o.r * 1.8f + 5.0f, yhi = top;
    for (int i = 0; i < o.nb; i++) {
        const Blob &b = o.blobs[i];
        reach = std::max(reach, std::max(std::fabs(b.x - o.x), std::fabs(b.z - o.z)) + b.rx * 1.25f);
        yhi = std::max(yhi, b.y + b.ry * 1.25f);
    }
    for (int i = 0; i < o.ns; i++) {
        const Seg &s = o.segs[i];
        const float r = std::max(s.r0, s.r1);
        reach = std::max(reach, std::max({std::fabs(s.ax - o.x), std::fabs(s.bx - o.x), std::fabs(s.az - o.z), std::fabs(s.bz - o.z)}) + r);
        yhi = std::max(yhi, std::max(s.ay, s.by) + r);
    }
    if (o.cone_r > 0.0f) { reach = std::max(reach, o.cone_r + 1.0f); yhi = std::max(yhi, (float)o.cone_hi); }
    o.reach = reach + 1.0f;
    o.y_hi = (int)std::ceil(yhi) + 1;
}

bool FineGen::col_tree(const FTree &t, float x, float z, float grow, ColTree &c) const {
    c.t = &t;
    c.bm = 0;
    c.sm = 0;
    c.trunk = c.cone = false;
    c.ylo = 1 << 30;
    c.yhi = -(1 << 30);
    const float dx = x - t.x, dz = z - t.z, d2 = dx * dx + dz * dz;
    const float tr = t.r * 1.8f + 4.0f + grow;
    if (d2 <= tr * tr) { c.trunk = true; c.ylo = t.base; c.yhi = t.top; }
    for (int i = 0; i < t.nb; i++) {
        const Blob &b = t.blobs[i];
        const float rx = b.rx * 1.25f + grow + 1.0f, rz = b.rz * 1.25f + grow + 1.0f;
        if (std::fabs(x - b.x) > rx || std::fabs(z - b.z) > rz) continue;
        c.bm |= 1ull << i;
        const float ry = b.ry * 1.25f + grow + 1.0f;
        c.ylo = std::min(c.ylo, (int)std::floor(b.y - ry));
        c.yhi = std::max(c.yhi, (int)std::ceil(b.y + ry));
    }
    for (int i = 0; i < t.ns; i++) {
        const Seg &s = t.segs[i];
        const float r = std::max(s.r0, s.r1) + grow + 1.0f;
        if (x < std::min(s.ax, s.bx) - r || x > std::max(s.ax, s.bx) + r || z < std::min(s.az, s.bz) - r || z > std::max(s.az, s.bz) + r) continue;
        c.sm |= 1u << i;
        c.ylo = std::min(c.ylo, (int)std::floor(std::min(s.ay, s.by) - r));
        c.yhi = std::max(c.yhi, (int)std::ceil(std::max(s.ay, s.by) + r));
    }
    const float cr = t.cone_r + 2.0f + grow;
    if (t.cone_r > 0.0f && d2 <= cr * cr) { c.cone = true; c.ylo = std::min(c.ylo, t.cone_lo); c.yhi = std::max(c.yhi, t.cone_hi); }
    return c.trunk || c.bm || c.sm || c.cone;
}

// leaf_n: the leaf noise at this voxel, computed once (2 = not yet) and shared by every tree.
uint16_t FineGen::tree_voxel(const ColTree &c, float x, int y, float z, float grow, float &leaf_n) const {
    if (y < c.ylo || y > c.yhi) return Mat::AIR;
    const FTree &t = *c.t;
    const float fy = (float)y;
    const float dx = x - t.x, dz = z - t.z, d2 = dx * dx + dz * dz;
    if (c.trunk && y >= t.base && y <= t.top) {
        const float frac = std::clamp((float)(y - t.ground) / (float)std::max(1, t.top - t.ground), 0.0f, 1.0f);
        float r = t.r + (t.r_top - t.r) * frac + grow;
        const float flare = (float)(t.ground + 3) + t.r * 0.8f - fy;   // the trunk widens into its roots
        if (flare > 0.0f) r += flare * 0.3f;
        if (d2 <= r * r) return d2 > (r - 1.5f) * (r - 1.5f) ? t.bark : t.log;
    }
    for (uint32_t m = c.sm; m; m &= m - 1) {
        const Seg &s = t.segs[__builtin_ctz(m)];
        const float vx = s.bx - s.ax, vy = s.by - s.ay, vz = s.bz - s.az;
        const float wx = x - s.ax, wy = fy - s.ay, wz = z - s.az;
        const float l2 = vx * vx + vy * vy + vz * vz;
        const float u = l2 > 0.0f ? std::clamp((wx * vx + wy * vy + wz * vz) / l2, 0.0f, 1.0f) : 0.0f;
        const float ex = wx - vx * u, ey = wy - vy * u, ez = wz - vz * u;
        const float r = s.r0 + (s.r1 - s.r0) * u + grow;
        const float e2 = ex * ex + ey * ey + ez * ez;
        if (e2 <= r * r) return e2 > (r - 1.5f) * (r - 1.5f) ? t.bark : t.log;
    }
    const int ix = (int)std::floor(x), iz = (int)std::floor(z);
    if (c.cone && y >= t.cone_lo && y <= t.cone_hi) {
        const float u = (float)(t.cone_hi - y) / (float)std::max(1, t.cone_hi - t.cone_lo);
        float rr = t.cone_r * u + 0.8f;
        if (((t.cone_hi - y) / 3) & 1) rr *= 0.78f;   // layered boughs
        rr += grow;
        if (d2 <= rr * rr) {
            if (d2 > (rr - 1.5f) * (rr - 1.5f) && (h3(ix, y, iz) & 3) == 0) return Mat::AIR;
            return t.leaves;
        }
    }
    for (uint64_t m = c.bm; m; m &= m - 1) {
        const Blob &b = t.blobs[__builtin_ctzll(m)];
        const float ex = (x - b.x) / (b.rx + grow), ey = (fy - b.y) / (b.ry + grow), ez = (z - b.z) / (b.rz + grow);
        const float d = ex * ex + ey * ey + ez * ez;
        if (d > 1.5f) continue;
        if (leaf_n > 1.5f) leaf_n = n_leaf_.GetNoise(x, fy, z);
        const float edge = d + leaf_n * 0.45f;   // lumpy clumps
        if (edge > 1.0f) continue;
        if (grow == 0.0f && edge > 0.85f && (h3(ix, y, iz) & 15) == 0) continue;   // a few gaps at the edge (each costs faces)
        return t.leaves;
    }
    return Mat::AIR;
}

// Young trees between the canopy trees: one chance per 3 m cell, as often as the biome has trees,
// never on top of the cell's canopy tree. Deterministic per cell, so every block sees the same ones.
void FineGen::young_trees(int x_lo, int z_lo, int x_hi, int z_hi, const WorldGen::Ruin *ruins, int nruins, std::vector<FTree> &out) const {
    const WorldGen &g = *g_.ptr();
    constexpr int YC = 24, M = 20;
    const int cx0 = floor_div(x_lo - M, YC), cx1 = floor_div(x_hi + M, YC);
    const int cz0 = floor_div(z_lo - M, YC), cz1 = floor_div(z_hi + M, YC);
    for (int cz = cz0; cz <= cz1; cz++)
        for (int cx = cx0; cx <= cx1; cx++) {
            const uint32_t hs = h3(cx, cz, g.seed_ + 613);
            const float roll = (float)(hs & 1023) / 1024.0f;
            if (roll >= 0.7f) continue;
            const float fx = (float)(cx * YC + 3 + (int)((hs >> 10) % (YC - 6))) + 0.5f;
            const float fz = (float)(cz * YC + 3 + (int)((hs >> 16) % (YC - 6))) + 0.5f;
            const int mx = floor_div((int)fx, FS), mz = floor_div((int)fz, FS);
            const WorldGen::Column col = g.column_base(mx, mz, false);
            if (col.height <= g.sea_ + 2 || col.alpine) continue;
            const float dens = technosim_biome::TREE_DENSITY[col.biome];
            if (roll >= std::min(0.7f, dens * 0.55f)) continue;
            const WorldGen::Hydro hy = g.hydro_at(mx, mz, col.height);
            if (hy.river > 0.05f || hy.lake || hy.puddle) continue;
            bool blocked = false;
            for (int r = 0; r < nruins && !blocked; r++) blocked = WorldGen::in_ruin(ruins[r], mx, mz, 3);
            if (blocked) continue;
            WorldGen::Tree big;
            if (g.tree_at_cell(floor_div(mx, WorldGen::TREE_CELL), floor_div(mz, WorldGen::TREE_CELL), big)) {
                const float bx = (float)(big.x * FS + 2) - fx, bz = (float)(big.z * FS + 2) - fz;
                if (bx * bx + bz * bz < 14.0f * 14.0f) continue;
            }
            const uint32_t ts = h3(cx, cz, g.seed_ + 617);
            int kind = technosim_biome::TREE_KIND[col.biome];
            if ((float)((ts >> 8) & 1023) / 1024.0f < technosim_biome::TREE_MIX[col.biome]) kind = technosim_biome::TREE_MIX_KIND[col.biome];
            // poles reaching for the light: a tall bare trunk, the crown above head height
            const float age = 0.1f + 0.22f * (float)(ts & 255) / 255.0f;
            const int trunk_h = (kind == WorldGen::TREE_SPRUCE || kind == WorldGen::TREE_BIRCH ? 12 : 10) + (int)((ts >> 20) % 5);
            FTree t;
            build_tree(kind, fx, fz, trunk_h, 2, age, false, ts, t);
            if (t.x + t.reach < x_lo || t.x - t.reach > x_hi || t.z + t.reach < z_lo || t.z - t.reach > z_hi) continue;
            out.push_back(t);
        }
}

void FineGen::build_stamp(const WorldGen::Stamp &s, FStamp &o) const {
    o.kind = s.kind;
    o.seed = s.seed;
    o.x = (float)(s.x * FS + 2);
    o.z = (float)(s.z * FS + 2);
    o.ground = (int)std::floor(surface_f(s.x * FS + 2, s.z * FS + 2) + 0.5f);
    o.mossy = s.mossy;
    o.wood = WorldGen::tree_log(s.tree_kind);
    o.bark = WorldGen::tree_bark(s.tree_kind);
    o.leaves = WorldGen::tree_leaves(s.tree_kind);
    const float an = (float)((s.seed >> 9) & 1023) / 1023.0f * 6.2832f;
    o.dx = std::cos(an);
    o.dz = std::sin(an);
    o.r = 0.0f;
    switch (s.kind) {
        case WorldGen::STAMP_BOULDER: o.size = ((float)s.size + 0.4f) * FS * 0.9f; break;
        case WorldGen::STAMP_BUSH: o.size = (float)s.size * FS + 1.5f; break;
        case WorldGen::STAMP_FALLEN_LOG:
            o.size = (float)s.size * FS * 0.5f + 4.0f;
            o.r = 2.2f + (float)((s.seed >> 20) & 7) * 0.4f;
            break;
        default: o.size = (float)s.size * FS; break;
    }
}

// The forest floor: one chance per 2 m cell for a fallen log (any direction, mossy, with
// mushrooms), a stump, a bush (leafy or blueberry) or a mossy rock; forests get most of them,
// open land a few bushes and stones.
void FineGen::floor_features(int x_lo, int z_lo, int x_hi, int z_hi, const WorldGen::Ruin *ruins, int nruins, std::vector<FStamp> &out) const {
    const WorldGen &g = *g_.ptr();
    constexpr int FC = 16, M = 34;
    const int cx0 = floor_div(x_lo - M, FC), cx1 = floor_div(x_hi + M, FC);
    const int cz0 = floor_div(z_lo - M, FC), cz1 = floor_div(z_hi + M, FC);
    for (int cz = cz0; cz <= cz1; cz++)
        for (int cx = cx0; cx <= cx1; cx++) {
            const uint32_t hs = h3(cx, cz, g.seed_ + 601);
            const float roll = (float)(hs & 1023) / 1024.0f;
            if (roll >= 0.3f) continue;
            const float fx = (float)(cx * FC + 2 + (int)((hs >> 10) % (FC - 4))) + 0.5f;
            const float fz = (float)(cz * FC + 2 + (int)((hs >> 16) % (FC - 4))) + 0.5f;
            const int mx = floor_div((int)fx, FS), mz = floor_div((int)fz, FS);
            const WorldGen::Column col = g.column_base(mx, mz, false);
            if (col.height <= g.sea_ + 1 || col.alpine || col.snow) continue;
            const float dens = technosim_biome::TREE_DENSITY[col.biome];
            const float f = std::clamp(dens / 0.5f, 0.0f, 1.3f);
            const float p_log = 0.05f * f, p_stump = p_log + 0.03f * f, p_bush = p_stump + 0.02f + 0.14f * f, p_rock = p_bush + 0.012f + 0.02f * f;
            if (roll >= p_rock) continue;
            if (dens <= 0.0f && roll < p_stump) continue;   // logs and stumps come from trees
            const WorldGen::Hydro hy = g.hydro_at(mx, mz, col.height);
            if (hy.river > 0.05f || hy.lake || hy.puddle) continue;
            bool blocked = false;
            for (int r = 0; r < nruins && !blocked; r++) blocked = WorldGen::in_ruin(ruins[r], mx, mz, 2);
            if (blocked) continue;
            FStamp s;
            s.seed = h3(cx, cz, g.seed_ + 607);
            int tk = technosim_biome::TREE_KIND[col.biome];
            if ((float)((s.seed >> 12) & 1023) / 1024.0f < technosim_biome::TREE_MIX[col.biome]) tk = technosim_biome::TREE_MIX_KIND[col.biome];
            s.wood = WorldGen::tree_log(tk);
            s.bark = WorldGen::tree_bark(tk);
            s.leaves = WorldGen::tree_leaves(tk);
            s.mossy = dens >= 0.2f;
            s.x = fx;
            s.z = fz;
            const float an = (float)(s.seed & 1023) / 1023.0f * 6.2832f;
            s.dx = std::cos(an);
            s.dz = std::sin(an);
            const float u1 = (float)((s.seed >> 10) & 255) / 255.0f, u2 = (float)((s.seed >> 18) & 255) / 255.0f;
            s.r = 0.0f;
            if (roll < p_log) { s.kind = WorldGen::STAMP_FALLEN_LOG; s.r = 2.0f + 3.5f * u1; s.size = 10.0f + 18.0f * u2; }
            else if (roll < p_stump) { s.kind = STUMP; s.r = 2.5f + 4.0f * u1; s.size = 3.0f + 5.0f * u2; }
            else if (roll < p_bush) {
                s.kind = WorldGen::STAMP_BUSH;
                s.size = 2.5f + 4.0f * u1;
                if (dens > 0.0f && (s.seed >> 26) % 6 == 0) s.leaves = Mat::BLUEBERRY_BUSH;
            } else { s.kind = WorldGen::STAMP_BOULDER; s.size = 2.0f + 3.0f * u1; }
            s.ground = (int)std::floor(surface_f((int)fx, (int)fz) + 0.5f);
            out.push_back(s);
        }
    // Fallen sticks (the twigs pickup, Vintage Story style in voxels): 1-1.5 m long, one voxel thick,
    // a forked side branch, lying at any angle; the column loop lays them on the ground. Half the
    // 2 m cells of a forest hold one, a few in open land.
    constexpr int SM = 12;
    const int sx0 = floor_div(x_lo - SM, FC), sx1 = floor_div(x_hi + SM, FC);
    const int sz0 = floor_div(z_lo - SM, FC), sz1 = floor_div(z_hi + SM, FC);
    for (int cz = sz0; cz <= sz1; cz++)
        for (int cx = sx0; cx <= sx1; cx++) {
            const uint32_t hs = h3(cx, cz, g.seed_ + 613);
            const float roll = (float)(hs & 1023) / 1024.0f;
            if (roll >= 0.55f) continue;
            const float fx = (float)(cx * FC + 3 + (int)((hs >> 10) % (FC - 6))) + 0.5f;
            const float fz = (float)(cz * FC + 3 + (int)((hs >> 16) % (FC - 6))) + 0.5f;
            const int mx = floor_div((int)fx, FS), mz = floor_div((int)fz, FS);
            const WorldGen::Column col = g.column_base(mx, mz, false);
            if (col.height <= g.sea_ + 1 || col.alpine || col.snow) continue;
            const float dens = technosim_biome::TREE_DENSITY[col.biome];
            if (roll >= 0.05f + 0.5f * std::clamp(dens / 0.5f, 0.0f, 1.0f)) continue;
            const WorldGen::Hydro hy = g.hydro_at(mx, mz, col.height);
            if (hy.river > 0.05f || hy.lake || hy.puddle || hy.marsh) continue;
            bool blocked = false;
            for (int r = 0; r < nruins && !blocked; r++) blocked = WorldGen::in_ruin(ruins[r], mx, mz, 2);
            if (blocked) continue;
            FStamp s{};
            s.kind = STICK;
            s.seed = h3(cx, cz, g.seed_ + 617);
            s.x = fx;
            s.z = fz;
            const float an = (float)(s.seed & 1023) / 1023.0f * 3.1416f;
            s.dx = std::cos(an);
            s.dz = std::sin(an);
            s.size = 4.0f + (float)((s.seed >> 10) & 7) * 0.4f;   // half length: 8-12 voxels long
            s.ground = (int)std::floor(surface_f((int)fx, (int)fz) + 0.5f);
            out.push_back(s);
        }
}

float FineGen::stick_dist(const FStamp &s, float x, float z) {
    const float d = std::sqrt(seg_dist2(x, 0.0f, z, s.x - s.dx * s.size, 0.0f, s.z - s.dz * s.size, s.x + s.dx * s.size, 0.0f, s.z + s.dz * s.size));
    // the fork: a third of the way along, 3-4 voxels out at 40 degrees to one side, pointing forward
    const float along = s.size * (((s.seed >> 14) & 1) ? 0.35f : -0.35f);
    const float side = ((s.seed >> 15) & 1) ? 1.0f : -1.0f;
    const float bx = s.x + s.dx * along, bz = s.z + s.dz * along;
    const float bl = 3.0f + (float)((s.seed >> 16) & 1);
    const float ex = bx + (s.dx * 0.77f - s.dz * side * 0.64f) * bl, ez = bz + (s.dz * 0.77f + s.dx * side * 0.64f) * bl;
    return std::min(d, std::sqrt(seg_dist2(x, 0.0f, z, bx, 0.0f, bz, ex, 0.0f, ez)));
}

uint16_t FineGen::stamp_voxel(const FStamp &s, float x, int y, float z, float grow) const {
    const float dx = x - s.x, dz = z - s.z, fy = (float)y;
    switch (s.kind) {
        case WorldGen::STAMP_BOULDER: {
            const float rx = s.size * (0.85f + 0.3f * (float)(s.seed & 7) / 7.0f) + grow, rz = s.size + grow, ry = s.size * 0.7f + grow;
            const float cy = (float)s.ground - s.size * 0.25f;
            auto inside = [&](float yy) {
                const float e = (dx * dx) / (rx * rx) + ((yy - cy) * (yy - cy)) / (ry * ry) + (dz * dz) / (rz * rz);
                const float wob = ((float)(h3((int)std::floor(x) >> 1, (int)std::floor(yy) >> 1, (int)std::floor(z) >> 1) & 15) / 15.0f - 0.5f) * 0.3f;
                return e < 1.0f + wob;
            };
            if (inside(fy)) return Mat::STONE;
            if (s.mossy && inside(fy - 1.0f) && (h3((int)x, (int)z, (int)s.seed) % 3) != 0) return Mat::MOSS;
            return Mat::AIR;
        }
        case WorldGen::STAMP_BUSH: {
            const float r = s.size + grow;
            const float e = (dx * dx + dz * dz) / (r * r) + ((fy - (float)s.ground) * (fy - (float)s.ground)) / (r * r * 0.5f);
            if (e > 1.0f + n_leaf_.GetNoise(x, fy, z) * 0.35f) return Mat::AIR;
            if (dx * dx + dz * dz < 2.0f && y < s.ground + 2) return s.wood;
            return s.leaves;
        }
        case WorldGen::STAMP_FALLEN_LOG: {
            const float a = dx * s.dx + dz * s.dz, b = -dx * s.dz + dz * s.dx;
            if (std::fabs(a) > s.size + 0.5f) return Mat::AIR;
            const float cy = (float)s.ground + s.r - 0.6f;   // settled into the ground
            const float r = s.r + grow;
            const float d2 = b * b + (fy - cy) * (fy - cy);
            if (d2 <= r * r) return std::fabs(a) > s.size - 0.5f ? s.wood : s.bark;
            // a broken-off branch stub
            const float sa = s.size * 0.3f * (((s.seed >> 3) & 1) ? 1.0f : -1.0f), side = ((s.seed >> 4) & 1) ? 1.0f : -1.0f;
            const float bx0 = s.x + s.dx * sa, bz0 = s.z + s.dz * sa;
            if (seg_dist2(x, fy, z, bx0, cy, bz0, bx0 - s.dz * side * (r + 3.0f), cy + r + 1.0f, bz0 + s.dx * side * (r + 3.0f)) <= (0.8f + grow) * (0.8f + grow)) return s.bark;
            if (s.mossy && d2 <= (r + 1.0f) * (r + 1.0f) && fy > cy + r * 0.5f) {
                const uint32_t hh = h3((int)std::floor(x), y, (int)std::floor(z) + (int)s.seed);
                if (hh % 3 == 0) return Mat::MOSS;
                if (hh % 29 == 1) return (hh >> 8) % 3 == 0 ? Mat::MUSHROOM_RED : Mat::MUSHROOM_BROWN;
            }
            return Mat::AIR;
        }
        case STICK: return Mat::AIR;   // laid on each column's ground by the column loop
        case STUMP: {
            const float d2 = dx * dx + dz * dz;
            const int top = s.ground + (int)s.size;
            if (y < s.ground - 3 || y > top + 1) return Mat::AIR;
            float r = s.r + grow;
            if (y < s.ground + 2) r += (float)(s.ground + 2 - y) * 0.6f;   // root flare
            if (y <= top && d2 <= r * r) return y == top ? s.wood : s.bark;
            if (y == top + 1 && s.mossy && d2 <= r * r && h3((int)std::floor(x), y, (int)std::floor(z)) % 3 == 0) return Mat::MOSS;
            return Mat::AIR;
        }
        default: {   // dead tree: a bare trunk with one stub branch
            const float r = 1.3f + grow;
            if (y >= s.ground - 2 && y < s.ground + (int)s.size && dx * dx + dz * dz <= r * r) return s.wood;
            static const float BX[4] = {1, -1, 0, 0}, BZ[4] = {0, 0, 1, -1};
            const int bi = (int)((s.seed >> 8) & 3);
            const float by = (float)s.ground + s.size * 0.7f;
            if (seg_dist2(x, fy, z, s.x, by, s.z, s.x + BX[bi] * 5.0f, by + 3.0f, s.z + BZ[bi] * 5.0f) <= (0.9f + grow) * (0.9f + grow)) return s.wood;
            return Mat::AIR;
        }
    }
}

// Positive inside a cave (macro units): cheese rooms and spaghetti tunnels as one
// smooth field, so the fine voxels can interpolate it instead of evaluating 3D noise each.
float FineGen::cave_field(int mx, int my, int mz, int height) const {
    const WorldGen &g = *g_.ptr();
    const float fx = (float)mx, fy = (float)my, fz = (float)mz;
    const float depth = (float)(height - my);
    const float thr = 0.62f - 0.12f * smoothstep(20.0f, 200.0f, depth);
    const float cheese = (g.n_cheese_.GetNoise(fx, fy * 1.6f, fz) - thr) / 0.08f;
    const float s1 = g.n_spag1_.GetNoise(fx, fy * 1.3f, fz), s2 = g.n_spag2_.GetNoise(fx, fy * 1.3f, fz);
    const float spag = (0.0115f - (s1 * s1 + s2 * s2)) / 0.0115f;
    return std::max(cheese, spag);
}

// ---------- blocks ----------

PackedByteArray FineGen::generate_block(Vector3i origin, Vector3i size, int lod, int bytes_per_voxel) const {
    PackedByteArray out;
    const WorldGen &g = *g_.ptr();
    const int step = 1 << lod;
    const int sx = size.x, sy = size.y, sz = size.z;
    if (sx <= 0 || sy <= 0 || sz <= 0) return out;
    const int world_h = g.world_height_ * FS, sea = g.sea_ * FS;
    if (origin.y >= world_h) return out;
    // Lattice of macro columns: every macro column at lod <= 2, every sample beyond.
    const int L = std::max(FS, step);
    const int lx0 = floor_div(origin.x, L) * L, lz0 = floor_div(origin.z, L) * L;
    const int nx = floor_div(origin.x + (sx - 1) * step - lx0, L) + 2;
    const int nz = floor_div(origin.z + (sz - 1) * step - lz0, L) + 2;
    const int mx0 = lx0 / FS, mz0 = lz0 / FS, mstep = L / FS;
    WorldGen::Ruin ruins[4];
    const int nruins = g.ruins_near(mx0 - 1, mz0 - 1, mx0 + nx * mstep + 1, mz0 + nz * mstep + 1, ruins, 4);
    std::vector<MacroPt> lat((size_t)nx * nz);
    int hmax_m = 0, wl_max = 0, fall_max = -1;
    for (int j = 0; j < nz; j++)
        for (int i = 0; i < nx; i++) {
            MacroPt &m = lat[(size_t)j * nx + i];
            m = macro_point(mx0 + i * mstep, mz0 + j * mstep, ruins, nruins);
            hmax_m = std::max(hmax_m, (int)std::ceil(m.hc) + m.spike);
            wl_max = std::max(wl_max, m.water_level);
            fall_max = std::max(fall_max, m.hy.fall_top);
        }
    const int y_top = origin.y + sy * step;
    const bool above_sea = y_top > sea;
    // Trees (lod <= 3 as shapes, from lod 4 only a leaf-coloured surface) and decorations (lod <= 2).
    static constexpr int MAXT = 160, MAXS = 32;
    WorldGen::Tree mtrees[MAXT];
    std::vector<FTree> trees;
    std::vector<FStamp> stamps;
    const int x_lo = origin.x, x_hi = origin.x + (sx - 1) * step, z_lo = origin.z, z_hi = origin.z + (sz - 1) * step;
    int feature_top = 0;
    if (above_sea && origin.y < (hmax_m + 40) * FS) {
        if (lod <= 3) {
            // canopy trees reach up to ~7 m from their trunk: search 12 macro voxels beyond the block
            const int nt = g.trees_near(floor_div(x_lo, FS) - 12, floor_div(z_lo, FS) - 12, floor_div(x_hi, FS) + 12, floor_div(z_hi, FS) + 12, mtrees, MAXT);
            trees.reserve(nt + 16);
            for (int i = 0; i < nt; i++) {
                bool blocked = false;
                for (int r = 0; r < nruins && !blocked; r++) blocked = WorldGen::in_ruin(ruins[r], mtrees[i].x, mtrees[i].z, 3);
                if (blocked) continue;
                const WorldGen::Tree &mt = mtrees[i];
                const uint32_t ts = h3(mt.x, mt.z, g.seed_ + 77);
                const float fx = (float)(mt.x * FS + 2) + (float)((int)(ts & 3) - 1) * 0.5f;
                const float fz = (float)(mt.z * FS + 2) + (float)((int)((ts >> 2) & 3) - 1) * 0.5f;
                // cheap reject before the tree is built
                const float bound = (12.0f + 4.0f) * FS;
                if (fx + bound < x_lo || fx - bound > x_hi || fz + bound < z_lo || fz - bound > z_hi) continue;
                FTree t;
                build_tree(mt.kind, fx, fz, mt.trunk_h, mt.radius, tree_age(mt, ts), mt.biome == technosim_biome::OLD_GROWTH_FOREST, ts, t);
                if (t.x + t.reach < x_lo || t.x - t.reach > x_hi || t.z + t.reach < z_lo || t.z - t.reach > z_hi) continue;
                if (t.y_hi < origin.y || t.base >= y_top) continue;
                feature_top = std::max(feature_top, t.y_hi);
                trees.push_back(t);
            }
            if (lod <= 2) {
                const size_t first = trees.size();
                young_trees(x_lo, z_lo, x_hi, z_hi, ruins, nruins, trees);
                for (size_t i = first; i < trees.size();) {
                    if (trees[i].y_hi < origin.y || trees[i].base >= y_top) { trees[i] = trees.back(); trees.pop_back(); continue; }
                    feature_top = std::max(feature_top, trees[i].y_hi);
                    i++;
                }
            }
        }
        if (lod <= 2) {
            WorldGen::Stamp ms[MAXS];
            const int ns = g.stamps_near(floor_div(x_lo, FS) - 2, floor_div(z_lo, FS) - 2, floor_div(x_hi, FS) + 2, floor_div(z_hi, FS) + 2, ms, MAXS);
            for (int i = 0; i < ns; i++) {
                bool blocked = false;
                for (int r = 0; r < nruins && !blocked; r++) blocked = WorldGen::in_ruin(ruins[r], ms[i].x, ms[i].z, 2);
                if (blocked) continue;
                FStamp s;
                build_stamp(ms[i], s);
                stamps.push_back(s);
            }
            floor_features(x_lo, z_lo, x_hi, z_hi, ruins, nruins, stamps);
            for (const FStamp &s : stamps) feature_top = std::max(feature_top, s.ground + (int)(s.size + s.r) + 8);
        }
    }
    // Nothing reaches this block: all air.
    const int far_canopy_top = (lod >= 4 && above_sea) ? (hmax_m + 2) * FS + 112 : 0;   // far forests stand ~14 m tall
    const int reach_top = std::max({(hmax_m + 12) * FS, wl_max * FS + 1, fall_max * FS + 1, feature_top + 1, (hmax_m + 1) * FS + 12, far_canopy_top});
    if (origin.y >= reach_top) return out;

    // Caves: a field on the macro lattice (lod 0, and cave mouths at lod 1), interpolated per voxel.
    const bool do_caves = lod <= 1;
    const bool do_ores = lod == 0;
    const bool do_veg = lod <= 1;
    int cy0 = 0, cny = 0;
    std::vector<float> cav;
    if (do_caves && y_top > 12 * FS && origin.y < hmax_m * FS) {
        cy0 = floor_div(origin.y - step, FS);
        cny = floor_div(origin.y + sy * step, FS) - cy0 + 2;
        cav.resize((size_t)nx * nz * cny);
        for (int j = 0; j < nz; j++)
            for (int i = 0; i < nx; i++) {
                const MacroPt &m = lat[(size_t)j * nx + i];
                for (int k = 0; k < cny; k++)
                    cav[((size_t)j * nx + i) * cny + k] = cave_field(mx0 + i, cy0 + k, mz0 + j, m.col.height);
            }
    }
    const int n = sx * sy * sz;
    std::vector<uint16_t> tmp(n, 0);
    uint16_t *w = tmp.data();
    bool any = false;
    std::vector<uint8_t> cave(sy + 2);
    for (int z = 0; z < sz; z++) {
        const int wz = origin.z + z * step;
        const float uz = (float)(wz - lz0) / L;
        const int j = std::min((int)uz, nz - 2);
        const float tz = uz - j;
        for (int x = 0; x < sx; x++) {
            const int wx = origin.x + x * step;
            const float ux = (float)(wx - lx0) / L;
            const int i = std::min((int)ux, nx - 2);
            const float tx = ux - i;
            const MacroPt &c00 = lat[(size_t)j * nx + i], &c10 = lat[(size_t)j * nx + i + 1];
            const MacroPt &c01 = lat[(size_t)(j + 1) * nx + i], &c11 = lat[(size_t)(j + 1) * nx + i + 1];
            // discrete attributes from a jittered nearest corner: organic borders
            const uint32_t cj = h3(wx, wz, g.seed_ + 211);
            const float jx = ((float)(cj & 255) / 255.0f - 0.5f) * 0.6f, jz = ((float)((cj >> 8) & 255) / 255.0f - 0.5f) * 0.6f;
            const bool ex = tx + jx >= 0.5f, ez = tz + jz >= 0.5f;
            const MacroPt &m = ez ? (ex ? c11 : c01) : (ex ? c10 : c00);
            const int mx = floor_div(wx, FS), mz = floor_div(wz, FS);
            const float hf = bilerp(c00.hc, c10.hc, c01.hc, c11.hc, tx, tz) * FS + ground_noise(wx, wz) * bilerp(c00.gw, c10.gw, c01.gw, c11.gw, tx, tz);
            const int h = std::clamp((int)std::floor(hf + 0.5f), 8 * FS, world_h - 8 * FS);
            // real slope of the interpolated surface (fine voxels per fine voxel)
            const float gx = ((c10.hc - c00.hc) * (1.0f - tz) + (c11.hc - c01.hc) * tz) * FS / L;
            const float gz = ((c01.hc - c00.hc) * (1.0f - tx) + (c11.hc - c10.hc) * tx) * FS / L;
            const float slope = std::max({std::fabs(gx), std::fabs(gz), m.col.slope * 0.75f});
            const WorldGen::Column &col = m.col;
            const WorldGen::Hydro &hy = m.hy;
            // Rivers and lakes (P13 water look): the surface is interpolated between the wet corners, so a
            // river descends a voxel (12.5 cm) at a time instead of in 0.5 m terraces with glass walls between,
            // and a bank column beside the water whose ground lies lower fills up to it (the water runs into
            // the sides instead of hanging over a dry pocket).
            int water_level = m.water_level * FS;
            bool side_fill = false;
            {
                const MacroPt *cs[4] = {&c00, &c10, &c01, &c11};
                const float ws[4] = {(1.0f - tx) * (1.0f - tz), tx * (1.0f - tz), (1.0f - tx) * tz, tx * tz};
                float sum = 0.0f, wsum = 0.0f;
                for (int k = 0; k < 4; k++) {
                    // only corners that hold water: hydrology marks a river's whole trench (river ~1) but
                    // gives only its channel a level; the rest of the trench is filled from it here
                    if ((cs[k]->hy.river <= 0.0f && !cs[k]->hy.lake) || cs[k]->hy.water_level < 0) continue;
                    sum += std::max(ws[k], 0.02f) * (float)cs[k]->water_level;
                    wsum += std::max(ws[k], 0.02f);
                }
                if (wsum > 0.0f) {
                    const int wl = (int)std::floor(sum / wsum * FS + 0.5f);
                    if ((hy.river > 0.0f || hy.lake) && hy.water_level >= 0) water_level = wl;
                    else if (wl > water_level && h < wl) { water_level = wl; side_fill = true; }
                }
            }
            const bool lakebed = (m.lakebed || side_fill) && h < water_level, riverbed = m.riverbed, marshbed = m.marshbed;
            const WorldGen::Ruin *ruin = m.ruin >= 0 ? &ruins[m.ruin] : nullptr;
            const int spike = m.spike * FS;
            const uint16_t spire_m = spike > 0 ? (uint16_t)technosim_biome::SPIRE_MATERIAL[col.biome] : Mat::AIR;
            const bool beach_sand = col.biome == WorldGen::BIOME_BEACH || (col.biome == WorldGen::BIOME_OCEAN && col.c > -0.4f);
            const bool steep = slope > 1.1f, scree = slope > 0.75f && slope <= 1.1f && col.alpine;
            const bool cold = col.t < -0.45f || col.snow;
            const uint32_t ph = h3(wx >> 1, wz >> 1, g.seed_ + 19);   // 25 cm patches
            uint16_t top_m, under_m;
            if (lakebed || riverbed || marshbed) {
                const uint32_t bh = ph & 7;
                top_m = bh < 3 ? Mat::CLAY : (bh < 5 ? Mat::SAND_STATIC : Mat::GRAVEL_STATIC);
                under_m = (bh & 1) ? Mat::CLAY : Mat::GRAVEL_STATIC;
            } else if (hy.river > 0.08f && h <= water_level + 2 * FS) {
                top_m = under_m = Mat::SAND_STATIC;
            } else if (col.snow && !steep) {
                top_m = Mat::SNOW; under_m = Mat::STONE;
            } else if (steep) {
                top_m = under_m = technosim_biome::SURFACE_UNDER[col.biome] == Mat::TERRACOTTA ? Mat::TERRACOTTA : Mat::STONE;
            } else if (scree) {
                top_m = under_m = Mat::GRAVEL_STATIC;
            } else if (col.alpine) {
                const uint32_t ah = h3(wx >> 2, wz >> 2, g.seed_ + 23) & 7;
                top_m = ah < 3 ? Mat::GRASS : (ah < 5 ? Mat::GRAVEL_STATIC : Mat::STONE);
                under_m = ah < 3 ? Mat::DIRT : Mat::STONE;
            } else if (hy.marsh) {
                const uint32_t mh = ph & 7;
                top_m = mh < 2 ? Mat::CLAY : (mh < 5 ? Mat::DIRT : Mat::GRASS);
                under_m = mh < 4 ? Mat::CLAY : Mat::DIRT;
            } else {
                top_m = (uint16_t)technosim_biome::SURFACE_TOP[col.biome];
                under_m = (uint16_t)technosim_biome::SURFACE_UNDER[col.biome];
                if (col.biome == WorldGen::BIOME_OCEAN && !beach_sand) top_m = under_m = Mat::GRAVEL_STATIC;
                if (technosim_biome::SURFACE_STEEP[col.biome] >= 0 && slope > 0.6f) top_m = (uint16_t)technosim_biome::SURFACE_STEEP[col.biome];
            }
            if (spike > 0) top_m = under_m = spire_m;
            const bool strata = under_m == Mat::TERRACOTTA;
            const int soil = SOIL_FINE + (int)(ph >> 8) % 4;
            const int sandstone_depth = FS * (col.biome == technosim_biome::BADLANDS ? 24 : col.biome == WorldGen::BIOME_DESERT ? 16
                : ((col.biome == WorldGen::BIOME_BEACH || col.biome == technosim_biome::SNOWY_BEACH) ? 8 : 0));
            const int ti = technosim_biome::band_of(technosim_biome::TEMP_BAND_MAX, col.t);
            const int hi = technosim_biome::band_of(technosim_biome::HUM_BAND_MAX, col.h);
            // ore vein of this macro column's cell
            WorldGen::Vein vein;
            const bool has_vein = g.vein_at_cell(floor_div(mx, technosim_ore::CELL), floor_div(mz, technosim_ore::CELL), vein);
            const float vein_ht = (has_vein && do_ores) ? g.vein_half_thickness(vein, mx, mz) : 0.0f;
            // Plants: the macro column decides the kind, the fine column whether it stands here.
            uint16_t veg = Mat::AIR, veg_head = Mat::AIR;
            int veg_h = 0;
            bool pickup = false;   // the macro column holds a ground pickup: its 0.5 m stays bare so the pickup shows
            if (do_veg && !ruin && h >= water_level && hy.fall_top < 0 && h > sea && spike == 0 && !lakebed && !riverbed) {
                int mh = 1;
                uint16_t kind = g.vegetation_at(col, mx, mz, top_m, hy, mh);
                if (kind == Mat::TWIGS) kind = Mat::TALL_GRASS;   // sticks are stamps, longer than a macro column
                if (has_vein && (kind == Mat::AIR || kind == Mat::TALL_GRASS)) {
                    const uint16_t ind = g.indicator_at(vein, mx, mz, h / FS, top_m);
                    if (ind != Mat::AIR) kind = ind;
                }
                const int lx = wx - mx * FS, lz = wz - mz * FS;
                const uint32_t hv = h3(wx, wz, g.seed_ + 5);
                const bool centre = lx == 1 && lz == 2;
                const bool grassy = top_m == Mat::GRASS;
                pickup = is_pickup(kind);
                switch (kind) {
                    case Mat::AIR: break;
                    case Mat::TALL_GRASS: {
                        // tufts on about a third of the columns, mostly one voxel tall, so the ground reads
                        // as grass with tufts rather than a carpet of cubes
                        const uint32_t th = (hv >> 4) % 20;
                        if ((hv & 15) < 5) { veg = Mat::TALL_GRASS; veg_h = th < 13 ? 1 : (th < 19 ? 2 : 3); }
                        break;
                    }
                    case Mat::FERN:
                        if ((hv & 7) < 4) { veg = Mat::FERN; veg_h = 2 + (int)((hv >> 3) % 3); }
                        break;
                    case Mat::REED:
                        if ((hv & 7) < 4) { veg = Mat::REED; veg_h = 4 + (int)((hv >> 3) % 5); }
                        break;
                    case Mat::CACTUS:
                        if (lx >= 1 && lx <= 2 && lz >= 1 && lz <= 2) { veg = Mat::CACTUS; veg_h = mh * FS; }
                        break;
                    case Mat::DEAD_BUSH:
                        if ((hv & 7) < 2) { veg = Mat::DEAD_BUSH; veg_h = 1 + (int)((hv >> 3) % 2); }
                        break;
                    case Mat::STONE:
                        if (lx < 3 && lz < 3) { veg = Mat::STONE; veg_h = (lx + lz < 3) ? 2 : 1; }
                        break;
                    case Mat::PUMPKIN:
                        if (lx >= 1 && lx <= 2 && lz >= 1 && lz <= 2) { veg = Mat::PUMPKIN; veg_h = 2; }
                        break;
                    case Mat::BLUEBERRY_BUSH: {
                        // a rounded leafy dome filling the 0.5 m column: 4 voxels high in the middle, 2-3 at
                        // the edges, a ragged 0-1 at the corners (a flat 3x3 patch did not read as a bush)
                        const int ex = (lx == 0 || lx == 3) ? 1 : 0, ez = (lz == 0 || lz == 3) ? 1 : 0;
                        const int edge = ex + ez;
                        const int hh = edge == 0 ? 4 - (int)((hv >> 2) % 4 == 0) : (edge == 1 ? 2 + (int)((hv >> 2) & 1) : (int)((hv >> 2) & 1));
                        if (hh > 0) { veg = Mat::BLUEBERRY_BUSH; veg_h = hh; }
                        break;
                    }
                    default:
                        if (is_flower(kind)) {
                            if (centre) { veg = Mat::TALL_GRASS; veg_h = 2; veg_head = kind; }
                            else if (grassy && (hv & 7) < 2) { veg = Mat::TALL_GRASS; veg_h = 1; }
                        } else if (pickup) {
                            veg_h = pickup_shape(kind, lx, lz, h3(mx, mz, g.seed_ + 41));
                            if (veg_h > 0) veg = kind;
                        } else if (centre) {
                            veg = kind; veg_h = 1;   // mushrooms, wild crops, carcasses: one voxel
                        } else if (grassy && !pickup && (hv & 7) < 2) {
                            veg = Mat::TALL_GRASS; veg_h = 1;
                        }
                        break;
                }
            }
            // far LOD: a crown-shaped dome of leaves over the cell's tree (its cell clips it), so far
            // forests keep their height instead of turning into green ground
            bool far_canopy = false;
            int far_top = 0;
            uint16_t far_leaves = Mat::LEAVES;
            if (lod >= 4 && above_sea && !ruin && h > water_level && technosim_biome::TREE_DENSITY[col.biome] > 0.0f) {
                WorldGen::Tree t;
                const int tcx = floor_div(mx, WorldGen::TREE_CELL), tcz = floor_div(mz, WorldGen::TREE_CELL);
                if (g.tree_at_cell(tcx, tcz, t)) {
                    const uint32_t ts = h3(t.x, t.z, g.seed_ + 77);
                    const float a = tree_age(t, ts);
                    const float H = (float)(t.trunk_h * FS) * (0.55f + 0.65f * a);
                    const float rc = t.kind == WorldGen::TREE_SPRUCE ? (float)(t.radius * FS) * (0.55f + 0.75f * a) + 2.0f
                                                                     : 10.0f + 30.0f * a + 6.0f * (float)(t.radius - 2);
                    const float dx = (float)(wx - (t.x * FS + 2)), dz = (float)(wz - (t.z * FS + 2));
                    const float q = 1.0f - (dx * dx + dz * dz) / (rc * rc);
                    if (q > 0.0f) {
                        far_canopy = true;
                        far_top = h + (int)(H * 0.55f + (H * 0.45f + rc * 0.5f) * std::sqrt(q));
                        far_leaves = WorldGen::tree_leaves(t.kind);
                    }
                }
            }
            // trees and decorations whose bounds touch this column
            const float grow = step > 1 ? (float)step * 0.45f : 0.0f;
            ColTree ctrees[24];
            int nct = 0;
            bool shaded = false;
            for (const FTree &t : trees) {
                if (nct >= 24) break;
                if (std::fabs((float)wx - t.x) > t.reach + step || std::fabs((float)wz - t.z) > t.reach + step) continue;
                if (col_tree(t, (float)wx, (float)wz, grow, ctrees[nct])) {
                    shaded = shaded || ctrees[nct].bm || ctrees[nct].cone;
                    nct++;
                }
            }
            const FStamp *cst[16];
            int ncs = 0;
            if (!ruin && spike == 0 && !lakebed && !riverbed && h >= water_level - FS) {
                for (const FStamp &s : stamps) {
                    if (ncs >= 16) break;
                    const float rr = std::max(s.size * 1.3f, s.size + s.r) + 6.0f;
                    if (std::fabs((float)wx - s.x) <= rr && std::fabs((float)wz - s.z) <= rr) cst[ncs++] = &s;
                }
            }
            // The forest floor and open grass (2026-10-23): leaf litter, ferns, mushrooms at the foot of
            // trunks and twigs under the canopy; grass that thickens and thins in patches, with patches of
            // one kind of flower, in the open.
            if (top_m == Mat::GRASS && !ruin && spike == 0 && h > water_level && hy.fall_top < 0) {
                const bool forest = technosim_biome::TREE_DENSITY[col.biome] >= 0.2f;
                const bool under = forest && shaded;
                if (under && n_litter_.GetNoise((float)wx, (float)wz) > -0.4f) top_m = Mat::LEAF_LITTER;
                if (do_veg && h > sea && !pickup && (veg == Mat::AIR || veg == Mat::TALL_GRASS) && veg_head == Mat::AIR) {
                    const uint32_t fh = h3(wx, wz, g.seed_ + 31);
                    const uint32_t r10 = fh & 1023;
                    veg = Mat::AIR;
                    veg_h = 0;
                    if (under) {
                        bool near_wood = false;
                        for (int k = 0; k < nct && !near_wood; k++) {
                            if (!ctrees[k].trunk) continue;
                            const float dx = (float)wx - ctrees[k].t->x, dz = (float)wz - ctrees[k].t->z, rr = ctrees[k].t->r + 5.0f;
                            near_wood = dx * dx + dz * dz <= rr * rr;
                        }
                        if (near_wood && r10 < 150) { veg = ((fh >> 10) & 3) == 0 ? Mat::MUSHROOM_RED : Mat::MUSHROOM_BROWN; veg_h = 1; }
                        else if (r10 < 300) { veg = Mat::FERN; veg_h = 2 + (int)((fh >> 12) % 3); }
                        else if (r10 < 420) { veg = Mat::TALL_GRASS; veg_h = 1 + (int)((fh >> 12) & 1); }
                    } else {
                        const float mead = n_meadow_.GetNoise((float)wx, (float)wz);
                        const float cover = 0.12f + 0.3f * mead + (forest ? 0.1f : 0.0f);   // every tuft costs ~10 faces
                        if ((float)r10 / 1024.0f < cover) {
                            const uint32_t th = (fh >> 10) % 20;
                            veg = Mat::TALL_GRASS;
                            veg_h = (th < 9 ? 1 : (th < 16 ? 2 : (th < 19 ? 3 : 4))) + (mead > 0.4f ? 1 : 0);
                        }
                        if (n_meadow_.GetNoise((float)wx * 0.37f + 500.0f, (float)wz * 0.37f - 500.0f) > 0.42f && ((fh >> 16) & 15) == 0) {
                            static const uint16_t FL[5] = {Mat::FLOWER, Mat::FLOWER_RED, Mat::CORNFLOWER, Mat::OXEYE_DAISY, Mat::ALLIUM};
                            veg = Mat::TALL_GRASS;
                            veg_h = 2;
                            veg_head = FL[h3(wx >> 5, wz >> 5, g.seed_ + 37) % 5];
                        }
                    }
                }
            }
            if (ncs > 0 || nct > 0) {
                // a plant under a trunk, root, boulder or log is gone
                float ln = 2.0f;
                for (int k = 0; k < nct && veg != Mat::AIR; k++) if (tree_voxel(ctrees[k], (float)wx, h, (float)wz, 0.0f, ln) != Mat::AIR) veg = veg_head = Mat::AIR;
                for (int k = 0; k < ncs && veg != Mat::AIR; k++) if (stamp_voxel(*cst[k], (float)wx, h, (float)wz, 0.0f) != Mat::AIR) veg = veg_head = Mat::AIR;
            }
            // fallen sticks lie on this column's ground, on a strip the grass and ferns keep clear of
            if (do_veg && ncs > 0 && !pickup && h > water_level && hy.fall_top < 0) {
                float sd = 1e9f;
                for (int k = 0; k < ncs; k++) if (cst[k]->kind == STICK) sd = std::min(sd, stick_dist(*cst[k], (float)wx, (float)wz));
                if (sd <= 0.72f) { veg = Mat::TWIGS; veg_h = 1; veg_head = Mat::AIR; }
                else if (sd <= 2.5f) veg = veg_head = Mat::AIR;
                // a rim of bare soil under and beside it, so the pale stick reads against grass and litter
                if (sd <= 1.6f && (top_m == Mat::GRASS || top_m == Mat::LEAF_LITTER)) top_m = Mat::DIRT;
            }
            // ruin walls (macro geometry: 0.5 m stones)
            int wall_top = -1;
            uint32_t wall_seed = 0;
            bool rubble = false;
            if (ruin) {
                const int ax = std::abs(mx - ruin->x), az = std::abs(mz - ruin->z);
                const bool on_perimeter = (ax == ruin->hw && az <= ruin->hd) || (az == ruin->hd && ax <= ruin->hw);
                const bool corner = ax == ruin->hw && az == ruin->hd;
                if (on_perimeter) {
                    const int seg = corner ? 99 : ((ax == ruin->hw) ? (mz - ruin->z + ruin->hd) / 4 + 100 * (mx > ruin->x) : (mx - ruin->x + ruin->hw) / 4 + 200 * (mz > ruin->z));
                    const uint32_t sh = h3((int)ruin->seed, seg, 7);
                    int hgt = corner ? 6 + (int)(sh % 4) : (int)(sh % 7);
                    if (!corner && ((sh >> 8) & 7) == 0) hgt = 0;
                    wall_top = ruin->base + hgt;
                    wall_seed = h3(mx, mz, (int)ruin->seed);
                } else if (!WorldGen::in_ruin(*ruin, mx, mz, 0)) {
                    rubble = (h3(mx, mz, (int)ruin->seed + 5) & 15) == 0;
                }
            }
            // cave mask for the column, one voxel beyond the block on both ends
            const int no_cave_above = (hy.lake || hy.river > 0.0f || hy.marsh) ? water_level - 40 * FS : -1;
            const int cave_band = lod == 0 ? 1 << 30 : CAVE_MOUTH_DEPTH;
            if (!cav.empty()) {
                const float fx = (float)(wx - lx0) / FS, fz = (float)(wz - lz0) / FS;
                const int ci = std::min((int)fx, nx - 2), cj2 = std::min((int)fz, nz - 2);
                const float ctx = fx - ci, ctz = fz - cj2;
                const float *p00 = &cav[((size_t)cj2 * nx + ci) * cny], *p10 = &cav[((size_t)cj2 * nx + ci + 1) * cny];
                const float *p01 = &cav[((size_t)(cj2 + 1) * nx + ci) * cny], *p11 = &cav[((size_t)(cj2 + 1) * nx + ci + 1) * cny];
                for (int y = -1; y <= sy; y++) {
                    const int wy = origin.y + y * step;
                    bool c = wy >= 12 * FS && wy < h - 5 * FS && h - wy <= cave_band && (no_cave_above < 0 || wy < no_cave_above) && !(wy < sea && h < sea + 2 * FS);
                    if (c) {
                        const float fy = (float)wy / FS - (float)cy0;
                        const int k = std::clamp((int)std::floor(fy), 0, cny - 2);
                        const float ty = std::clamp(fy - (float)k, 0.0f, 1.0f);
                        auto lerp_y = [&](const float *p) { return p[k] + (p[k + 1] - p[k]) * ty; };
                        c = bilerp(lerp_y(p00), lerp_y(p10), lerp_y(p01), lerp_y(p11), ctx, ctz) > 0.0f;
                    }
                    cave[y + 1] = c;
                }
            } else {
                std::fill(cave.begin(), cave.end(), 0);
            }
            uint16_t *colp = w + sy * (x + sx * z);
            for (int y = 0; y < sy; y++) {
                const int wy = origin.y + y * step;
                uint16_t mat = Mat::AIR;
                if (ruin) {
                    const int my = floor_div(wy, FS);
                    if (my >= ruin->base && my <= ruin->base + 10 && WorldGen::in_ruin(*ruin, mx, mz, 2)) {
                        if (my == ruin->base && WorldGen::in_ruin(*ruin, mx, mz, 0)) mat = (h3(mx, mz, 3) & 3) ? Mat::STONE : Mat::GRAVEL_STATIC;
                        else if (wall_top >= 0 && my > ruin->base && my <= wall_top) {
                            const uint32_t vh = h3(my, (int)wall_seed, 11);
                            mat = (vh & 15) < (uint32_t)(2 + (my - ruin->base)) ? Mat::AIR : Mat::STONE;
                        } else if (rubble && my == ruin->base + 1) mat = Mat::STONE;
                        else if (wy < h) mat = top_m;
                        colp[y] = mat;
                        if (mat != Mat::AIR) any = true;
                        continue;
                    }
                }
                float ln = 2.0f;
                int owner = -1;
                for (int k = 0; k < nct && mat == Mat::AIR; k++) {
                    mat = tree_voxel(ctrees[k], (float)wx, wy, (float)wz, grow, ln);
                    if (mat != Mat::AIR) owner = k;
                }
                // Trees never weld: wood within a voxel of an earlier tree's wood stays empty, so a limb
                // that grows into the next trunk stops short of it and a felled tree is not held up by
                // its neighbours (P12, rigid pieces).
                if (owner > 0 && (mat == ctrees[owner].t->log || mat == ctrees[owner].t->bark))
                    for (int j = 0; j < owner; j++) {
                        float lj = 2.0f;
                        const uint16_t o = tree_voxel(ctrees[j], (float)wx, wy, (float)wz, grow + 1.0f, lj);
                        if (o == ctrees[j].t->log || o == ctrees[j].t->bark) {
                            mat = Mat::AIR;
                            break;
                        }
                    }
                if (mat == Mat::AIR && wy >= h - 8)
                    for (int k = 0; k < ncs && mat == Mat::AIR; k++) mat = stamp_voxel(*cst[k], (float)wx, wy, (float)wz, grow);
                if (mat != Mat::AIR) { colp[y] = mat; any = true; continue; }
                if (wy < 3 * FS) {
                    mat = Mat::BEDROCK;
                } else if (wy < h) {
                    // a coarse cell covering [wy, wy + step) is classified by what it contains (ADR 0017)
                    const int cell_top = std::min(h - 1, wy + step - 1);
                    const int d_top = h - cell_top;
                    const int n_solid = cell_top - wy + 1;
                    const int n_soil = std::clamp(soil + 1 - d_top, 0, n_solid);
                    const int depth = h - wy;
                    if (cave[y + 1]) {
                        mat = (wy < sea - 180 * FS) ? Mat::LAVA : Mat::AIR;
                    } else if (d_top > 1 && n_soil * 2 < n_solid) {
                        if (d_top <= sandstone_depth) mat = strata ? g.strata_at(mx, floor_div(wy, FS), mz) : Mat::SANDSTONE;
                        else if (vein_ht > 0.0f) {
                            // GregTech layers in the lens, 25 cm grains
                            using namespace technosim_ore;
                            const float dy = (float)wy / FS - (float)vein.y;
                            mat = Mat::STONE;
                            if (dy <= vein_ht && dy >= -vein_ht) {
                                const uint32_t hh = h3(wx >> 1, wz >> 1, (wy >> 1) * 1031 + g.seed_ + 7759);
                                if ((float)(hh & 1023) < DENSITY[vein.type] * 1024.0f) {
                                    if ((float)((hh >> 10) & 1023) < SPORADIC_CHANCE * 1024.0f) mat = (uint16_t)SPORADIC[vein.type];
                                    else {
                                        const float t = (dy + vein_ht) / (2.0f * vein_ht);
                                        mat = (uint16_t)(t > 0.6667f ? PRIMARY[vein.type] : (t < 0.3333f ? SECONDARY[vein.type] : BETWEEN[vein.type]));
                                    }
                                }
                            }
                        } else mat = Mat::STONE;
                        if (spike > 0 && wy >= h - spike) mat = spire_m == Mat::TERRACOTTA ? g.strata_at(mx, floor_div(wy, FS), mz) : spire_m;
                        if (cave[y + 2] && mat == Mat::STONE) {
                            // cave floor: the 3D cave biome decides what you stand on
                            const float cn = g.n_cavebiome_.GetNoise((float)mx, (float)wy / FS, (float)mz);
                            const int di = technosim_biome::band_of(technosim_biome::DEPTH_BAND_MAX, (float)depth / FS, technosim_biome::DEPTH_BAND_COUNT);
                            const int ni = technosim_biome::band_of(technosim_biome::CAVE_NOISE_BAND_MAX, cn, technosim_biome::CAVE_NOISE_BAND_COUNT);
                            const int cb = depth < technosim_biome::SOIL_DEPTH * FS ? col.biome : technosim_biome::cave_biome(ti, hi, di, ni);
                            mat = (uint16_t)technosim_biome::SURFACE_TOP[cb];
                            // the biome's pool (biomes.json surface.pool: water, toxic sludge) lies in the floor in patches
                            const int pool = technosim_biome::SURFACE_POOL[cb];
                            if (pool >= 0 && cave[y] == 0 && n_pool_.GetNoise((float)wx, (float)wz) > 0.35f) mat = (uint16_t)pool;
                        } else if (cave[y] && mat == Mat::STONE && depth >= technosim_biome::SOIL_DEPTH * FS) {
                            const float cn = g.n_cavebiome_.GetNoise((float)mx, (float)wy / FS, (float)mz);
                            const int di = technosim_biome::band_of(technosim_biome::DEPTH_BAND_MAX, (float)depth / FS, technosim_biome::DEPTH_BAND_COUNT);
                            const int ni = technosim_biome::band_of(technosim_biome::CAVE_NOISE_BAND_MAX, cn, technosim_biome::CAVE_NOISE_BAND_COUNT);
                            const int ceil_m = technosim_biome::SURFACE_CEILING[technosim_biome::cave_biome(ti, hi, di, ni)];
                            if (ceil_m >= 0) mat = (uint16_t)ceil_m;
                        }
                    } else {
                        mat = d_top == 1 ? (far_canopy ? far_leaves : top_m) : under_m;
                        if (mat == Mat::TERRACOTTA) mat = g.strata_at(mx, floor_div(wy, FS), mz);
                    }
                } else if ((water_level - wy) * 2 >= step) {
                    const bool inland = hy.lake || hy.river > 0.0f || hy.marsh || side_fill;
                    if (wy + step >= water_level && cold && (hy.lake || hy.marsh)) mat = Mat::ICE;
                    else if (wy + step >= water_level && !inland && technosim_biome::SURFACE_ICE[col.biome] >= 0) mat = (uint16_t)technosim_biome::SURFACE_ICE[col.biome];
                    else mat = inland ? Mat::WATER_STATIC : Mat::WATER;
                } else if (hy.fall_top * FS > wy) {
                    mat = Mat::WATER_STATIC;   // waterfall sheet hanging from the lip
                } else if (far_canopy && wy < far_top) {
                    mat = far_leaves;
                } else if (veg != Mat::AIR && wy < h + veg_h) {
                    mat = veg;
                } else if (veg_head != Mat::AIR && wy == h + veg_h) {
                    mat = veg_head;
                }
                colp[y] = mat;
                if (mat != Mat::AIR) any = true;
            }
        }
    }
    if (!any) return out;
    const int bpv = (bytes_per_voxel == 1 || bytes_per_voxel == 4) ? bytes_per_voxel : 2;
    out.resize(n * bpv);
    uint8_t *o = out.ptrw();
    if (bpv == 2) {
        std::memcpy(o, w, n * 2);
    } else if (bpv == 1) {
        for (int i = 0; i < n; i++) o[i] = (uint8_t)std::min<uint16_t>(w[i], 255);
    } else {
        std::memset(o, 0, n * 4);
        for (int i = 0; i < n; i++) { o[i * 4] = (uint8_t)(w[i] & 0xFF); o[i * 4 + 1] = (uint8_t)(w[i] >> 8); }
    }
    return out;
}

// ---------- maps and queries (fine coordinates in and out) ----------

PackedByteArray FineGen::map_image(int center_x, int center_z, int radius, int step) const {
    return g_->map_image(floor_div(center_x, FS), floor_div(center_z, FS), std::max(1, radius / FS), std::max(1, step / FS));
}

PackedByteArray FineGen::climate_map(int x0, int z0, int n, int step) const {
    return g_->climate_map(floor_div(x0, FS), floor_div(z0, FS), n, std::max(1, step / FS));
}

PackedInt32Array FineGen::water_floor_map(int x0, int z0, int n, int step) const {
    PackedInt32Array m = g_->water_floor_map(floor_div(x0, FS), floor_div(z0, FS), n, std::max(1, step / FS));
    int32_t *p = m.ptrw();
    for (int i = 0; i < m.size(); i++)
        if (p[i] >= 0) p[i] = ((p[i] & 0xFFFF) * FS) | (((p[i] >> 16) * FS) << 16);
    return m;
}

Vector3i FineGen::find_nearest(const String &kind, int x, int z, int max_radius) const {
    const Vector3i r = g_->find_nearest(kind, floor_div(x, FS), floor_div(z, FS), std::max(1, max_radius / FS));
    if (r.y < 0) return r;
    const int fx = r.x * FS + 2, fz = r.z * FS + 2;
    // surface features stand on the fine surface; caves keep their depth
    const int surf = height_at(fx, fz), y = r.y * FS;
    return Vector3i(fx, y < surf - 2 * FS ? y : surf, fz);
}

Array FineGen::waterfalls_near(int x, int z, int radius) const {
    Array a = g_->waterfalls_near(floor_div(x, FS), floor_div(z, FS), std::max(1, radius / FS));
    for (int i = 0; i < a.size(); i++) {
        Dictionary d = a[i];
        d["x"] = (int)d["x"] * FS + 2;
        d["z"] = (int)d["z"] * FS + 2;
        d["y"] = (int)d["y"] * FS;
        d["top"] = (int)d["top"] * FS;
        d["width"] = (float)d["width"] * FS;
    }
    return a;
}

Dictionary FineGen::hydrology_stats(int x, int z) const { return g_->hydrology_stats(floor_div(x, FS), floor_div(z, FS)); }

Dictionary FineGen::vein_at(int x, int z) const {
    Dictionary d = g_->vein_at(floor_div(x, FS), floor_div(z, FS));
    if (d.has("x")) {
        d["x"] = (int)d["x"] * FS + 2;
        d["y"] = (int)d["y"] * FS;
        d["z"] = (int)d["z"] * FS + 2;
        d["half_thickness"] = (float)d["half_thickness"] * FS;
    }
    return d;
}
