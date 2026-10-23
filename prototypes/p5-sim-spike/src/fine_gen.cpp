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
// Ground noise on dry land (fine voxels): lumps and hollows the 0.5 m fields cannot make.
constexpr float GROUND_AMP = 1.4f;
constexpr float GROUND_AMP2 = 0.6f;
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
    setup_noise(n_ground_, p_seed + 101, 1.0f / 40.0f, 2);   // 5 m lumps (fine voxels)
    setup_noise(n_ground2_, p_seed + 102, 1.0f / 9.0f, 1);   // ~1 m roughness
    setup_noise(n_leaf_, p_seed + 103, 1.0f / 7.0f, 1);
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
    return m;
}

float FineGen::ground_detail(int x, int z, const MacroPt &m) const {
    if (m.lakebed || m.riverbed || m.marshbed || m.ruin >= 0 || m.hy.river > 0.05f || m.spike > 0) return 0.0f;
    return n_ground_.GetNoise((float)x, (float)z) * GROUND_AMP + n_ground2_.GetNoise((float)x, (float)z) * GROUND_AMP2;
}

float FineGen::surface_f(int x, int z) const {
    const int mx = floor_div(x, FS), mz = floor_div(z, FS);
    WorldGen::Ruin ruins[4];
    const int nr = g_->ruins_near(mx - 1, mz - 1, mx + 2, mz + 2, ruins, 4);
    const MacroPt a = macro_point(mx, mz, ruins, nr), b = macro_point(mx + 1, mz, ruins, nr);
    const MacroPt c = macro_point(mx, mz + 1, ruins, nr), d = macro_point(mx + 1, mz + 1, ruins, nr);
    const float tx = (float)(x - mx * FS) / FS, tz = (float)(z - mz * FS) / FS;
    const MacroPt &near = tz < 0.5f ? (tx < 0.5f ? a : b) : (tx < 0.5f ? c : d);
    return bilerp(a.hc, b.hc, c.hc, d.hc, tx, tz) * FS + ground_detail(x, z, near);
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

void FineGen::build_tree(const WorldGen::Tree &t, FTree &o) const {
    const int sd = g_->seed_;
    o.seed = h3(t.x, t.z, sd + 77);
    o.x = (float)(t.x * FS + 2) + (float)((int)(o.seed & 3) - 1) * 0.5f;
    o.z = (float)(t.z * FS + 2) + (float)((int)((o.seed >> 2) & 3) - 1) * 0.5f;
    o.ground = (int)std::floor(surface_f((int)o.x, (int)o.z) + 0.5f);
    o.base = o.ground - 6;
    o.top = o.ground + (WorldGen::trunk_top(t) - t.base) * FS + (int)((o.seed >> 4) % 3);
    o.kind = t.kind;
    o.log = WorldGen::tree_log(t.kind);
    o.leaves = WorldGen::tree_leaves(t.kind);
    o.nb = o.ns = 0;
    o.cone_r = 0.0f;
    const float R = (float)(t.radius * FS);
    const float top = (float)o.top;
    auto rnd = [&](int k) { return (float)(h3((int)o.seed, k, sd + 79) & 1023) / 1023.0f; };
    switch (t.kind) {
        case WorldGen::TREE_SPRUCE:
            o.r = 1.4f;
            o.cone_lo = o.ground + 3 * FS;
            o.cone_hi = o.top + 2 * FS;
            o.cone_r = R + 2.5f;
            break;
        case WorldGen::TREE_BIRCH:
            o.r = 1.1f;
            for (int i = 0; i < 3; i++)
                o.blobs[o.nb++] = {o.x + (rnd(i) - 0.5f) * 2.0f, top - FS * 2.5f + i * FS * 1.4f, o.z + (rnd(i + 9) - 0.5f) * 2.0f, R * 0.8f + 1.5f, FS * 1.3f, R * 0.8f + 1.5f};
            break;
        case WorldGen::TREE_ACACIA: {
            o.r = 1.3f;
            const float a = rnd(1) * 6.2832f;
            const float ox = std::cos(a) * R * 0.6f, oz = std::sin(a) * R * 0.6f;
            o.segs[o.ns++] = {o.x, top - FS * 2.0f, o.z, o.x + ox, top + 1.0f, o.z + oz, 1.2f};
            o.segs[o.ns++] = {o.x, top - FS * 1.5f, o.z, o.x - ox * 0.8f, top - 1.0f, o.z - oz * 0.8f, 1.0f};
            o.blobs[o.nb++] = {o.x + ox, top + 2.0f, o.z + oz, R * 1.1f + 3.0f, 2.2f, R * 1.1f + 3.0f};
            o.blobs[o.nb++] = {o.x - ox * 0.8f, top, o.z - oz * 0.8f, R * 0.7f + 2.0f, 1.8f, R * 0.7f + 2.0f};
            break;
        }
        default: {   // oak and jungle: a crown blob over the trunk, side blobs on branches
            const bool jungle = t.kind == WorldGen::TREE_JUNGLE;
            o.r = jungle ? 2.2f + 0.3f * (float)(t.radius - 3) : (t.radius >= 3 ? 2.0f : 1.6f);
            o.blobs[o.nb++] = {o.x, top - 1.0f, o.z, R + 2.0f, R * 0.65f + 2.0f, R + 2.0f};
            const int k = 3 + (int)(o.seed >> 8) % 3;
            const float a0 = rnd(2) * 6.2832f;
            for (int i = 0; i < k && o.nb < 8 && o.ns < 8; i++) {
                const float a = a0 + i * 6.2832f / k + (rnd(10 + i) - 0.5f) * 0.8f;
                const float dist = R * (0.7f + 0.3f * rnd(20 + i));
                const float by = top - FS * (jungle ? 1.0f : 1.8f) + rnd(30 + i) * FS * 1.5f;
                const float bx = o.x + std::cos(a) * dist, bz = o.z + std::sin(a) * dist;
                const float br = R * 0.55f + 1.5f;
                o.blobs[o.nb++] = {bx, by, bz, br, br * 0.75f, br};
                const float sy = top - FS * 3.0f - (float)i;
                o.segs[o.ns++] = {o.x, sy, o.z, o.x + (bx - o.x) * 0.8f, by - 1.0f, o.z + (bz - o.z) * 0.8f, jungle ? 1.3f : 1.0f};
            }
            break;
        }
    }
    float reach = o.r + 3.0f, yhi = top;
    for (int i = 0; i < o.nb; i++) {
        const Blob &b = o.blobs[i];
        reach = std::max(reach, std::max(std::fabs(b.x - o.x), std::fabs(b.z - o.z)) + b.rx);
        yhi = std::max(yhi, b.y + b.ry);
    }
    if (o.cone_r > 0.0f) { reach = std::max(reach, o.cone_r); yhi = std::max(yhi, (float)o.cone_hi); }
    o.reach = reach + 1.0f;
    o.y_hi = (int)std::ceil(yhi) + 1;
}

uint16_t FineGen::tree_voxel(const FTree &t, float x, int y, float z, float grow) const {
    if (y < t.base || y > t.y_hi) return Mat::AIR;
    const float dx = x - t.x, dz = z - t.z, d2 = dx * dx + dz * dz;
    if (y <= t.top) {
        const float frac = std::clamp((float)(y - t.ground) / (float)std::max(1, t.top - t.ground), 0.0f, 1.0f);
        float r = t.r * (1.0f - 0.3f * frac) + grow;
        if (y < t.ground + 6) r += (float)(t.ground + 6 - y) * 0.22f;   // root flare
        if (d2 <= r * r) return t.log;
    }
    const float fy = (float)y;
    for (int i = 0; i < t.ns; i++) {
        const Seg &s = t.segs[i];
        const float r = s.r + grow;
        if (seg_dist2(x, fy, z, s.ax, s.ay, s.az, s.bx, s.by, s.bz) <= r * r) return t.log;
    }
    const int ix = (int)std::floor(x), iz = (int)std::floor(z);
    if (t.cone_r > 0.0f && y >= t.cone_lo && y <= t.cone_hi) {
        const float u = (float)(t.cone_hi - y) / (float)std::max(1, t.cone_hi - t.cone_lo);
        float rr = t.cone_r * u + 0.8f;
        if (((t.cone_hi - y) / 3) & 1) rr *= 0.78f;   // layered boughs
        rr += grow;
        if (d2 <= rr * rr) {
            if (d2 > (rr - 1.5f) * (rr - 1.5f) && (h3(ix, y, iz) & 3) == 0) return Mat::AIR;
            return t.leaves;
        }
    }
    for (int i = 0; i < t.nb; i++) {
        const Blob &b = t.blobs[i];
        const float ex = (x - b.x) / (b.rx + grow), ey = (fy - b.y) / (b.ry + grow), ez = (z - b.z) / (b.rz + grow);
        const float d = ex * ex + ey * ey + ez * ez;
        if (d > 1.15f) continue;
        const float edge = d + n_leaf_.GetNoise(x, fy, z) * 0.35f;
        if (edge > 1.0f) continue;
        if (edge > 0.7f && (h3(ix, y, iz) % 5) == 0) continue;   // ragged, see-through edge
        return t.leaves;
    }
    return Mat::AIR;
}

void FineGen::build_stamp(const WorldGen::Stamp &s, FStamp &o) const {
    o.kind = s.kind;
    o.seed = s.seed;
    o.x = (float)(s.x * FS + 2);
    o.z = (float)(s.z * FS + 2);
    o.ground = (int)std::floor(surface_f(s.x * FS + 2, s.z * FS + 2) + 0.5f);
    o.mossy = s.mossy;
    o.along_x = (s.seed >> 9) & 1;
    o.wood = WorldGen::tree_log(s.tree_kind);
    o.bark = WorldGen::tree_bark(s.tree_kind);
    o.leaves = WorldGen::tree_leaves(s.tree_kind);
    switch (s.kind) {
        case WorldGen::STAMP_BOULDER: o.size = ((float)s.size + 0.4f) * FS * 0.9f; break;
        case WorldGen::STAMP_BUSH: o.size = (float)s.size * FS + 1.5f; break;
        case WorldGen::STAMP_FALLEN_LOG: o.size = (float)s.size * FS; break;
        default: o.size = (float)s.size * FS; break;
    }
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
            if (e > 1.0f + n_leaf_.GetNoise(x, fy, z) * 0.3f) return Mat::AIR;
            if (dx * dx + dz * dz < 2.0f && y < s.ground + 2) return s.wood;
            return s.leaves;
        }
        case WorldGen::STAMP_FALLEN_LOG: {
            const float a = s.along_x ? dx : dz, b = s.along_x ? dz : dx;
            const float cy = (float)s.ground + 1.4f;
            const float r = 1.7f + grow;
            if (std::fabs(a) > s.size * 0.5f) return Mat::AIR;
            const float d2 = b * b + (fy - cy) * (fy - cy);
            if (d2 <= r * r) return s.bark;
            if (s.mossy && d2 <= (r + 1.0f) * (r + 1.0f) && fy > cy + r - 0.5f && (h3((int)x, (int)z, (int)s.seed + 3) % 3) == 0) return Mat::MOSS;
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
    static constexpr int MAXT = 64, MAXS = 32;
    WorldGen::Tree mtrees[MAXT];
    std::vector<FTree> trees;
    std::vector<FStamp> stamps;
    const int x_lo = origin.x, x_hi = origin.x + (sx - 1) * step, z_lo = origin.z, z_hi = origin.z + (sz - 1) * step;
    int feature_top = 0;
    if (above_sea && origin.y < (hmax_m + 40) * FS) {
        if (lod <= 3) {
            const int nt = g.trees_near(floor_div(x_lo, FS) - 4, floor_div(z_lo, FS) - 4, floor_div(x_hi, FS) + 4, floor_div(z_hi, FS) + 4, mtrees, MAXT);
            for (int i = 0; i < nt; i++) {
                bool blocked = false;
                for (int r = 0; r < nruins && !blocked; r++) blocked = WorldGen::in_ruin(ruins[r], mtrees[i].x, mtrees[i].z, 3);
                if (blocked) continue;
                FTree t;
                build_tree(mtrees[i], t);
                if (t.x + t.reach < x_lo || t.x - t.reach > x_hi || t.z + t.reach < z_lo || t.z - t.reach > z_hi) continue;
                if (t.y_hi < origin.y || t.base >= y_top) continue;
                feature_top = std::max(feature_top, t.y_hi);
                trees.push_back(t);
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
                feature_top = std::max(feature_top, s.ground + (int)s.size + 8);
                stamps.push_back(s);
            }
        }
    }
    // Nothing reaches this block: all air.
    const int reach_top = std::max({(hmax_m + 12) * FS, wl_max * FS + 1, fall_max * FS + 1, feature_top + 1, (hmax_m + 1) * FS + 12});
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
            const float hf = bilerp(c00.hc, c10.hc, c01.hc, c11.hc, tx, tz) * FS + ground_detail(wx, wz, m);
            const int h = std::clamp((int)std::floor(hf + 0.5f), 8 * FS, world_h - 8 * FS);
            // real slope of the interpolated surface (fine voxels per fine voxel)
            const float gx = ((c10.hc - c00.hc) * (1.0f - tz) + (c11.hc - c01.hc) * tz) * FS / L;
            const float gz = ((c01.hc - c00.hc) * (1.0f - tx) + (c11.hc - c10.hc) * tx) * FS / L;
            const float slope = std::max({std::fabs(gx), std::fabs(gz), m.col.slope * 0.75f});
            const WorldGen::Column &col = m.col;
            const WorldGen::Hydro &hy = m.hy;
            const int water_level = m.water_level * FS;
            const bool lakebed = m.lakebed && h < water_level, riverbed = m.riverbed, marshbed = m.marshbed;
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
            if (do_veg && !ruin && h >= water_level && hy.fall_top < 0 && h > sea && spike == 0 && !lakebed && !riverbed) {
                int mh = 1;
                uint16_t kind = g.vegetation_at(col, mx, mz, top_m, hy, mh);
                if (has_vein && (kind == Mat::AIR || kind == Mat::TALL_GRASS)) {
                    const uint16_t ind = g.indicator_at(vein, mx, mz, h / FS, top_m);
                    if (ind != Mat::AIR) kind = ind;
                }
                const int lx = wx - mx * FS, lz = wz - mz * FS;
                const uint32_t hv = h3(wx, wz, g.seed_ + 5);
                const bool centre = lx == 1 && lz == 2;
                const bool grassy = top_m == Mat::GRASS;
                switch (kind) {
                    case Mat::AIR: break;
                    case Mat::TALL_GRASS:
                        if ((hv & 7) < 3) { veg = Mat::TALL_GRASS; veg_h = 1 + (int)((hv >> 3) % 3); }
                        break;
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
                    case Mat::BLUEBERRY_BUSH:
                        if (lx < 3 && lz < 3 && (hv & 3) != 0) { veg = Mat::BLUEBERRY_BUSH; veg_h = 1 + (int)((hv >> 2) & 1); }
                        break;
                    default:
                        if (is_flower(kind)) {
                            if (centre) { veg = Mat::TALL_GRASS; veg_h = 2; veg_head = kind; }
                            else if (grassy && (hv & 7) < 2) { veg = Mat::TALL_GRASS; veg_h = 1; }
                        } else if (centre) {
                            veg = kind; veg_h = 1;   // pickups, mushrooms, wild crops, carcasses: one voxel
                        } else if (grassy && (hv & 7) < 2) {
                            veg = Mat::TALL_GRASS; veg_h = 1;
                        }
                        break;
                }
            }
            // far LOD: a leaf-coloured surface under a tree's crown
            bool far_canopy = false;
            uint16_t far_leaves = Mat::LEAVES;
            if (lod >= 4 && above_sea && !ruin && h > water_level && technosim_biome::TREE_DENSITY[col.biome] > 0.0f) {
                WorldGen::Tree t;
                const int tcx = floor_div(mx, WorldGen::TREE_CELL), tcz = floor_div(mz, WorldGen::TREE_CELL);
                if (g.tree_at_cell(tcx, tcz, t)) {
                    const int dx = mx - t.x, dz = mz - t.z;
                    far_canopy = dx * dx + dz * dz <= t.radius * t.radius + 2;
                    far_leaves = WorldGen::tree_leaves(t.kind);
                }
            }
            // trees and decorations whose bounds touch this column
            const FTree *ctrees[16];
            int nct = 0;
            for (const FTree &t : trees) {
                if (nct >= 16) break;
                if (std::fabs((float)wx - t.x) <= t.reach + step && std::fabs((float)wz - t.z) <= t.reach + step) ctrees[nct++] = &t;
            }
            const FStamp *cst[8];
            int ncs = 0;
            if (!ruin && spike == 0 && !lakebed && !riverbed && h >= water_level - FS) {
                for (const FStamp &s : stamps) {
                    if (ncs >= 8) break;
                    if (std::fabs((float)wx - s.x) <= s.size * 1.3f + 6 && std::fabs((float)wz - s.z) <= s.size * 1.3f + 6) cst[ncs++] = &s;
                }
            }
            if (ncs > 0 || nct > 0) {
                // a plant under a trunk, boulder or log is gone
                for (int k = 0; k < nct && veg != Mat::AIR; k++) if (tree_voxel(*ctrees[k], (float)wx, h, (float)wz, 0.0f) != Mat::AIR) veg = Mat::AIR;
                for (int k = 0; k < ncs && veg != Mat::AIR; k++) if (stamp_voxel(*cst[k], (float)wx, h, (float)wz, 0.0f) != Mat::AIR) veg = Mat::AIR;
            }
            const float grow = step > 1 ? (float)step * 0.45f : 0.0f;
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
                for (int k = 0; k < nct && mat == Mat::AIR; k++) mat = tree_voxel(*ctrees[k], (float)wx, wy, (float)wz, grow);
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
                    const bool inland = hy.lake || hy.river > 0.0f || hy.marsh;
                    if (wy + step >= water_level && cold && (hy.lake || hy.marsh)) mat = Mat::ICE;
                    else if (wy + step >= water_level && !inland && technosim_biome::SURFACE_ICE[col.biome] >= 0) mat = (uint16_t)technosim_biome::SURFACE_ICE[col.biome];
                    else mat = inland ? Mat::WATER_STATIC : Mat::WATER;
                } else if (hy.fall_top * FS > wy) {
                    mat = Mat::WATER_STATIC;   // waterfall sheet hanging from the lip
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
    return Vector3i(fx, height_at(fx, fz), fz);
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
