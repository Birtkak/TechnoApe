#include "fine_gen.h"

#include <godot_cpp/core/class_db.hpp>

#include <algorithm>
#include <atomic>
#include <chrono>
#include <cmath>
#include <cstring>
#include <vector>

using namespace godot;

namespace {
constexpr int FS = FineGen::FS;
// Ore lumps in a vein's lens (ADR 0064): Voronoi lumps around jittered points NUGGET_CELL voxels apart,
// squashed NUGGET_FLAT times in y (ore lies in the flat lens), parted by a seam of host rock where the
// two nearest points are within NUGGET_SEAM of each other (in that squashed metric), so a lump digs out
// as one piece (ADR 0047). POCKET_* shape the field that bunches the lumps: rich pockets ~4 m across,
// hardly a lump between them (developer 2026-10-28: "make ores bunch up more so they are easier to mine";
// pockets were ~3 m with one lump in eight strewn between; about the same amount of ore, in solid masses).
constexpr int NUGGET_CELL = 7;   // 5 until 2026-10-28: bigger lumps, thinner share of seams, more ore per dig
constexpr float NUGGET_FLAT = 1.35f;
constexpr float NUGGET_SEAM = 1.0f;
constexpr float POCKET_XZ = 34.0f, POCKET_Y = 16.0f;   // pocket field lattice, voxels
constexpr float POCKET_RICH = 3.0f;    // lump chance in a pocket, times the vein's density and the host rock's bias
constexpr float POCKET_STRAY = 0.03f; // ... and between pockets
// Soil under the top voxel: about a metre of the biome's under material.
constexpr int SOIL_FINE = 8;
// Ground relief on dry land (fine voxels): swells, hummocks and roughness the 0.5 m fields cannot
// make, faded out toward water, rivers and ruins (MacroPt::gw).
constexpr float GROUND_AMP = 3.5f;    // ~8 m swells
constexpr float HUMP_AMP = 1.5f;      // ~2.5 m hummocks
constexpr float GROUND_AMP2 = 0.6f;   // ~1 m roughness
// At lod 1 caves are carved only this close to the surface (cave mouths, ADR 0020).
constexpr int CAVE_MOUTH_DEPTH = 24 * FS;
// Caverns and ravines are carved up to this LOD (to ~120 m from the camera), so a big hall has walls past the
// 24 m full-detail edge instead of a hole into nothing (ADR 0052).
constexpr int BIG_CAVE_LOD = 2;
// The cave field is read once per CAVE_STEP voxels up (at the middle of each band), so cave floors, walls and
// ceilings change height in ledges of 0.5 m instead of single-voxel stairs, and floors have flat treads that
// hold bigger pools (bug report 2026-10-25 21-23-02). Bands are aligned to the world grid: neighbouring
// columns share their levels.
constexpr int CAVE_STEP = 4;
// The tallest plant a column grows on its ground (reeds, cacti, tall grass): a block this far over the ground
// still decides whether those plants stand over an opened cave top.
constexpr int VEG_REACH = 4 * FS;
// Jungle vines hang up to this many voxels under a leaf clump.
constexpr int VINE_MAX = 18;
// Cave pools: where n_pool_ (~25 m blobs) is above this, a flat stretch of cave floor holds its biome's pool.
constexpr float POOL_THR = 0.0f;
// Ore stringers (ADR 0077, 0086): sheets where a value noise sits within STRINGER_HALF of its middle (two to three
// voxels thick), from STRINGER_MIN_DEPTH down, within STRINGER_REACH voxels of cave air. Ore lies only in the sheet's
// clumps (a value noise STRINGER_CLUMP voxels across over a threshold), not in single voxels picked one by one (bug
// report 2026-10-27 17-49-40); the sheet is 2.5 times the old one and the clumps hold 1/2.5 of it, so the amount of
// ore stays as before.
constexpr float STRINGER_HALF = 0.03f;    // 0.012 with per-voxel picks; 0.022 that way sugar-coated the shafts
constexpr float STRINGER_CLUMP = 6.0f;   // 3.5 until 2026-10-28: clumps ~75 cm across, so a strike is a few blocks of ore
constexpr int STRINGER_MIN_DEPTH = 40 * FS;   // 20 m
constexpr int STRINGER_REACH = 12;            // 1.5 m
// Fallen sticks' half-widths in voxels (FStamp::r): thin (one voxel), medium (two), thick (two
// across, two tall along the middle).
constexpr float STICK_THICK[3] = {0.72f, 1.05f, 1.45f};
// Below this many voxels under the lowest carved surface of a block's columns nothing but stone is
// generated from LOD 1 on: past the deepest sandstone and badlands strata (24 macro), the cave mouths
// (CAVE_MOUTH_DEPTH) and the ground's swells, with room to spare.
constexpr int OPEN_CAVE_DEPTH = 160 * FS;   // 80 m: ground_range's reach under a cave opening (ADR 0105)
constexpr int DEEP_ROCK_MARGIN = (24 * FS > CAVE_MOUTH_DEPTH ? 24 * FS : CAVE_MOUTH_DEPTH) + 24 * FS;

inline int floor_div(int a, int b) { return a >= 0 ? a / b : -((-a + b - 1) / b); }
inline float cave_band_y(int wy) { return (float)(floor_div(wy, CAVE_STEP) * CAVE_STEP) + 0.5f * (float)(CAVE_STEP - 1); }
inline float smoothstep(float a, float b, float x) {
    const float t = std::clamp((x - a) / (b - a), 0.0f, 1.0f);
    return t * t * (3.0f - 2.0f * t);
}
inline float bilerp(float a00, float a10, float a01, float a11, float tx, float tz) {
    return (a00 * (1.0f - tx) + a10 * tx) * (1.0f - tz) + (a01 * (1.0f - tx) + a11 * tx) * tz;
}
inline uint32_t h3(int a, int b, int c) { return WorldGen::hash3(a, b, c); }
// Smooth value noise, 0..1, on the integer lattice of (x, y, z).
// The 8 lattice corners of the last cell value_noise3 read (a column walks up through the same cell for several voxels).
struct ValueNoiseCell {
    int ix = INT32_MIN, iy = 0, iz = 0, seed = 0;
    float c[8];   // corner (a, b, c) at c[a + 2 * b + 4 * c]
};
inline float value_noise3(float x, float y, float z, int seed, ValueNoiseCell &cell) {
    const int ix = (int)std::floor(x), iy = (int)std::floor(y), iz = (int)std::floor(z);
    float fx = x - (float)ix, fy = y - (float)iy, fz = z - (float)iz;
    fx = fx * fx * (3.0f - 2.0f * fx);
    fy = fy * fy * (3.0f - 2.0f * fy);
    fz = fz * fz * (3.0f - 2.0f * fz);
    if (cell.ix != ix || cell.iy != iy || cell.iz != iz || cell.seed != seed) {
        cell.ix = ix, cell.iy = iy, cell.iz = iz, cell.seed = seed;
        for (int k = 0; k < 8; k++)
            cell.c[k] = (float)(h3(ix + (k & 1), iz + (k >> 2), (iy + ((k >> 1) & 1)) * 1031 + seed) & 0xFFFF) / 65535.0f;
    }
    auto v = [&](int a, int b, int c) { return cell.c[a + 2 * b + 4 * c]; };
    const float x00 = v(0, 0, 0) + (v(1, 0, 0) - v(0, 0, 0)) * fx, x10 = v(0, 1, 0) + (v(1, 1, 0) - v(0, 1, 0)) * fx;
    const float x01 = v(0, 0, 1) + (v(1, 0, 1) - v(0, 0, 1)) * fx, x11 = v(0, 1, 1) + (v(1, 1, 1) - v(0, 1, 1)) * fx;
    const float y0 = x00 + (x10 - x00) * fy, y1 = x01 + (x11 - x01) * fy;
    return y0 + (y1 - y0) * fz;
}
inline float value_noise3(float x, float y, float z, int seed) {
    ValueNoiseCell cell;
    return value_noise3(x, y, z, seed, cell);
}
// A voxel of a vein's lens (ADR 0064; the caller checks it lies in the lens): host stone or the ore of its
// lump. A lump is ore or not as a whole, decided at its point by the pocket field, and takes its layer
// (GregTech primary on top, secondary below, between in the middle; ADR 0047) from its point's height,
// with the pocket field wobbling the layer borders; a few lumps are the sporadic ore.
} // namespace
uint16_t FineGen::vein_voxel(const WorldGen::Vein &v, float ht, int wx, int wy, int wz, float bias, int seed) {
    using namespace technosim_ore;
    constexpr int NC = NUGGET_CELL;
    constexpr float J = (float)NC / 256.0f;
    const int cx = floor_div(wx, NC), cy = floor_div(wy, NC), cz = floor_div(wz, NC);
    const float px = (float)wx + 0.5f, py = (float)wy + 0.5f, pz = (float)wz + 0.5f;
    float d0 = 1e30f, d1 = 1e30f, fy = 0.0f, fx = 0.0f, fz = 0.0f;
    int bx = 0, by = 0, bz = 0;
    for (int k = -1; k <= 1; k++)
        for (int j = -1; j <= 1; j++)
            for (int i = -1; i <= 1; i++) {
                const int gx = cx + i, gy = cy + k, gz = cz + j;
                const uint32_t h = h3(gx, gz, gy * 1031 + seed + 7759);
                const float qx = (float)(gx * NC) + ((float)(h & 255) + 0.5f) * J;
                const float qy = (float)(gy * NC) + ((float)((h >> 8) & 255) + 0.5f) * J;
                const float qz = (float)(gz * NC) + ((float)((h >> 16) & 255) + 0.5f) * J;
                const float dx = px - qx, dy = (py - qy) * NUGGET_FLAT, dz = pz - qz;
                const float d = dx * dx + dy * dy + dz * dz;
                if (d < d0) { d1 = d0; d0 = d; fx = qx; fy = qy; fz = qz; bx = gx; by = gy; bz = gz; }
                else if (d < d1) d1 = d;
            }
    if (std::sqrt(d1) - std::sqrt(d0) < NUGGET_SEAM) return WorldGen::STONE;
    const float pocket = 0.65f * value_noise3(fx / POCKET_XZ, fy / POCKET_Y, fz / POCKET_XZ, seed + 7771)
                       + 0.35f * value_noise3(fx / (POCKET_XZ * 0.45f), fy / (POCKET_Y * 0.45f), fz / (POCKET_XZ * 0.45f), seed + 7779);
    // the lens is richest at its heart: a prospector who finds the centre finds ore
    const float cdx = fx - (float)(v.x * FS), cdz = fz - (float)(v.z * FS);
    const float heart = std::max(0.0f, 1.0f - std::sqrt(cdx * cdx + cdz * cdz) / (0.5f * (float)(SIZE[v.type] * FS)));
    const float rich = smoothstep(0.44f, 0.54f, pocket + 0.3f * heart * heart);
    const float p = DENSITY[v.type] * bias * (POCKET_STRAY + (POCKET_RICH - POCKET_STRAY) * rich);
    const uint32_t hl = h3(bx + 7, bz - 3, by * 1031 + seed + 7789);
    if ((float)(hl & 1023) >= std::min(1.0f, p) * 1024.0f) return WorldGen::STONE;
    if ((float)((hl >> 10) & 1023) < SPORADIC_CHANCE * 1024.0f) return (uint16_t)SPORADIC[v.type];
    const float t = (fy / (float)FS - (float)v.y + ht) / (2.0f * ht) + 0.35f * (pocket - 0.5f);
    return (uint16_t)(t > 0.6667f ? PRIMARY[v.type] : (t < 0.3333f ? SECONDARY[v.type] : BETWEEN[v.type]));
}
namespace {
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
    if (kind == WorldGen::FLINT_STONES) {
        // A shard, not a lump (2026-10-26: "looks too much like normal pebbles"): a sharp spine 3
        // voxels tall along x, z or a diagonal, its flanks one voxel high, the ends pinched to points.
        static const float AX[4][2] = {{1.0f, 0.0f}, {0.0f, 1.0f}, {0.7071f, 0.7071f}, {0.7071f, -0.7071f}};
        const int dir = (int)((s >> 20) & 3);
        const float cx = dir >= 2 ? 2.0f : 1.5f + (float)(s & 1), cz = dir >= 2 ? 2.0f : 1.5f + (float)((s >> 1) & 1);
        const float len = 1.9f + 0.5f * (float)((s >> 8) & 15) / 15.0f, wid = 1.35f + 0.25f * (float)((s >> 12) & 15) / 15.0f;
        const float px = (float)lx + 0.5f - cx, pz = (float)lz + 0.5f - cz;
        const float a = std::abs(px * AX[dir][0] + pz * AX[dir][1]) / len;
        const float b = std::abs(-px * AX[dir][1] + pz * AX[dir][0]) / wid;
        if (a > 1.0f || b > 1.0f - 0.55f * a * a) return 0;
        return std::clamp((int)std::ceil(3.0f * (1.0f - b) * (1.0f - 0.45f * a)), 1, 3);
    }
    // An irregular lump over the patch (2026-10-25: "a bit bigger, irregular, variety, thicker"): a
    // dome with its own centre, radii and height per patch (2-3 voxels, loose stones 1-2), its edge
    // eroded cell by cell. Every patch differs; the cells stay touching, so it is one piece.
    const bool loose = kind == WorldGen::PEBBLES;
    const float cx = 1.6f + 0.8f * (float)(s & 15) / 15.0f, cz = 1.6f + 0.8f * (float)((s >> 4) & 15) / 15.0f;
    const float rx = 1.7f + 0.9f * (float)((s >> 8) & 15) / 15.0f, rz = 1.7f + 0.9f * (float)((s >> 12) & 15) / 15.0f;
    const int H = loose ? 1 + (int)((s >> 16) % 3) : 2 + (int)((s >> 16) % 3);   // stones 1-3, chunks and flint 2-4
    const float dx = ((float)lx + 0.5f - cx) / rx, dz = ((float)lz + 0.5f - cz) / rz;
    const float d = dx * dx + dz * dz;
    if (d > 1.0f) return 0;
    const uint32_t hh = WorldGen::hash3(lx, lz, (int)(s >> 3));
    if (d > 0.55f && (hh & 3) == 0) return 0;              // a bite out of the rim
    int h = (int)std::ceil((float)H * (1.0f - d * 0.8f));
    if (h > 1 && ((hh >> 2) & 3) == 0) h--;                  // lumpy top
    return std::clamp(h, 1, H);
}
} // namespace

FineGen::FineGen() {
    g_.instantiate();
    configure(1337, 256 * FS, 1280 * FS);
}

void FineGen::_bind_methods() {
    ClassDB::bind_method(D_METHOD("configure", "seed", "sea_level", "world_height"), &FineGen::configure);
    ClassDB::bind_method(D_METHOD("get_seed"), &FineGen::get_seed);
    ClassDB::bind_method(D_METHOD("height_at", "x", "z"), &FineGen::height_at);
    ClassDB::bind_method(D_METHOD("biome_at", "x", "z"), &FineGen::biome_at);
    ClassDB::bind_method(D_METHOD("column_info", "x", "z"), &FineGen::column_info);
    ClassDB::bind_method(D_METHOD("generate_block", "origin", "size", "lod", "bytes_per_voxel"), &FineGen::generate_block, DEFVAL(2));
    ClassDB::bind_method(D_METHOD("set_uniform_blocks", "on"), &FineGen::set_uniform_blocks);
    ClassDB::bind_static_method("FineGen", D_METHOD("gen_stats"), &FineGen::gen_stats);
    ClassDB::bind_method(D_METHOD("ground_range", "x", "z", "size"), &FineGen::ground_range);
    ClassDB::bind_method(D_METHOD("get_uniform_blocks"), &FineGen::get_uniform_blocks);
    ClassDB::bind_method(D_METHOD("map_image", "center_x", "center_z", "radius", "step"), &FineGen::map_image);
    ClassDB::bind_method(D_METHOD("surface_map", "x0", "z0", "n", "step"), &FineGen::surface_map);
    ClassDB::bind_method(D_METHOD("cave_map", "x0", "z0", "n", "step"), &FineGen::cave_map);
    ClassDB::bind_method(D_METHOD("climate_map", "x0", "z0", "n", "step"), &FineGen::climate_map);
    ClassDB::bind_method(D_METHOD("water_floor_map", "x0", "z0", "n", "step"), &FineGen::water_floor_map);
    ClassDB::bind_method(D_METHOD("find_nearest", "kind", "x", "z", "max_radius"), &FineGen::find_nearest);
    ClassDB::bind_method(D_METHOD("waterfalls_near", "x", "z", "radius"), &FineGen::waterfalls_near);
    ClassDB::bind_method(D_METHOD("hydrology_stats", "x", "z"), &FineGen::hydrology_stats);
    ClassDB::bind_method(D_METHOD("vein_at", "x", "z"), &FineGen::vein_at);
    ClassDB::bind_method(D_METHOD("pickup_heights", "kind", "seed"), &FineGen::pickup_heights);
    ClassDB::bind_method(D_METHOD("ruin_pots_near", "x", "z", "radius"), &FineGen::ruin_pots_near);
    ClassDB::bind_method(D_METHOD("ruin_at", "x", "z"), &FineGen::ruin_at);
    ClassDB::bind_method(D_METHOD("get_macro"), &FineGen::get_macro);
    ClassDB::bind_integer_constant(get_class_static(), StringName(), "FS", FS);
    ClassDB::bind_integer_constant(get_class_static(), StringName(), "RUIN_RAMP", RUIN_RAMP);
}

static void setup_noise(FastNoiseLite &n, int seed, float freq, int octaves) {
    n.SetSeed(seed);
    n.SetNoiseType(FastNoiseLite::NoiseType_OpenSimplex2);
    n.SetFrequency(freq);
    n.SetFractalType(octaves > 1 ? FastNoiseLite::FractalType_FBm : FastNoiseLite::FractalType_None);
    n.SetFractalOctaves(octaves);
}

// The shape the generator gives a stone pickup of `kind` on a patch hashed to `seed` (the showcase
// sets pickups down as the world lays them). Kinds that are not stone pickups count as loose stones.
PackedInt32Array FineGen::pickup_heights(int kind, int seed) const {
    PackedInt32Array out;
    out.resize(FS * FS);
    const uint16_t k = is_pickup((uint16_t)kind) ? (uint16_t)kind : (uint16_t)WorldGen::PEBBLES;
    const uint32_t s = WorldGen::hash3(seed, 41, 977);
    for (int lz = 0; lz < FS; lz++)
        for (int lx = 0; lx < FS; lx++)
            out.set(lz * FS + lx, pickup_shape(k, lx, lz, s));
    return out;
}

void FineGen::configure(int p_seed, int p_sea_level, int p_world_height) {
    ctx_cache_.clear();   // a new seed is a new world
    tree_cache_.clear();
    young_cache_.clear();
    g_->configure(p_seed, p_sea_level / FS, p_world_height / FS);
    setup_noise(n_ground_, p_seed + 101, 1.0f / 64.0f, 2);   // 8 m swells (fine voxels)
    setup_noise(n_ground2_, p_seed + 102, 1.0f / 9.0f, 1);   // ~1 m roughness
    setup_noise(n_hump_, p_seed + 104, 1.0f / 20.0f, 1);     // 2.5 m hummocks
    setup_noise(n_leaf_, p_seed + 103, 1.0f / 6.0f, 1);      // lumps on leaf clumps
    setup_noise(n_litter_, p_seed + 105, 1.0f / 14.0f, 2);   // leaf litter vs grass under trees
    setup_noise(n_meadow_, p_seed + 106, 1.0f / 48.0f, 2);   // thick and thin grass, flower patches
    setup_noise(n_pool_, p_seed + 107, 1.0f / 200.0f, 1);    // ~25 m blobs: where cave floors hold their biome's pool
    setup_noise(n_terrace_, p_seed + 108, 1.0f / 1200.0f, 1); // ~150 m patches: how tall the ground's ledges are
    rocks_.configure(p_seed, p_sea_level);
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
    if (m.hy.berm >= 0 && h < (float)m.hy.berm) h = (float)m.hy.berm;   // a shallow lake's rim
    if (m.hy.water_level > m.water_level) m.water_level = m.hy.water_level;
    if (m.hy.lake && h < (float)m.hy.water_level) m.lakebed = true;
    if (m.hy.river > 0.0f && h < (float)m.hy.water_level) m.riverbed = true;
    if (m.hy.marsh && m.hy.river < 0.3f) {
        h = (float)WorldGen::marsh_floor((int)std::lround(h), m.hy);
        m.marshbed = m.hy.puddle;
    }
    for (int i = 0; i < nruins; i++)
        if (WorldGen::in_ruin(ruins[i], mx, mz, 2)) { m.ruin = i; break; }
    // A ruin's floor is level; round it the ground ramps to the land's own height over RUIN_RAMP cells,
    // a cell up or down per cell out, so it neither stands on a plinth nor sinks into a slope (ADR 0062).
    for (int i = 0; i < nruins; i++) {
        const int d = std::max(std::abs(mx - ruins[i].x) - ruins[i].hw, std::abs(mz - ruins[i].z) - ruins[i].hd);
        if (d > RUIN_RAMP) continue;
        const float ground = (float)ruins[i].base;
        h = d <= 0 ? ground : std::clamp(h, ground - (float)d, ground + (float)d);
        break;
    }
    if (technosim_biome::DECOR_SPIRE[m.col.biome] > 0.0f && m.ruin < 0 && !m.lakebed && !m.riverbed && !m.marshbed && h > (float)m.water_level) {
        m.spike = g.spire_at(m.col.biome, mx, mz);
        h += (float)m.spike;
    }
    m.hc = h;
    m.roof = g.cave_roof(mx, mz);
    float gw = (m.lakebed || m.riverbed || m.marshbed || m.spike > 0 || m.ruin >= 0) ? 0.0f : 1.0f;
    gw *= 1.0f - std::clamp(m.hy.river * 4.0f, 0.0f, 1.0f);
    gw *= smoothstep(0.5f, 6.0f, h - (float)m.water_level);
    if (m.hy.marsh) gw *= 0.3f;
    for (int i = 0; i < nruins && gw > 0.0f; i++)
        if (WorldGen::in_ruin(ruins[i], mx, mz, 6)) gw *= 0.4f;
    m.gw = gw;
    return m;
}

float FineGen::surface_f(int x, int z) const {
    const int mx = floor_div(x, FS), mz = floor_div(z, FS);
    WorldGen::Ruin ruins[4];
    const int nr = g_->ruins_near(mx - 1, mz - 1, mx + 2, mz + 2, ruins, 4);
    const MacroPt a = macro_point(mx, mz, ruins, nr), b = macro_point(mx + 1, mz, ruins, nr);
    const MacroPt c = macro_point(mx, mz + 1, ruins, nr), d = macro_point(mx + 1, mz + 1, ruins, nr);
    const float tx = (float)(x - mx * FS) / FS, tz = (float)(z - mz * FS) / FS;
    return ground_f(bilerp(a.hc, b.hc, c.hc, d.hc, tx, tz) * FS, x, z, bilerp(a.gw, b.gw, c.gw, d.gw, tx, tz));
}

// The fine ground over the interpolated macro height `base` (fine voxels), with the ground's weight w (zero
// at water, rivers, marsh and ruins). Swells (~8 m) are added, then the ground is rounded to ledges
// (bug report 2026-10-25 21-23-02: "having every 1 single voxel step up makes everything feel like lego"):
// levels T voxels apart, so a slope is flat ground broken by drops of several voxels instead of a staircase
// of single ones. T changes in ~150 m patches (3, 4, 5, 6, rarely 8 voxels; past 4 a drop needs a jump); the
// rounding keeps 15 % of the slope so a tread is not dead flat. Hummocks and roughness go on after the
// rounding at 40 % on full ground, so they texture the treads instead of standing up as lone pillars.
float FineGen::ground_f(float base, int x, int z, float w) const {
    if (w <= 0.0f) return base;
    const float fx = (float)x, fz = (float)z;
    const float hf = base + n_ground_.GetNoise(fx, fz) * GROUND_AMP * w;
    const float tn = n_terrace_.GetNoise(fx, fz);
    const float T = tn < -0.3f ? 3.0f : (tn < 0.05f ? 4.0f : (tn < 0.35f ? 5.0f : (tn < 0.6f ? 6.0f : 8.0f)));
    const float q = std::floor(hf / T + 0.5f) * T;
    const float small = n_hump_.GetNoise(fx, fz) * HUMP_AMP + n_ground2_.GetNoise(fx, fz) * GROUND_AMP2;
    return hf + (q - hf) * 0.85f * w + small * w * (1.0f - 0.6f * w);
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
    o.base = o.ground - 10;   // deep enough to meet the lower tread when a ledge (up to 8 voxels) runs past the trunk
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
        c.ylo = std::min(c.ylo, (int)std::floor(b.y - ry) - (t.kind == WorldGen::TREE_JUNGLE ? VINE_MAX : 0));
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
    // Coarse LODs widen shapes so thin parts survive the sparse samples; wood only by under half as much
    // as leaves, or far trunks turn into thick pillars (bug report 2026-10-24 22-15-31, "chunky LOD trees").
    const float wgrow = grow * 0.45f;
    if (c.trunk && y >= t.base && y <= t.top) {
        const float frac = std::clamp((float)(y - t.ground) / (float)std::max(1, t.top - t.ground), 0.0f, 1.0f);
        float r = t.r + (t.r_top - t.r) * frac + wgrow;
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
        const float r = s.r0 + (s.r1 - s.r0) * u + wgrow;
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
        // lumpy clumps up close; the lumps fade with the LOD, where coarse cells only turn them into jagged
        // blocks and extra faces
        const float lump = grow == 0.0f ? 0.45f : (grow < 1.0f ? 0.25f : 0.0f);
        if (lump > 0.0f && leaf_n > 1.5f) leaf_n = n_leaf_.GetNoise(x, fy, z);
        const float edge = lump > 0.0f ? d + leaf_n * lump : d;
        if (edge > 1.0f) continue;
        if (grow == 0.0f && edge > 0.85f && (h3(ix, y, iz) & 15) == 0) continue;   // a few gaps at the edge (each costs faces)
        return t.leaves;
    }
    if (t.kind == WorldGen::TREE_JUNGLE && grow < 1.0f) {
        // vines (bug report 2026-11-02 18-54-25, "dense and green"): strands of leaves hanging from one column in
        // ~28 under a jungle clump, from inside it (so a lumpy underside never leaves one floating) to 0.5-2.3 m
        // below it
        const uint32_t vh = h3(ix, iz, (int)t.seed + 811);
        if (vh % 28 == 0)
            for (uint64_t m = c.bm; m; m &= m - 1) {
                const Blob &b = t.blobs[__builtin_ctzll(m)];
                const float ex = (x - b.x) / b.rx, ez = (z - b.z) / b.rz, q = 1.0f - ex * ex - ez * ez;
                if (q < 0.25f || fy > b.y) continue;
                if (fy >= b.y - b.ry * std::sqrt(q) - (float)(4 + (int)((vh >> 8) % (VINE_MAX - 4)))) return t.leaves;
            }
    }
    return Mat::AIR;
}

// Young trees between the canopy trees: one chance per 3 m cell, as often as the biome has trees,
// never on top of the cell's canopy tree. Deterministic per cell, so every block sees the same ones.
void FineGen::young_trees(int x_lo, int z_lo, int x_hi, int z_hi, const WorldGen::Ruin *ruins, int nruins, std::vector<std::shared_ptr<const FTree>> &out) const {
    const WorldGen &g = *g_.ptr();
    constexpr int YC = 24, M = 20;
    const int cx0 = floor_div(x_lo - M, YC), cx1 = floor_div(x_hi + M, YC);
    const int cz0 = floor_div(z_lo - M, YC), cz1 = floor_div(z_hi + M, YC);
    for (int cz = cz0; cz <= cz1; cz++)
        for (int cx = cx0; cx <= cx1; cx++) {
            const uint32_t hs = h3(cx, cz, g.seed_ + 613);
            const float roll = (float)(hs & 1023) / 1024.0f;
            if (roll >= 0.7f) continue;
            // the cell's tree with every check but the ruins (those near this block), the same for every block
            const std::shared_ptr<const YoungCell> yc = young_cache_.get(((uint64_t)(uint32_t)cx << 32) | (uint32_t)cz, [&]() {
                auto c = std::make_shared<YoungCell>();
                const float fx = (float)(cx * YC + 3 + (int)((hs >> 10) % (YC - 6))) + 0.5f;
                const float fz = (float)(cz * YC + 3 + (int)((hs >> 16) % (YC - 6))) + 0.5f;
                const int mx = floor_div((int)fx, FS), mz = floor_div((int)fz, FS);
                c->mx = mx, c->mz = mz;
                const WorldGen::Column col = g.column_base(mx, mz, false);
                if (col.height <= g.sea_ + (col.biome == technosim_biome::MANGROVE ? 0 : 2) || col.alpine) return c;
                const float dens = technosim_biome::TREE_DENSITY[col.biome];
                if (roll >= std::min(0.7f, dens * (col.biome == technosim_biome::JUNGLE ? 1.0f : 0.55f))) return c;   // the jungle's middle storey
                const WorldGen::Hydro hy = g.hydro_at(mx, mz, col.height);
                if (hy.river > 0.05f || hy.lake || hy.puddle) return c;
                if (g.ravine_near(mx, mz, 4)) return c;
                WorldGen::Tree big;
                if (g.tree_at_cell(floor_div(mx, WorldGen::TREE_CELL), floor_div(mz, WorldGen::TREE_CELL), big)) {
                    const float bx = (float)(big.x * FS + 2) - fx, bz = (float)(big.z * FS + 2) - fz;
                    if (bx * bx + bz * bz < 14.0f * 14.0f) return c;
                }
                const uint32_t ts = h3(cx, cz, g.seed_ + 617);
                int kind = technosim_biome::TREE_KIND[col.biome];
                if ((float)((ts >> 8) & 1023) / 1024.0f < technosim_biome::TREE_MIX[col.biome]) kind = technosim_biome::TREE_MIX_KIND[col.biome];
                // poles reaching for the light: a tall bare trunk, the crown above head height
                const float age = 0.1f + 0.22f * (float)(ts & 255) / 255.0f;
                const int trunk_h = (kind == WorldGen::TREE_SPRUCE || kind == WorldGen::TREE_BIRCH ? 12 : 10) + (int)((ts >> 20) % 5);
                build_tree(kind, fx, fz, trunk_h, 2, age, false, ts, c->t);
                c->any = true;
                return c;
            });
            if (!yc->any) continue;
            bool blocked = false;
            for (int r = 0; r < nruins && !blocked; r++) blocked = WorldGen::in_ruin(ruins[r], yc->mx, yc->mz, RUIN_RAMP + 4);
            if (blocked) continue;
            const FTree &t = yc->t;
            if (t.x + t.reach < x_lo || t.x - t.reach > x_hi || t.z + t.reach < z_lo || t.z - t.reach > z_hi) continue;
            out.push_back(std::shared_ptr<const FTree>(yc, &yc->t));
        }
}

void FineGen::build_stamp(const WorldGen::Stamp &s, FStamp &o) const {
    o.kind = s.kind;
    o.seed = s.seed;
    o.x = (float)(s.x * FS + 2);
    o.z = (float)(s.z * FS + 2);
    o.ground = (int)std::floor(surface_f(s.x * FS + 2, s.z * FS + 2) + 0.5f);
    o.mossy = s.mossy;
    o.biome = g_->column_base(s.x, s.z, false).biome;
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
void FineGen::floor_features(int x_lo, int z_lo, int x_hi, int z_hi, const WorldGen::Ruin *ruins, int nruins, bool clutter, std::vector<FStamp> &out) const {
    const WorldGen &g = *g_.ptr();
    constexpr int FC = 16, M = 34;
    const int cx0 = floor_div(x_lo - M, FC), cx1 = floor_div(x_hi + M, FC);
    const int cz0 = floor_div(z_lo - M, FC), cz1 = floor_div(z_hi + M, FC);
    for (int cz = cz0; cz <= cz1; cz++)
        for (int cx = cx0; cx <= cx1; cx++) {
            const uint32_t hs = h3(cx, cz, g.seed_ + 601);
            const float roll = (float)(hs & 1023) / 1024.0f;
            if (roll >= 0.3f) continue;
            const float fx = (float)(cx * FC + 2 +(int)((hs >> 10) % (FC - 4))) + 0.5f;
            const float fz = (float)(cz * FC + 2 + (int)((hs >> 16) % (FC - 4))) + 0.5f;
            const int mx = floor_div((int)fx, FS), mz = floor_div((int)fz, FS);
            const WorldGen::Column col = g.column_base(mx, mz, false);
            if (col.height <= g.sea_ + 1 || col.alpine || col.snow) continue;
            // the jungle's undergrowth: big leafy bushes in nearly every featured cell (bug report 2026-11-02 18-54-25)
            const bool jungle = col.biome == technosim_biome::JUNGLE;
            const float dens = technosim_biome::TREE_DENSITY[col.biome];
            const float f = std::clamp(dens / 0.5f, 0.0f, 1.3f);
            const float p_log = 0.05f * f, p_stump = p_log + 0.03f * f, p_bush = p_stump + 0.02f + (jungle ? 0.17f : 0.14f * f), p_rock = p_bush + 0.012f + 0.02f * f;
            if (roll >= p_rock) continue;
            if (dens <= 0.0f && roll < p_stump) continue;   // logs and stumps come from trees
            const WorldGen::Hydro hy = g.hydro_at(mx, mz, col.height);
            if (hy.river > 0.05f || hy.lake || hy.puddle) continue;
            if (g.ravine_near(mx, mz, 3)) continue;
            bool blocked = false;
            for (int r = 0; r < nruins && !blocked; r++) blocked = WorldGen::in_ruin(ruins[r], mx, mz, RUIN_RAMP);
            if (blocked) continue;
            FStamp s;
            s.seed = h3(cx, cz, g.seed_ + 607);
            s.biome = col.biome;
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
                s.size = jungle ? 3.5f + 5.0f * u1 : 2.5f + 4.0f * u1;
                if ((float)(s.seed >> 22) / 1024.0f < technosim_biome::DECOR_BERRIES[col.biome]) s.leaves = Mat::BLUEBERRY_BUSH;
            } else { s.kind = WorldGen::STAMP_BOULDER; s.size = 2.0f + 3.0f * u1; }
            s.ground = (int)std::floor(surface_f((int)fx, (int)fz) + 0.5f);
            out.push_back(s);
        }
    // Fallen sticks (the twigs pickup, Vintage Story style in voxels): 1-1.5 m long, one voxel thick,
    // a forked side branch, lying at any angle; the column loop lays them on the ground. Half the
    // 2 m cells of a forest hold one, a few in open land.
    constexpr int SM = 16;   // a stick reaches ~15 voxels from its cell's point
    const int sx0 = floor_div(x_lo - SM, FC), sx1 = floor_div(x_hi + SM, FC);
    const int sz0 = floor_div(z_lo - SM, FC), sz1 = floor_div(z_hi + SM, FC);
    for (int cz = sz0; cz <= sz1; cz++)
        for (int cx = sx0; cx <= sx1; cx++) {
            const uint32_t hs = h3(cx, cz, g.seed_ + 613);
            const float roll = (float)(hs & 1023) / 1024.0f;
            if (roll >= 0.4f) continue;
            const float fx = (float)(cx * FC + 3 + (int)((hs >> 10) % (FC - 6))) + 0.5f;
            const float fz = (float)(cz * FC + 3 + (int)((hs >> 16) % (FC - 6))) + 0.5f;
            const int mx = floor_div((int)fx, FS), mz = floor_div((int)fz, FS);
            const WorldGen::Column col = g.column_base(mx, mz, false);
            if (col.height <= g.sea_ + 1 || col.alpine || col.snow) continue;
            const float dens = technosim_biome::TREE_DENSITY[col.biome];
            if (roll >= 0.03f + 0.37f * std::clamp(dens / 0.5f, 0.0f, 1.0f)) continue;   // ~70 % of the 2026-10-25 rate (bug report 2026-10-27 15-16-47)
            const WorldGen::Hydro hy = g.hydro_at(mx, mz, col.height);
            if (hy.river > 0.05f || hy.lake || hy.puddle || hy.marsh) continue;
            if (g.ravine_near(mx, mz, 3)) continue;
            bool blocked = false;
            for (int r = 0; r < nruins && !blocked; r++) blocked = WorldGen::in_ruin(ruins[r], mx, mz, RUIN_RAMP);
            if (blocked) continue;
            FStamp s{};
            s.kind = STICK;
            s.seed = h3(cx, cz, g.seed_ + 617);
            s.x = fx;
            s.z = fz;
            const float an = (float)(s.seed & 1023) / 1023.0f * 3.1416f;
            s.dx = std::cos(an);
            s.dz = std::sin(an);
            s.size = 4.5f + (float)((s.seed >> 10) & 7) * 0.45f;   // half length: 9-15 voxels long
            s.r = STICK_THICK[(s.seed >> 20) % 3];                // thin, medium or thick (2026-10-25)
            s.ground = (int)std::floor(surface_f((int)fx, (int)fz) + 0.5f);
            // only on level ground: over a ledge or a cliff the column loop laid a few voxels of it on each step
            // (bug report 2026-10-27 15-16-47, "twigs on cliffsides with like 3 voxels")
            const float ex = s.dx * (s.size + 1.0f), ez = s.dz * (s.size + 1.0f);
            const float e0 = surface_f((int)(fx + ex), (int)(fz + ez)), e1 = surface_f((int)(fx - ex), (int)(fz - ez));
            if (std::max({std::fabs(e0 - s.ground), std::fabs(e1 - s.ground)}) > 2.0f) continue;
            out.push_back(s);
        }
    // Giant flowers (bug report 2026-11-02 18-57-25, "tree sized flowers in the flower meadow"): about one per
    // 8 m cell of the meadow, a 3.5-5.5 m stalk under a 2.5-3.5 m red head (stamp_voxel draws it).
    constexpr int GC = 64, GM = 26;
    for (int cz = floor_div(z_lo - GM, GC); cz <= floor_div(z_hi + GM, GC); cz++)
        for (int cx = floor_div(x_lo - GM, GC); cx <= floor_div(x_hi + GM, GC); cx++) {
            const uint32_t hs = h3(cx, cz, g.seed_ + 619);
            if ((hs & 1023) >= 560) continue;
            const float fx = (float)(cx * GC + 12 + (int)((hs >> 10) % (GC - 24))) + 0.5f;
            const float fz = (float)(cz * GC + 12 + (int)((hs >> 16) % (GC - 24))) + 0.5f;
            const int mx = floor_div((int)fx, FS), mz = floor_div((int)fz, FS);
            const WorldGen::Column col = g.column_base(mx, mz, false);
            if (col.biome != technosim_biome::FLOWER_MEADOW || col.height <= g.sea_ + 1) continue;
            const WorldGen::Hydro hy = g.hydro_at(mx, mz, col.height);
            if (hy.river > 0.05f || hy.lake || hy.puddle || hy.marsh) continue;
            if (g.ravine_near(mx, mz, 4)) continue;
            bool blocked = false;
            for (int r = 0; r < nruins && !blocked; r++) blocked = WorldGen::in_ruin(ruins[r], mx, mz, RUIN_RAMP + 4);
            if (blocked) continue;
            FStamp s{};
            s.kind = GIANT_FLOWER;
            s.seed = h3(cx, cz, g.seed_ + 623);
            s.biome = col.biome;
            s.x = fx;
            s.z = fz;
            const float an = (float)(s.seed & 1023) / 1023.0f * 6.2832f;
            s.dx = std::cos(an);
            s.dz = std::sin(an);
            s.size = 28.0f + (float)((s.seed >> 10) & 15);        // stalk 28-43 voxels
            s.r = 10.0f + (float)((s.seed >> 14) & 7) * 0.6f;      // head radius 10-14
            s.ground = (int)std::floor(surface_f((int)fx, (int)fz) + 0.5f);
            out.push_back(s);
        }
    // Clutter pieces of whole voxels (ADR 0047): flowers in meadow patches, mushrooms on forest floors,
    // seashells on beaches. At most one per CLUTTER_CELL, centred, so the next is at least a voxel away
    // (touching voxels of one group would dig as one piece). LOD 0 only: nothing coarser draws them.
    if (!clutter) return;
    constexpr int CL = 6;
    static const uint16_t FL[5] = {Mat::FLOWER, Mat::FLOWER_RED, Mat::CORNFLOWER, Mat::OXEYE_DAISY, Mat::ALLIUM};
    const int kx0 = floor_div(x_lo - 3, CL), kx1 = floor_div(x_hi + 3, CL);
    const int kz0 = floor_div(z_lo - 3, CL), kz1 = floor_div(z_hi + 3, CL);
    for (int cz = kz0; cz <= kz1; cz++)
        for (int cx = kx0; cx <= kx1; cx++) {
            const uint32_t hs = h3(cx, cz, g.seed_ + 631);
            const int px = cx * CL + 3, pz = cz * CL + 3;
            const int mx = floor_div(px, FS), mz = floor_div(pz, FS);
            const WorldGen::Column col = g.column_base(mx, mz, false);
            if (col.height <= g.sea_) continue;
            const bool snowy = col.alpine || col.snow;
            const bool beach = col.biome == WorldGen::BIOME_BEACH || (col.biome == WorldGen::BIOME_OCEAN && col.shore > -90.0f);
            const float dens = technosim_biome::TREE_DENSITY[col.biome];
            int kind = 0;
            uint16_t m = Mat::AIR;
            if (snowy) {
            } else if (beach) {
                if ((hs & 15) == 0) { kind = SHELL_S; m = Mat::SEASHELL; }
            } else if (col.height <= g.sea_ + 1) {
                continue;
            } else if (dens >= 0.2f) {
                if ((float)((hs >> 12) & 1023) / 1024.0f < technosim_biome::DECOR_MUSHROOMS[col.biome]) { kind = MUSHROOM_S; m = ((hs >> 5) & 3) == 0 ? Mat::MUSHROOM_RED : Mat::MUSHROOM_BROWN; }
            } else if (technosim_biome::SURFACE_TOP[col.biome] == Mat::GRASS) {
                const bool meadow = col.biome == technosim_biome::FLOWER_MEADOW;
                if (n_meadow_.GetNoise((float)px * 0.37f + 500.0f, (float)pz * 0.37f - 500.0f) > (meadow ? -0.2f : 0.42f) && (meadow || (hs & 1))) {
                    kind = FLOWER_S;
                    m = FL[h3(px >> 5, pz >> 5, g.seed_ + 37) % 5];
                }
            }
            if (kind == 0) rocks_.clutter_pick(col.biome, beach, snowy, dens >= 0.2f, hs, kind, m);   // ADR 0057
            if (kind == 0) continue;
            const WorldGen::Hydro hy = g.hydro_at(mx, mz, col.height);
            if (hy.river > 0.05f || hy.lake || hy.puddle || hy.marsh) continue;
            if (g.ravine_near(mx, mz, 1)) continue;
            bool blocked = false;
            for (int r = 0; r < nruins && !blocked; r++) blocked = WorldGen::in_ruin(ruins[r], mx, mz, RUIN_RAMP);
            if (blocked) continue;
            FStamp s{};
            s.kind = kind;
            s.seed = h3(cx, cz, g.seed_ + 641);
            s.x = (float)px + 0.5f;
            s.z = (float)pz + 0.5f;
            s.size = 2.0f;
            s.wood = m;
            s.ground = (int)std::floor(surface_f(px, pz) + 0.5f);
            out.push_back(s);
        }
}

// A clutter piece's voxel (ADR 0047), dy = 0 the first voxel above its ground. Flowers: a stem of
// plant_stem with two leaves and a head of petals (~30 voxels; alliums a ball on a taller stem);
// mushrooms: a pale stem under a cap (a big one in three); seashells: a ribbed fan. Air elsewhere.
uint16_t FineGen::clutter_voxel(const FStamp &s, int dx, int dy, int dz) {
    const int adx = std::abs(dx), adz = std::abs(dz);
    switch (s.kind) {
        case FLOWER_S: {
            const bool ball = s.wood == Mat::ALLIUM;
            const int S = ball ? 6 : 4 + (int)((s.seed >> 3) & 1);   // stem height
            if (dx == 0 && dz == 0 && dy < S) return Mat::PLANT_STEM;
            const int lx = (s.seed & 1) ? 1 : 0, lz = 1 - lx;        // leaves on one axis, one each side
            if ((dx == lx && dz == lz && dy == 1) || (dx == -lx && dz == -lz && dy == 2)) return Mat::PLANT_STEM;
            if (ball) {
                const int c = S + 1;
                if (adx <= 1 && adz <= 1 && std::abs(dy - c) <= 1 && adx + adz + std::abs(dy - c) < 3) return s.wood;
                return Mat::AIR;
            }
            if (dy == S && ((adx <= 1 && adz <= 1) || (adx + adz == 2 && (adx == 0 || adz == 0)))) return s.wood;
            if (dy == S + 1 && adx <= 1 && adz <= 1) return s.wood;
            return Mat::AIR;
        }
        case MUSHROOM_S: {
            const bool big = (s.seed % 3) == 0;
            if ((s.seed % 3) == 1 && ((s.seed >> 6) & 1)) {
                // tall and thin: a 4-5 high stem under a small domed cap (2026-10-25: variety)
                const int T = 4 + (int)((s.seed >> 7) & 1);
                if (dx == 0 && dz == 0 && dy < T) return Mat::MUSHROOM_STEM;
                if (dy == T && adx <= 1 && adz <= 1 && adx + adz < 2) return s.wood;
                if (dy == T && adx == 1 && adz == 1 && ((s.seed >> 8) & 1)) return s.wood;
                if (dy == T + 1 && dx == 0 && dz == 0) return s.wood;
                return Mat::AIR;
            }
            const int H = big ? 3 + (int)((s.seed >> 2) & 1) : 2;
            if (big) {
                // 2x2 stem (dx, dz in 0..1), a 4x4 cap without corners, a 2x2 crown
                const int ex = dx * 2 - 1, ez = dz * 2 - 1;   // -1/+1 on the stem, wider out
                if (dy < H && (dx == 0 || dx == 1) && (dz == 0 || dz == 1)) return Mat::MUSHROOM_STEM;
                if (dy == H && dx >= -1 && dx <= 2 && dz >= -1 && dz <= 2 && !(std::abs(ex) == 3 && std::abs(ez) == 3)) return s.wood;
                if (dy == H + 1 && (dx == 0 || dx == 1) && (dz == 0 || dz == 1)) return s.wood;
                return Mat::AIR;
            }
            if (dx == 0 && dz == 0 && dy < H) return Mat::MUSHROOM_STEM;
            if (dy == H && adx <= 1 && adz <= 1) return s.wood;
            if (dy == H + 1 && adx + adz <= 1) return s.wood;
            return Mat::AIR;
        }
        case SHELL_S: {
            const bool turn = s.seed & 1;
            const int a = turn ? dz : dx, b = turn ? dx : dz;   // a across the fan, b along it
            switch ((s.seed >> 3) % 3) {
                case 0:   // a ribbed fan, 3 across, 2 along, ridged
                    if (dy == 0 && std::abs(a) <= 1 && (b == 0 || b == 1)) return s.wood;
                    if (dy == 1 && a == 0 && (b == 0 || b == 1)) return s.wood;
                    return Mat::AIR;
                case 1:   // a clam: a 4x3 dome, 2 high
                    if (dy == 0 && a >= -2 && a <= 1 && b >= -1 && b <= 1 && !(std::abs(b) == 1 && (a == -2 || a == 1))) return s.wood;
                    if (dy == 1 && a >= -1 && a <= 0 && std::abs(b) <= 1 && !(std::abs(b) == 1 && ((s.seed >> 5) & 1))) return s.wood;
                    return Mat::AIR;
                default:  // a snail shell: a whorl climbing to a point
                    if (dy == 0 && a >= -1 && a <= 1 && b >= 0 && b <= 1) return s.wood;
                    if (dy == 1 && a >= -1 && a <= 0 && b >= 0 && b <= 1) return s.wood;
                    if (dy == 2 && a == -1 && b == 1) return s.wood;
                    return Mat::AIR;
            }
        }
        default: return technosim_rock::RockTypes::clutter_voxel(s.kind, s.seed, s.wood, dx, dy, dz);   // ADR 0057
    }
    return Mat::AIR;
}

float FineGen::stick_dist(const FStamp &s, float x, float z, float *fork_out) {
    // A crooked stick (bug report 2026-11-02 18-49-07, "more irregular, a cohesive twig/branch"): the shaft bends
    // once, off its middle, 1-2.5 voxels to one side, and thins to one voxel toward both ends; a side twig
    // forks off forward with a short sprig at its tip, and most sticks carry a second, shorter twig on the
    // other side. The distance to the shaft comes back widened toward the ends (the taper), so the caller's
    // half-width test thins it.
    auto u = [&](int b, int n) { return (float)((s.seed >> b) & ((1u << n) - 1)) / (float)((1u << n) - 1); };
    const float px = -s.dz, pz = s.dx;   // the stick's side
    const float kt = (u(24, 3) - 0.5f) * 0.5f * s.size;                                   // the bend, along
    const float kb = (1.0f + 1.5f * u(27, 3)) * (((s.seed >> 30) & 1) ? 1.0f : -1.0f);    // and aside
    const float ax = s.x - s.dx * s.size, az = s.z - s.dz * s.size, bx2 = s.x + s.dx * s.size, bz2 = s.z + s.dz * s.size;
    const float kx = s.x + s.dx * kt + px * kb, kz = s.z + s.dz * kt + pz * kb;
    const float da = seg_dist2(x, 0.0f, z, ax, 0.0f, az, kx, 0.0f, kz), db = seg_dist2(x, 0.0f, z, kx, 0.0f, kz, bx2, 0.0f, bz2);
    const float t = std::clamp(std::fabs((x - s.x) * s.dx + (z - s.z) * s.dz) / s.size, 0.0f, 1.0f);
    const float d = std::sqrt(std::min(da, db)) + std::max(0.0f, s.r - 0.6f) * smoothstep(0.35f, 0.95f, t);
    // the fork: a third of the way along, 3-6 voxels out at 40 degrees to one side, pointing forward, with a sprig
    const float along = s.size * (((s.seed >> 14) & 1) ? 0.35f : -0.35f);
    const float side = ((s.seed >> 15) & 1) ? 1.0f : -1.0f;
    const float fwd = along > 0.0f ? 1.0f : -1.0f;
    const float bx = s.x + s.dx * along + px * kb * 0.5f, bz = s.z + s.dz * along + pz * kb * 0.5f;
    const float bl = 3.0f + (float)((s.seed >> 16) & 3);   // 3-6 voxels
    const float ex = bx + (s.dx * fwd * 0.77f + px * side * 0.64f) * bl, ez = bz + (s.dz * fwd * 0.77f + pz * side * 0.64f) * bl;
    const float sl = 1.5f + 1.5f * u(18, 2);
    const float sx2 = ex + (s.dx * fwd * 0.2f - px * side * 0.98f) * sl, sz2 = ez + (s.dz * fwd * 0.2f - pz * side * 0.98f) * sl;
    float f2 = std::min(seg_dist2(x, 0.0f, z, bx, 0.0f, bz, ex, 0.0f, ez), seg_dist2(x, 0.0f, z, ex - (ex - bx) * 0.3f, 0.0f, ez - (ez - bz) * 0.3f, sx2, 0.0f, sz2));
    if ((s.seed & 3) != 0) {
        // the second twig: from the other half, the other side, 2-4 voxels
        const float a2 = -along * 0.6f, l2 = 2.0f + 2.0f * u(21, 2);
        const float cx = s.x + s.dx * a2, cz = s.z + s.dz * a2;
        f2 = std::min(f2, seg_dist2(x, 0.0f, z, cx, 0.0f, cz, cx + (-s.dx * fwd * 0.6f - px * side * 0.8f) * l2, 0.0f, cz + (-s.dz * fwd * 0.6f - pz * side * 0.8f) * l2));
    }
    const float f = std::sqrt(f2);
    if (fork_out) {
        *fork_out = f;   // the side branch, measured apart: it stays thin on a thick stick
        return d;
    }
    return std::min(d, f);
}

uint16_t FineGen::stamp_voxel(const FStamp &s, float x, int y, float z, float grow) const {
    const float dx = x - s.x, dz = z - s.z, fy = (float)y;
    if (s.kind >= technosim_rock::RockTypes::KIND_BASE) return Mat::AIR;   // clutter pieces (ADR 0057), laid per column
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
                if ((float)((hh >> 12) & 1023) / 1024.0f < technosim_biome::DECOR_LOG_MUSHROOMS[s.biome]) return (hh >> 8) % 3 == 0 ? Mat::MUSHROOM_RED : Mat::MUSHROOM_BROWN;
            }
            return Mat::AIR;
        }
        case GIANT_FLOWER: {
            // a stalk that leans into a curve, two long leaves, and a head tipped toward the lean: a domed
            // yellow heart in a ring of 6-8 red petals that cup upward
            const float H = s.size, R = s.r, yy = fy - (float)s.ground;
            const float L = 2.5f + 3.5f * (float)((s.seed >> 18) & 7) / 7.0f;   // lean at the top, voxels
            if (yy < -3.0f || yy > H + R * 0.6f + 4.0f || dx * dx + dz * dz > (R + L + 10.0f) * (R + L + 10.0f)) return Mat::AIR;
            const float t = std::clamp(yy / H, 0.0f, 1.0f);
            const float cx = s.x + s.dx * L * t * t, cz = s.z + s.dz * L * t * t;
            if (yy <= H) {
                const float rs = 1.7f - 0.5f * t + grow;
                if ((x - cx) * (x - cx) + (z - cz) * (z - cz) <= rs * rs) return Mat::GIANT_STEM;
                for (int i = 0; i < 2; i++) {
                    // a leaf: a blade rising from the stalk, arching over and drooping at its tip
                    const float lt = i ? 0.5f : 0.28f, ly = H * lt;
                    const float la = (float)((s.seed >> (4 + 3 * i)) & 7) * 0.4f + (float)i * 3.1416f;
                    const float ldx = std::cos(la), ldz = std::sin(la), len = 7.0f + (float)((s.seed >> (20 + i)) & 1) * 2.0f;
                    const float ox = s.x + s.dx * L * lt * lt, oz = s.z + s.dz * L * lt * lt;
                    const float a = (x - ox) * ldx + (z - oz) * ldz, b = -(x - ox) * ldz + (z - oz) * ldx;
                    if (a < 0.5f || a > len) continue;
                    const float w = 0.4f + 1.6f * std::sin(3.1416f * a / len) + grow;
                    const float yc = ly + a * 0.9f - a * a * 0.11f;
                    if (std::fabs(b) <= w && std::fabs(yy - yc) <= 0.6f + grow) return Mat::GIANT_STEM;
                }
            }
            // the head, in a frame tipped toward the lean
            const float hx = s.x + s.dx * L, hz = s.z + s.dz * L, hy = (float)s.ground + H;
            float nx = s.dx * 0.45f, ny = 1.0f, nz = s.dz * 0.45f;
            const float nl = 1.0f / std::sqrt(nx * nx + ny * ny + nz * nz);
            nx *= nl, ny *= nl, nz *= nl;
            const float vx = x - hx, vy = fy - hy, vz = z - hz;
            const float hh = vx * nx + vy * ny + vz * nz;                  // height over the head's plane
            const float qx = vx - nx * hh, qy = vy - ny * hh, qz = vz - nz * hh;
            const float rho = std::sqrt(qx * qx + qy * qy + qz * qz);
            if (rho > R + 1.0f + grow || hh < -3.5f || hh > R * 0.6f + 3.0f) return Mat::AIR;
            const float Rh = R * 0.3f;
            if (rho <= Rh + grow && hh >= -1.0f && hh <= 2.2f - 1.4f * (rho / Rh) * (rho / Rh)) return Mat::GIANT_HEART;
            if (rho <= Rh + 1.0f + grow && hh < -1.0f && hh >= -3.0f + rho / (Rh + 1.0f)) return Mat::GIANT_STEM;   // the green cup under it
            // petals: the angle round the head's normal, in the plane's frame (e1 = the lean's side)
            const float e1x = -s.dz, e1z = s.dx;   // horizontal, so on the tipped plane up to a small error
            const float e2x = ny * e1z, e2y = nz * e1x - nx * e1z, e2z = -ny * e1x;   // n x e1
            const float th = std::atan2(qx * e2x + qy * e2y + qz * e2z, qx * e1x + qz * e1z);
            const int P = 6 + (int)((s.seed >> 22) % 3);
            const float sector = 6.2832f / (float)P, off = (float)((s.seed >> 25) & 63) / 63.0f * sector;
            float dth = std::fmod(th - off + 12.5664f, sector);
            if (dth > sector * 0.5f) dth -= sector;
            const float u = (rho - Rh * 0.6f) / (R - Rh * 0.6f);
            if (u < 0.0f || u > 1.0f) return Mat::AIR;
            const float half = rho * std::sin(std::fabs(dth));            // across the petal's mid line
            const float w = R * 0.34f * std::sin(3.1416f * std::min(1.0f, u * 0.85f + 0.15f)) + 0.6f + grow;
            const float hc = 0.4f + 3.2f * u * u;                           // the petal cups up toward its tip
            if (half <= w && std::fabs(dth) < sector * 0.5f && std::fabs(hh - hc) <= 0.75f + grow) return Mat::GIANT_PETAL;
            return Mat::AIR;
        }
        case STICK: case FLOWER_S: case MUSHROOM_S: case SHELL_S: return Mat::AIR;   // laid on each column's ground by the column loop
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
// ---------- blocks ----------

// Block counts and worker time by result (gen_stats, ADR 0105): what the generation queue spends on.
// 0 = all air, 1 = one uniform material, 2 = the full per-voxel buffer.
static std::atomic<uint64_t> s_gen_n[3], s_gen_us[3], s_ctx_hit, s_ctx_miss, s_ctx_build_us, s_sky;

PackedByteArray FineGen::generate_block(Vector3i origin, Vector3i size, int lod, int bytes_per_voxel) const {
    const auto t0 = std::chrono::steady_clock::now();
    PackedByteArray out = generate_block_impl(origin, size, lod, bytes_per_voxel);
    const int kind = out.is_empty() ? 0 : (out.size() <= 4 ? 1 : 2);
    s_gen_n[kind] += 1;
    s_gen_us[kind] += (uint64_t)std::chrono::duration_cast<std::chrono::microseconds>(std::chrono::steady_clock::now() - t0).count();
    return out;
}

Vector2i FineGen::ground_range(int x, int z, int size) const {
    const std::shared_ptr<const ColumnCtx> c = column_ctx(Vector3i(x, 0, z), Vector3i(size, size, size), 0);
    int lo = *std::min_element(c->h.begin(), c->h.end());
    // where a cave opens to the sky (a ravine, a sinkhole or a ramp: roof 0) it is seen from afar, so the column
    // reaches down past the deepest of them (ravines and ramps ~40 m, sinkholes up to ~70 m)
    for (const MacroPt &m : c->lat)
        if (m.roof == 0) {
            lo -= OPEN_CAVE_DEPTH;
            break;
        }
    // the top bound generate_block's sky check uses: nothing generated reaches above it
    const int hi = std::max({(c->hmax_m + 12) * FS, c->wl_max * FS + 1, c->fall_max * FS + 1, c->feat_top + 1, (c->hmax_m + 1) * FS + 12});
    return Vector2i(lo, hi);
}

Dictionary FineGen::gen_stats() {
    Dictionary d;
    const char *names[3] = {"air", "uniform", "full"};
    for (int i = 0; i < 3; i++) {
        d[String(names[i]) + "_n"] = (int64_t)s_gen_n[i].exchange(0);
        d[String(names[i]) + "_us"] = (int64_t)s_gen_us[i].exchange(0);
    }
    d["ctx_hit"] = (int64_t)s_ctx_hit.exchange(0);
    d["ctx_miss"] = (int64_t)s_ctx_miss.exchange(0);
    d["ctx_build_us"] = (int64_t)s_ctx_build_us.exchange(0);
    d["sky"] = (int64_t)s_sky.exchange(0);
    d["ctx_bytes_lod0"] = (int64_t)(sizeof(ColumnCtx) + 25 * sizeof(MacroPt));   // a 16-voxel footprint's 5 x 5 lattice
    return d;
}

void FineGen::gather_features(int x_lo, int z_lo, int x_hi, int z_hi, const WorldGen::Ruin *ruins, int nruins, int lod,
                              std::vector<std::shared_ptr<const FTree>> &trees, size_t &n_mature, std::vector<FStamp> &stamps) const {
    const WorldGen &g = *g_.ptr();
    static constexpr int MAXT = 160, MAXS = 32;
    n_mature = 0;
    if (lod <= 3) {
        // canopy trees reach up to ~7 m from their trunk: search 12 macro voxels beyond the block
        WorldGen::Tree mtrees[MAXT];
        const int nt = g.trees_near(floor_div(x_lo, FS) - 12, floor_div(z_lo, FS) - 12, floor_div(x_hi, FS) + 12, floor_div(z_hi, FS) + 12, mtrees, MAXT);
        trees.reserve(nt + 16);
        for (int i = 0; i < nt; i++) {
            bool blocked = false;
            for (int r = 0; r < nruins && !blocked; r++) blocked = WorldGen::in_ruin(ruins[r], mtrees[i].x, mtrees[i].z, RUIN_RAMP + 4);
            if (blocked) continue;
            const WorldGen::Tree &mt = mtrees[i];
            const uint32_t ts = h3(mt.x, mt.z, g.seed_ + 77);
            const float fx = (float)(mt.x * FS + 2) + (float)((int)(ts & 3) - 1) * 0.5f;
            const float fz = (float)(mt.z * FS + 2) + (float)((int)((ts >> 2) & 3) - 1) * 0.5f;
            // cheap reject before the tree is built
            const float bound = (12.0f + 4.0f) * FS;
            if (fx + bound < x_lo || fx - bound > x_hi || fz + bound < z_lo || fz - bound > z_hi) continue;
            // the tree is the same for every block it reaches: built once (mt is a function of its position)
            const std::shared_ptr<const FTree> tp = tree_cache_.get(((uint64_t)(uint32_t)mt.x << 32) | (uint32_t)mt.z, [&]() {
                auto t = std::make_shared<FTree>();
                build_tree(mt.kind, fx, fz, mt.trunk_h, mt.radius, tree_age(mt, ts), mt.biome == technosim_biome::OLD_GROWTH_FOREST, ts, *t);
                return t;
            });
            const FTree &t = *tp;
            if (t.x + t.reach < x_lo || t.x - t.reach > x_hi || t.z + t.reach < z_lo || t.z - t.reach > z_hi) continue;
            trees.push_back(tp);
        }
        n_mature = trees.size();
        if (lod <= 2) young_trees(x_lo, z_lo, x_hi, z_hi, ruins, nruins, trees);
    }
    if (lod <= 2) {
        WorldGen::Stamp ms[MAXS];
        const int ns = g.stamps_near(floor_div(x_lo, FS) - 2, floor_div(z_lo, FS) - 2, floor_div(x_hi, FS) + 2, floor_div(z_hi, FS) + 2, ms, MAXS);
        for (int i = 0; i < ns; i++) {
            bool blocked = false;
            for (int r = 0; r < nruins && !blocked; r++) blocked = WorldGen::in_ruin(ruins[r], ms[i].x, ms[i].z, RUIN_RAMP);
            if (blocked) continue;
            FStamp s;
            build_stamp(ms[i], s);
            stamps.push_back(s);
        }
        floor_features(x_lo, z_lo, x_hi, z_hi, ruins, nruins, lod == 0, stamps);
    }
}

std::shared_ptr<const FineGen::ColumnCtx> FineGen::column_ctx(Vector3i origin, Vector3i size, int lod) const {
    const uint64_t key = ((uint64_t)(uint32_t)origin.x * 0x9E3779B97F4A7C15ull) ^ ((uint64_t)(uint32_t)origin.z * 0xC2B2AE3D27D4EB4Full)
        ^ ((uint64_t)lod << 56) ^ ((uint64_t)(size.x & 0xff) << 40) ^ ((uint64_t)(size.z & 0xff) << 48);
    bool built = false;
    std::shared_ptr<const ColumnCtx> c = ctx_cache_.get(key, [&]() { return build_column_ctx(origin, size, lod); }, &built);
    if (c->x == origin.x && c->z == origin.z && c->sx == size.x && c->sz == size.z && c->lod == lod) {
        (built ? s_ctx_miss : s_ctx_hit) += 1;
        return c;
    }
    s_ctx_miss += 1;   // two footprints with one key: this one is built and not kept
    return build_column_ctx(origin, size, lod);
}

std::shared_ptr<const FineGen::ColumnCtx> FineGen::build_column_ctx(Vector3i origin, Vector3i size, int lod) const {
    const auto tb = std::chrono::steady_clock::now();
    const WorldGen &g = *g_.ptr();
    const int step = 1 << lod;
    const int L = std::max(FS, step);
    const int lx0 = floor_div(origin.x, L) * L, lz0 = floor_div(origin.z, L) * L;
    const int nx = floor_div(origin.x + (size.x - 1) * step - lx0, L) + 2;
    const int nz = floor_div(origin.z + (size.z - 1) * step - lz0, L) + 2;
    const int mx0 = lx0 / FS, mz0 = lz0 / FS, mstep = L / FS;
    auto c = std::make_shared<ColumnCtx>();
    c->x = origin.x, c->z = origin.z, c->sx = size.x, c->sz = size.z, c->lod = lod;
    c->nruins = g.ruins_near(mx0 - 1, mz0 - 1, mx0 + nx * mstep + 1, mz0 + nz * mstep + 1, c->ruins, 4);
    for (int i = 0; i < c->nruins; i++) technosim_ruin::prepare(c->ruins[i], c->rprep[i]);
    c->lat.resize((size_t)nx * nz);
    for (int j = 0; j < nz; j++)
        for (int i = 0; i < nx; i++) {
            MacroPt &m = c->lat[(size_t)j * nx + i];
            m = macro_point(mx0 + i * mstep, mz0 + j * mstep, c->ruins, c->nruins);
            c->hmin_m = std::min(c->hmin_m, m.hc);
            c->hmax_m = std::max(c->hmax_m, (int)std::ceil(m.hc) + m.spike);
            c->wl_max = std::max(c->wl_max, m.water_level);
            c->fall_max = std::max(c->fall_max, m.hy.fall_top);
        }
    gather_features(origin.x, origin.z, origin.x + (size.x - 1) * step, origin.z + (size.z - 1) * step, c->ruins, c->nruins, lod, c->trees, c->n_mature, c->stamps);
    for (const auto &t : c->trees) c->feat_top = std::max(c->feat_top, t->y_hi);
    for (const FStamp &s : c->stamps) c->feat_top = std::max(c->feat_top, s.ground + (int)(s.size + s.r) + 8);
    // the fine ground of every column, as the block pass interpolates it
    const int world_h = g.world_height_ * FS;
    c->h.resize((size_t)size.x * size.z);
    for (int z = 0; z < size.z; z++) {
        const int wz = origin.z + z * step;
        const float uz = (float)(wz - lz0) / L;
        const int j = std::min((int)uz, nz - 2);
        const float tz = uz - j;
        for (int x = 0; x < size.x; x++) {
            const int wx = origin.x + x * step;
            const float ux = (float)(wx - lx0) / L;
            const int i = std::min((int)ux, nx - 2);
            const float tx = ux - i;
            const MacroPt &c00 = c->lat[(size_t)j * nx + i], &c10 = c->lat[(size_t)j * nx + i + 1];
            const MacroPt &c01 = c->lat[(size_t)(j + 1) * nx + i], &c11 = c->lat[(size_t)(j + 1) * nx + i + 1];
            const float gwf = bilerp(c00.gw, c10.gw, c01.gw, c11.gw, tx, tz);
            const float hf = ground_f(bilerp(c00.hc, c10.hc, c01.hc, c11.hc, tx, tz) * FS, wx, wz, gwf);
            c->h[(size_t)x + (size_t)size.x * z] = std::clamp((int)std::floor(hf + 0.5f), 8 * FS, world_h - 8 * FS);
        }
    }
    s_ctx_build_us += (uint64_t)std::chrono::duration_cast<std::chrono::microseconds>(std::chrono::steady_clock::now() - tb).count();
    return c;
}

PackedByteArray FineGen::generate_block_impl(Vector3i origin, Vector3i size, int lod, int bytes_per_voxel) const {
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
    const std::shared_ptr<const ColumnCtx> ctx = column_ctx(origin, size, lod);
    const WorldGen::Ruin *ruins = ctx->ruins;
    const int nruins = ctx->nruins;
    const technosim_ruin::Prepared *rprep = ctx->rprep;
    const std::vector<MacroPt> &lat = ctx->lat;
    const int hmax_m = ctx->hmax_m, wl_max = ctx->wl_max, fall_max = ctx->fall_max;
    const float hmin_m = ctx->hmin_m;
    const int y_top = origin.y + sy * step;
    const bool above_sea = y_top > sea;
    // Sky: at or above reach_top (below) with the footprint's highest feature at any height in place of the
    // block's own, so never lower than the full pass's bound and the answer is the same: all air.
    if (origin.y >= std::max({(hmax_m + 12) * FS, wl_max * FS + 1, fall_max * FS + 1, ctx->feat_top + 1, (hmax_m + 1) * FS + 12,
                              (lod >= 4 && above_sea) ? (hmax_m + 2) * FS + 112 : 0})) {
        s_sky += 1;
        return out;
    }
    // Deep rock (2026-10-25, fast flight vs generation): from LOD 1 on there are no ores and caves only
    // reach CAVE_MOUTH_DEPTH, so a block wholly below the sandstone, the cave mouths and the ground's
    // swells is all stone. One voxel of it tells the caller to fill the block (TerrainGenerator); the
    // per-voxel pass cost 0.16 ms a block and the clipboxes are half rock.
    uint16_t deep_rock = Mat::STONE;
    bool deep = uniform_blocks_ && lod >= 1 && origin.y >= 8 * FS && (float)y_top <= hmin_m * FS - (float)DEEP_ROCK_MARGIN
        && (lod > BIG_CAVE_LOD || !g.big_cave_possible(mx0, floor_div(origin.y, FS), mz0, mx0 + (nx - 1) * mstep, floor_div(y_top, FS), mz0 + (nz - 1) * mstep, (int)hmin_m));
    if (deep) {
        // the rock kind of the block (ADR 0057); where strata cross it, the full path
        const technosim_rock::RockCol rc4[4] = {rocks_.column(lx0, lz0, lat[0].hc * FS), rocks_.column(lx0 + (nx - 1) * L, lz0, lat[nx - 1].hc * FS),
            rocks_.column(lx0, lz0 + (nz - 1) * L, lat[(size_t)(nz - 1) * nx].hc * FS), rocks_.column(lx0 + (nx - 1) * L, lz0 + (nz - 1) * L, lat[(size_t)nz * nx - 1].hc * FS)};
        deep = rocks_.uniform(rc4, 4, origin.y, y_top, deep_rock);
    }
    if (deep) {
        const int bpv = (bytes_per_voxel == 1 || bytes_per_voxel == 4) ? bytes_per_voxel : 2;
        out.resize(bpv);
        uint8_t *o = out.ptrw();
        std::memset(o, 0, bpv);
        o[0] = (uint8_t)deep_rock;
        return out;
    }
    // Trees (lod <= 3 as shapes, from lod 4 only a leaf-coloured surface) and decorations (lod <= 2).
    std::vector<FTree> trees;
    std::vector<FStamp> stamps;
    const int x_lo = origin.x, x_hi = origin.x + (sx - 1) * step, z_lo = origin.z, z_hi = origin.z + (sz - 1) * step;
    int feature_top = 0;
    // Wholly below the ground's lowest point with the deep-rock margin: no tree, stamp or soil reaches here.
    const bool deep_geo = origin.y >= 8 * FS && (float)y_top <= hmin_m * FS - (float)DEEP_ROCK_MARGIN;
    if (above_sea && origin.y < (hmax_m + 40) * FS && !deep_geo) {
        // the footprint's trees and stamps (column_ctx), cut to the block's height band: the mature trees keep
        // their order, the young ones are swapped out (as the per-block search did)
        for (size_t i = 0; i < ctx->n_mature; i++) {
            const FTree &t = *ctx->trees[i];
            if (t.y_hi < origin.y || t.base >= y_top) continue;
            feature_top = std::max(feature_top, t.y_hi);
            trees.push_back(t);
        }
        std::vector<const FTree *> young;
        for (size_t i = ctx->n_mature; i < ctx->trees.size(); i++) young.push_back(ctx->trees[i].get());
        for (size_t i = 0; i < young.size();) {
            if (young[i]->y_hi < origin.y || young[i]->base >= y_top) { young[i] = young.back(); young.pop_back(); continue; }
            feature_top = std::max(feature_top, young[i]->y_hi);
            i++;
        }
        for (const FTree *t : young) trees.push_back(*t);
        stamps = ctx->stamps;
        for (const FStamp &s : stamps) feature_top = std::max(feature_top, s.ground + (int)(s.size + s.r) + 8);
    }
    // Nothing reaches this block: all air.
    const int far_canopy_top = (lod >= 4 && above_sea) ? (hmax_m + 2) * FS + 112 : 0;   // far forests stand ~14 m tall
    const int reach_top = std::max({(hmax_m + 12) * FS, wl_max * FS + 1, fall_max * FS + 1, feature_top + 1, (hmax_m + 1) * FS + 12, far_canopy_top});
    if (origin.y >= reach_top) return out;

    // Rock kinds (ADR 0057): strata per lattice point, interpolated per column like the surface.
    std::vector<technosim_rock::RockCol> rlat((size_t)nx * nz);
    for (int j = 0; j < nz; j++)
        for (int i = 0; i < nx; i++) rlat[(size_t)j * nx + i] = rocks_.column(lx0 + i * L, lz0 + j * L, lat[(size_t)j * nx + i].hc * FS);

    // Caves: a field on the macro lattice (lod 0, and cave mouths at lod 1), interpolated per voxel.
    const bool do_caves = lod <= BIG_CAVE_LOD;
    const bool do_ores = lod == 0;
    const bool do_veg = lod <= 1;
    int cy0 = 0, cny = 0;
    std::vector<float> cav;
    // A block over the ground still needs the field down to open_top's depth under the columns whose plants
    // reach into it, or grass stood in mid air over a sinkhole the block below had opened (bug report
    // 2026-11-02 18-55-21: the field was skipped above hmax_m, and the swells rise past it).
    int h_lo = INT32_MAX, h_hi = INT32_MIN;
    for (const int ch : ctx->h) {
        h_hi = std::max(h_hi, ch);
        if (ch >= origin.y - VEG_REACH) h_lo = std::min(h_lo, ch);
    }
    if (do_caves && y_top > 12 * FS && origin.y < std::max(hmax_m * FS, h_hi + VEG_REACH)) {
        cy0 = floor_div(origin.y - step, FS) - 2;   // 2 below: open_top looks 9 voxels under the ground
        if (h_lo != INT32_MAX) cy0 = std::min(cy0, floor_div(h_lo - 1 - 2 * CAVE_STEP, FS) - 1);
        // nothing is carved above a column's ground, so the field stops at the block's highest ground
        cny = std::max(2, floor_div(std::min(origin.y + sy * step, h_hi), FS) - cy0 + 2);
        cav.resize((size_t)nx * nz * cny);
        // Caverns from a coarse grid of their noise (every 2 m across, 1 m up; the halls are tens of metres),
        // ravines from two 2D noises per column: the big caves cost little even at coarse LODs.
        const bool big = g.variant_[WorldGen::VAR_CAVES] != 0;
        const int gx0 = floor_div(mx0, 4) * 4, gz0 = floor_div(mz0, 4) * 4, gy0 = floor_div(cy0, 2) * 2;
        const int gnx = (mx0 + (nx - 1) * mstep - gx0) / 4 + 2, gnz = (mz0 + (nz - 1) * mstep - gz0) / 4 + 2;
        const int gny = (cy0 + cny - 1 - gy0) / 2 + 2;
        std::vector<float> cg;
        if (big) {
            cg.resize((size_t)gnx * gnz * gny);
            for (int j = 0; j < gnz; j++)
                for (int i = 0; i < gnx; i++)
                    for (int k = 0; k < gny; k++)
                        cg[((size_t)j * gnx + i) * gny + k] = g.n_cavern_.GetNoise((float)(gx0 + 4 * i), (float)(gy0 + 2 * k) * WorldGen::CAVERN_Y, (float)(gz0 + 4 * j));
        }
        for (int j = 0; j < nz; j++)
            for (int i = 0; i < nx; i++) {
                const MacroPt &m = lat[(size_t)j * nx + i];
                const int cmx = mx0 + i * mstep, cmz = mz0 + j * mstep;
                const float rm = big ? g.n_ravmask_.GetNoise((float)cmx, (float)cmz) : 0.0f;
                const float line = rm > 0.25f ? std::fabs(g.n_ravine_.GetNoise((float)cmx, (float)cmz)) : 1.0f;
                const float pillar = big ? g.n_pillar_.GetNoise((float)cmx, (float)cmz) : -1.0f;
                const float gfx = (float)(cmx - gx0) / 4.0f, gfz = (float)(cmz - gz0) / 4.0f;
                const int gi = std::min((int)gfx, gnx - 2), gj = std::min((int)gfz, gnz - 2);
                const float gtx = gfx - gi, gtz = gfz - gj;
                WorldGen::CaveColumn ccol;   // the column's mouths and gallery offset, read at every height below
                g.cave_column(cmx, cmz, ccol);
                for (int k = 0; k < cny; k++) {
                    const int cy = cy0 + k;
                    const float depth = (float)(m.col.height - cy);
                    // lod 0: everything; lod 1: tunnels and mouths near the ground; caverns and ravines up to lod 2
                    float v = (lod == 0 || (lod == 1 && depth <= (float)(CAVE_MOUTH_DEPTH / FS + 2))) ? g.cave_value(ccol, cy, m.col.height, true) : -1.0f;
                    if (big) {
                        const float gfy = (float)(cy - gy0) / 2.0f;
                        const int gk = std::min((int)gfy, gny - 2);
                        const float gty = gfy - gk;
                        auto at = [&](int a, int b) {
                            const float *p = &cg[((size_t)(gj + b) * gnx + gi + a) * gny + gk];
                            return p[0] + (p[1] - p[0]) * gty;
                        };
                        const float raw = bilerp(at(0, 0), at(1, 0), at(0, 1), at(1, 1), gtx, gtz);
                        v = std::max({v, WorldGen::cavern_value(raw, (float)cy, depth, pillar), WorldGen::ravine_value(rm, line, depth)});
                    }
                    cav[((size_t)j * nx + i) * cny + k] = v;
                }
            }
    }
    // Crystal geodes reaching into the block (lod 0-1; deep in the rock they are only ever seen opened)
    WorldGen::Geode geodes[4];
    int ngeo = 0;
    if (lod <= 1 && g.variant_[WorldGen::VAR_CAVES] != 0) {
        const int gx0 = floor_div(floor_div(x_lo, FS) - 16, WorldGen::GEODE_CELL), gx1 = floor_div(floor_div(x_hi, FS) + 16, WorldGen::GEODE_CELL);
        const int gz0 = floor_div(floor_div(z_lo, FS) - 16, WorldGen::GEODE_CELL), gz1 = floor_div(floor_div(z_hi, FS) + 16, WorldGen::GEODE_CELL);
        for (int cz = gz0; cz <= gz1 && ngeo < 4; cz++)
            for (int cx = gx0; cx <= gx1 && ngeo < 4; cx++) {
                WorldGen::Geode gd;
                if (!g.geode_at_cell(cx, cz, gd)) continue;
                const float R = gd.r * FS + 2.0f;
                if (gd.x * FS + R < x_lo || gd.x * FS - R > x_hi || gd.z * FS + R < z_lo || gd.z * FS - R > z_hi) continue;
                if (gd.y * FS + R < origin.y || gd.y * FS - R > y_top) continue;
                geodes[ngeo++] = gd;
            }
    }
    // Deep rock at full detail (ADR 0105): a buried block where the cave field is negative everywhere it is
    // read (so no cave, pool, crystal, stringer or lava sea), no geode and no vein lens reaches, in one rock
    // kind, is that stone throughout; the per-voxel pass cost ~0.7 ms a block and half the box is rock.
    if (uniform_blocks_ && lod == 0 && deep_geo && ngeo == 0
        && std::all_of(cav.begin(), cav.end(), [](float v) { return v <= 0.0f; })) {
        bool vein_free = true;
        const int my_lo = floor_div(origin.y, FS), my_hi = floor_div(y_top, FS);
        const int bx_lo = floor_div(x_lo, FS), bx_hi = floor_div(x_hi, FS), bz_lo = floor_div(z_lo, FS), bz_hi = floor_div(z_hi, FS);
        for (int cz = floor_div(bz_lo - 64, technosim_ore::CELL); cz <= floor_div(bz_hi + 64, technosim_ore::CELL) && vein_free; cz++)
            for (int cx = floor_div(bx_lo - 64, technosim_ore::CELL); cx <= floor_div(bx_hi + 64, technosim_ore::CELL) && vein_free; cx++) {
                WorldGen::Vein v;
                if (!g.vein_at_cell(cx, cz, v)) continue;
                // the lens at its widest and thickest (vein_half_thickness: radius 0.5 SIZE x <= 1, half THICKNESS)
                const float r = 0.5f * (float)technosim_ore::SIZE[v.type] + 1.0f, t = 0.5f * (float)technosim_ore::THICKNESS[v.type] + 1.0f;
                if ((float)my_hi < (float)v.y - t || (float)my_lo > (float)v.y + t) continue;
                const float dx = std::max({0.0f, (float)(bx_lo - v.x), (float)(v.x - bx_hi)}), dz = std::max({0.0f, (float)(bz_lo - v.z), (float)(v.z - bz_hi)});
                if (dx * dx + dz * dz < r * r) vein_free = false;
            }
        uint16_t rock = Mat::STONE;
        if (vein_free) {
            const technosim_rock::RockCol rc4[4] = {rocks_.column(lx0, lz0, lat[0].hc * FS), rocks_.column(lx0 + (nx - 1) * L, lz0, lat[nx - 1].hc * FS),
                rocks_.column(lx0, lz0 + (nz - 1) * L, lat[(size_t)(nz - 1) * nx].hc * FS), rocks_.column(lx0 + (nx - 1) * L, lz0 + (nz - 1) * L, lat[(size_t)nz * nx - 1].hc * FS)};
            if (rocks_.uniform(rc4, 4, origin.y, y_top, rock)) {
                const int bpv = (bytes_per_voxel == 1 || bytes_per_voxel == 4) ? bytes_per_voxel : 2;
                out.resize(bpv);
                uint8_t *o = out.ptrw();
                std::memset(o, 0, bpv);
                o[0] = (uint8_t)rock;
                return out;
            }
        }
    }
    const int n = sx * sy * sz;
    std::vector<uint16_t> tmp(n, 0);
    uint16_t *w = tmp.data();
    bool any = false;
    std::vector<uint8_t> cave(sy + 2);
    // The cave field at any voxel of the block's lattice (neighbour columns of a cave pool).
    auto cave_at = [&](int wx, int wy, int wz) {
        if (cav.empty()) return false;
        const float fx = (float)(wx - lx0) / FS, fz = (float)(wz - lz0) / FS, fy = cave_band_y(wy) / FS - (float)cy0;
        const int ci = std::clamp((int)std::floor(fx), 0, nx - 2), cj = std::clamp((int)std::floor(fz), 0, nz - 2);
        const int k = std::clamp((int)std::floor(fy), 0, cny - 2);
        const float tx = std::clamp(fx - ci, 0.0f, 1.0f), tz = std::clamp(fz - cj, 0.0f, 1.0f), ty = std::clamp(fy - (float)k, 0.0f, 1.0f);
        auto lerp_y = [&](const float *p) { return p[k] + (p[k + 1] - p[k]) * ty; };
        return bilerp(lerp_y(&cav[((size_t)cj * nx + ci) * cny]), lerp_y(&cav[((size_t)cj * nx + ci + 1) * cny]),
                      lerp_y(&cav[((size_t)(cj + 1) * nx + ci) * cny]), lerp_y(&cav[((size_t)(cj + 1) * nx + ci + 1) * cny]), tx, tz) > 0.0f;
    };
    // A cave pool cell needs rock on all four sides at its height, so its surface never stands over a
    // lower floor as a wall (bug report 2026-10-25 02-20-46: sludge draped over a slope).
    auto enclosed = [&](int wx, int wy, int wz) {
        return !cave_at(wx - step, wy, wz) && !cave_at(wx + step, wy, wz) && !cave_at(wx, wy, wz - step) && !cave_at(wx, wy, wz + step);
    };
    struct VeinCol {
        bool done = false, has = false;
        WorldGen::Vein vein{};
        float ht = 0.0f;
        int stringer = -1;
    };
    const int vmx0 = floor_div(origin.x, FS), vmz0 = floor_div(origin.z, FS);
    const int vnx = floor_div(origin.x + (sx - 1) * step, FS) - vmx0 + 1, vnz = floor_div(origin.z + (sz - 1) * step, FS) - vmz0 + 1;
    std::vector<VeinCol> vcols((size_t)vnx * vnz);
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
            const size_t ri = (size_t)j * nx + i;
            const technosim_rock::RockCol rc = technosim_rock::RockTypes::blend(rlat[ri], rlat[ri + 1], rlat[ri + nx], rlat[ri + nx + 1], tx, tz, rlat[ri + (ez ? nx : 0) + (ex ? 1 : 0)]);
            const int mx = floor_div(wx, FS), mz = floor_div(wz, FS);
            const int h = ctx->h[(size_t)x + (size_t)sx * z];   // the column's ground (column_ctx)
            // the block lies wholly under this column's top voxel (h - 1): plants, tufts, sticks, leaf litter and
            // seagrass touch only that voxel and the air above it, so they are skipped (ADR 0105)
            const bool buried = y_top <= h - 1;
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
                    // plain bilinear weights: a floor on them (once 0.02) made the surface jump by up to a macro step
                    // (4 voxels) at every lattice cell border where the set of wet corners changes (ADR 0069 C)
                    sum += ws[k] * (float)cs[k]->water_level;
                    wsum += ws[k];
                }
                if (wsum > 0.0f) {
                    const int wl = (int)std::floor(sum / wsum * FS + 0.5f);
                    if ((hy.river > 0.0f || hy.lake) && hy.water_level >= 0) {
                        water_level = wl;
                        // ADR 0069 A3: on a steep river the ground rises above the interpolated level on part of
                        // every step, and those columns stood dry: the cascade drew as stripes of water panels with
                        // ground between. A channel column up to 1 m above the level carries a one-voxel sheet, so
                        // the falling river is one wet surface that follows its bed
                        if (hy.river > 0.0f && !hy.lake && h >= water_level && h < water_level + 2 * FS)
                            water_level = h + 1;
                    }
                    else if (wl > water_level && h < wl) { water_level = wl; side_fill = true; }
                }
            }
            const bool lakebed = (m.lakebed || side_fill) && h < water_level, riverbed = m.riverbed, marshbed = m.marshbed;
            const WorldGen::Ruin *ruin = m.ruin >= 0 ? &ruins[m.ruin] : nullptr;
            const int spike = m.spike * FS;
            const uint16_t spire_m = spike > 0 ? (uint16_t)technosim_biome::SPIRE_MATERIAL[col.biome] : Mat::AIR;
            const bool volcanic = col.biome == technosim_biome::VOLCANIC;   // bare basalt above the tree line too
            const bool steep = slope > 1.1f, scree = slope > 0.75f && slope <= 1.1f && col.alpine && !volcanic;
            const bool cold = col.t < -0.45f || col.snow;
            const uint32_t ph = h3(wx >> 1, wz >> 1, g.seed_ + 19);   // 25 cm patches
            uint16_t top_m, under_m;
            if (lakebed || riverbed || marshbed) {
                const uint32_t bh = ph & 7;
                top_m = bh < 3 ? Mat::CLAY : (bh < 5 ? Mat::SAND_STATIC : Mat::GRAVEL_STATIC);
                under_m = (bh & 1) ? Mat::CLAY : Mat::GRAVEL_STATIC;
            } else if (hy.river > 0.08f && h <= water_level + 2 * FS) {
                top_m = under_m = Mat::SAND_STATIC;
            } else if (col.rock) {
                top_m = under_m = col.rock;   // a monolith: bare rock, top and sides (ADR 0051)
            } else if (col.snow && !steep) {
                top_m = Mat::SNOW; under_m = Mat::STONE;
            } else if (steep) {
                top_m = under_m = technosim_biome::SURFACE_UNDER[col.biome] == Mat::TERRACOTTA || technosim_biome::SURFACE_UNDER[col.biome] == Mat::BASALT ? (uint16_t)technosim_biome::SURFACE_UNDER[col.biome] : Mat::STONE;
            } else if (scree) {
                top_m = under_m = Mat::GRAVEL_STATIC;
            } else if (col.alpine && !volcanic) {
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
                if (col.biome == WorldGen::BIOME_OCEAN || col.biome == WorldGen::BIOME_DEEP_OCEAN || col.biome == technosim_biome::FROZEN_OCEAN)
                    top_m = under_m = g.seabed(col, slope);
                if (technosim_biome::SURFACE_STEEP[col.biome] >= 0 && slope > 0.6f) top_m = (uint16_t)technosim_biome::SURFACE_STEEP[col.biome];
                if (top_m == Mat::SAVANNA_GRASS) {
                    // bare red earth between the tussocks, in patches a few metres across with ragged
                    // edges (bug report 2026-10-25 20-37-22: the savanna looked like a lush meadow)
                    const float bare = n_meadow_.GetNoise((float)wx * 0.8f + 9100.0f, (float)wz * 0.8f - 7300.0f);
                    if (bare > 0.36f || (bare > 0.2f && (ph & 3) == 0)) top_m = Mat::RED_EARTH;
                }
            }
            if (spike > 0) top_m = under_m = spire_m;
            under_m = rocks_.soil(rc, under_m, h, wx, wz);   // ochre patches (ADR 0057)
            const bool strata = under_m == Mat::TERRACOTTA;
            const int soil = SOIL_FINE + (int)(ph >> 8) % 4;
            const int sandstone_depth = FS * (col.biome == technosim_biome::BADLANDS ? 24 : col.biome == WorldGen::BIOME_DESERT ? 16
                : ((col.biome == WorldGen::BIOME_BEACH || col.biome == technosim_biome::SNOWY_BEACH) ? 8 : 0));
            const int ti = technosim_biome::band_of(technosim_biome::TEMP_BAND_MAX, col.t);
            const int hi = technosim_biome::band_of(technosim_biome::HUM_BAND_MAX, col.h);
            // ore vein of this macro column's cell
            // (looked up once per macro column of the block: 16 fine columns share it)
            VeinCol &vc = vcols[(size_t)(mz - vmz0) * vnx + (mx - vmx0)];
            if (!vc.done) {
                vc.done = true;
                vc.has = g.vein_at_cell(floor_div(mx, technosim_ore::CELL), floor_div(mz, technosim_ore::CELL), vc.vein);
                vc.ht = (vc.has && do_ores) ? g.vein_half_thickness(vc.vein, mx, mz) : 0.0f;
                vc.stringer = (do_ores && !cav.empty()) ? g.stringer_type(floor_div(mx, technosim_ore::CELL), floor_div(mz, technosim_ore::CELL)) : -1;
            }
            const WorldGen::Vein &vein = vc.vein;
            const bool has_vein = vc.has;
            const float vein_ht = vc.ht;
            const int stringer = vc.stringer;
            // Plants: the macro column decides the kind, the fine column whether it stands here.
            uint16_t veg = Mat::AIR, veg_head = Mat::AIR;
            int veg_h = 0;
            bool pickup = false;   // the macro column holds a ground pickup: its 0.5 m stays bare so the pickup shows
            if (!buried && do_veg && !ruin && h >= water_level && hy.fall_top < 0 && h > sea && spike == 0 && !lakebed && !riverbed) {
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
                        if (is_flower(kind) || kind == Mat::MUSHROOM_BROWN || kind == Mat::MUSHROOM_RED) {
                            // flowers and mushrooms are clutter stamps of whole voxels now (ADR 0047)
                            if (grassy && (hv & 7) < 2) { veg = Mat::TALL_GRASS; veg_h = 1; }
                        } else if (pickup) {
                            veg_h = pickup_shape(kind, lx, lz, h3(mx, mz, g.seed_ + 41));
                            if (veg_h > 0) veg = kind == Mat::PEBBLES ? rocks_.loose(rc, h) : kind;   // loose stones of the local rock (ADR 0057)
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
                    const float rr = s.kind >= FLOWER_S ? 2.6f : s.kind == GIANT_FLOWER ? s.r + 18.0f : std::max(s.size * 1.3f, s.size + s.r) + 6.0f;
                    if (std::fabs((float)wx - s.x) <= rr && std::fabs((float)wz - s.z) <= rr) cst[ncs++] = &s;
                }
            }
            // The forest floor and open grass (2026-10-23): leaf litter, ferns, mushrooms at the foot of
            // trunks and twigs under the canopy; grass that thickens and thins in patches, with patches of
            // one kind of flower, in the open.
            if (!buried && (top_m == Mat::GRASS || top_m == Mat::SAVANNA_GRASS || top_m == Mat::RED_EARTH) && !ruin && spike == 0 && h > water_level && hy.fall_top < 0) {
                const bool dry = top_m != Mat::GRASS;   // the savanna floor
                const bool forest = !dry && technosim_biome::TREE_DENSITY[col.biome] >= 0.2f;
                const bool under = forest && shaded;
                const bool jungle = col.biome == technosim_biome::JUNGLE;
                if (under && n_litter_.GetNoise((float)wx, (float)wz) > (jungle ? 0.25f : -0.4f)) top_m = Mat::LEAF_LITTER;   // the jungle floor stays green
                if (do_veg && h > sea && !pickup && (veg == Mat::AIR || veg == Mat::TALL_GRASS) && veg_head == Mat::AIR) {
                    const uint32_t fh = h3(wx, wz, g.seed_ + 31);
                    const uint32_t r10 = fh & 1023;
                    veg = Mat::AIR;
                    veg_h = 0;
                    if (jungle) {
                        // waist-high ferns over most of the floor, grass between (bug report 2026-11-02 18-54-25)
                        if (r10 < 520) { veg = Mat::FERN; veg_h = 2 + (int)((fh >> 12) % 5); }
                        else if (r10 < 760) { veg = Mat::TALL_GRASS; veg_h = 1 + (int)((fh >> 12) % 3); }
                    } else if (under) {
                        bool near_wood = false;
                        for (int k = 0; k < nct && !near_wood; k++) {
                            if (!ctrees[k].trunk) continue;
                            const float dx = (float)wx - ctrees[k].t->x, dz = (float)wz - ctrees[k].t->z, rr = ctrees[k].t->r + 5.0f;
                            near_wood = dx * dx + dz * dz <= rr * rr;
                        }
                        if (near_wood && r10 < 150) { veg = Mat::FERN; veg_h = 1 + (int)((fh >> 12) & 1); }   // mushrooms are stamps (ADR 0047)
                        else if (r10 < 300) { veg = Mat::FERN; veg_h = 2 + (int)((fh >> 12) % 3); }
                        else if (r10 < 420) { veg = Mat::TALL_GRASS; veg_h = 1 + (int)((fh >> 12) & 1); }
                    } else if (dry) {
                        // savanna: tussocks of straw grass in clumps, up to knee high, the red earth between them
                        // nearly bare; no flowers, ferns or litter
                        const float mead = n_meadow_.GetNoise((float)wx, (float)wz);
                        const float cover = top_m == Mat::RED_EARTH ? 0.02f : std::max(0.025f, 0.045f + 0.17f * mead);
                        if ((float)r10 / 1024.0f < cover) {
                            const uint32_t th = (fh >> 10) % 20;
                            veg = Mat::SAVANNA_TALL_GRASS;
                            veg_h = (th < 7 ? 1 : (th < 14 ? 2 : (th < 18 ? 3 : 4))) + (mead > 0.35f ? 1 : 0);
                        }
                    } else {
                        const float mead = n_meadow_.GetNoise((float)wx, (float)wz);
                        const float cover = 0.12f + 0.3f * mead + (forest ? 0.1f : 0.0f);   // every tuft costs ~10 faces
                        if ((float)r10 / 1024.0f < cover) {
                            const uint32_t th = (fh >> 10) % 20;
                            veg = Mat::TALL_GRASS;
                            veg_h = (th < 9 ? 1 : (th < 16 ? 2 : (th < 19 ? 3 : 4))) + (mead > 0.4f ? 1 : 0);
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
            if (!buried && do_veg && ncs > 0 && !pickup && h > water_level && hy.fall_top < 0) {
                float sd = 1e9f, th = 0.72f, fd = 1e9f;
                for (int k = 0; k < ncs; k++) {
                    if (cst[k]->kind != STICK) continue;
                    float f;
                    const float d = stick_dist(*cst[k], (float)wx, (float)wz, &f);
                    if (d - cst[k]->r < sd - th) { sd = d; th = cst[k]->r; }
                    fd = std::min(fd, f);
                }
                // round in section: thick sticks rise to two voxels along their middle; the side branch
                // is always one voxel thin
                if (sd <= th) { veg = Mat::TWIGS; veg_h = (th > 1.0f && sd <= th - 0.75f) ? 2 : 1; veg_head = Mat::AIR; }
                else if (fd <= 0.6f) { veg = Mat::TWIGS; veg_h = 1; veg_head = Mat::AIR; }
                else if (std::min(sd - th, fd - 0.6f) <= 1.8f) veg = veg_head = Mat::AIR;
                sd = std::min(sd, fd + th - 0.6f);
                // a rim of bare soil under and beside it, so the pale stick reads against grass and litter
                if (sd <= th + 0.9f && (top_m == Mat::GRASS || top_m == Mat::LEAF_LITTER)) top_m = Mat::DIRT;
                else if (sd <= th + 0.9f && top_m == Mat::SAVANNA_GRASS) top_m = Mat::RED_EARTH;
            }
            // clutter pieces standing on this column (ADR 0047): their voxels from their own ground up
            int clut_base = INT32_MIN;
            uint16_t clut[CLUTTER_H] = {};
            if (lod == 0 && ncs > 0 && hy.fall_top < 0) {
                for (int k = 0; k < ncs; k++) {
                    const FStamp &cs = *cst[k];
                    if (cs.kind < FLOWER_S) continue;
                    const int dx = wx - (int)std::floor(cs.x), dz = wz - (int)std::floor(cs.z);
                    if (dx < -2 || dx > 2 || dz < -2 || dz > 2) continue;
                    bool hit = false;
                    for (int dy = 0; dy < CLUTTER_H; dy++) hit |= (clut[dy] = clutter_voxel(cs, dx, dy, dz)) != Mat::AIR;
                    if (hit) {
                        clut_base = cs.ground;
                        veg = veg_head = Mat::AIR;
                        break;
                    }
                }
            }
            // kelp forests and seagrass on the sea floor (ADR 0047): whole voxels standing in the water,
            // (still water, which stays put when they are cut unless something disturbs it)
            uint16_t aqua = Mat::AIR;
            int aqua_h = 0;
            if (!buried && lod == 0 && h < water_level && hy.river <= 0.05f && (top_m == Mat::SAND || top_m == Mat::GRAVEL_STATIC || top_m == Mat::GRAVEL || top_m == Mat::SAND_STATIC || top_m == Mat::DIRT)) {
                const int depth = water_level - h;
                const uint32_t ah = h3(wx, wz, g.seed_ + 653);
                const bool sea_water = !hy.lake && !hy.marsh;
                constexpr int VPM = 2 * FS;   // voxels per metre
                if (sea_water && depth >= 2 * VPM && depth <= 30 * VPM && n_meadow_.GetNoise((float)wx * 0.3f + 1700.0f, (float)wz * 0.3f) > 0.2f && (ah & 31) == 0) {   // ~40 voxels of faces each: sparse
                    aqua = Mat::KELP;
                    aqua_h = std::max(2, (int)((float)std::min(depth * 9 / 10, 10 * VPM) * (0.45f + 0.55f * (float)((ah >> 4) & 255) / 255.0f)));
                } else if (depth <= 3 * VPM && n_meadow_.GetNoise((float)wx * 0.6f - 900.0f, (float)wz * 0.6f) > 0.0f && (ah & 7) < 2) {
                    aqua = Mat::SEAGRASS;
                    aqua_h = std::min(1 + (int)((ah >> 4) % 3), depth - 1);
                }
            }
            // the ruin whose region holds this column (ADR 0062: its shape decides the voxels in its reach)
            const technosim_ruin::Prepared *rp = nullptr;
            for (int k = 0; k < nruins && !rp; k++)
                if (WorldGen::in_ruin(ruins[k], mx, mz, 2)) rp = &rprep[k];
            // cave mask for the column, one voxel beyond the block on both ends
            const int no_cave_above = (hy.lake || hy.river > 0.0f || hy.marsh) ? water_level - 40 * FS : -1;
            const int cave_band = 1 << 30;   // coarse LODs carry only the big caves below the mouths (cave_field)
            if (!cav.empty()) {
                const float fx = (float)(wx - lx0) / FS, fz = (float)(wz - lz0) / FS;
                const int ci = std::min((int)fx, nx - 2), cj2 = std::min((int)fz, nz - 2);
                const float ctx = fx - ci, ctz = fz - cj2;
                const float *p00 = &cav[((size_t)cj2 * nx + ci) * cny], *p10 = &cav[((size_t)cj2 * nx + ci + 1) * cny];
                const float *p01 = &cav[((size_t)(cj2 + 1) * nx + ci) * cny], *p11 = &cav[((size_t)(cj2 + 1) * nx + ci + 1) * cny];
                for (int y = -1; y <= sy; y++) {
                    const int wy = origin.y + y * step;
                    bool c = wy >= 12 * FS && wy < h - m.roof * FS && h - wy <= cave_band && (no_cave_above < 0 || wy < no_cave_above) && !(wy < sea && h < sea + 2 * FS);
                    if (c) {
                        const float fy = cave_band_y(wy) / FS - (float)cy0;
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
            // where an entrance may cut the surface, no plant stands over the hole (the tufts in mid air over
            // sinkholes, developer 2026-10-27)
            // and a skin of ground thinner than two cave bands over the hole is opened too (the floating grass
            // slabs over sinkholes and ravines, bug report 2026-10-27 15-20-15)
            const bool open_top = m.roof == 0 && (cave_at(wx, h - 1, wz) || cave_at(wx, h - 1 - CAVE_STEP, wz) || cave_at(wx, h - 1 - 2 * CAVE_STEP, wz));
            if (open_top) {
                veg = veg_head = Mat::AIR;
                for (int y = -1; y <= sy; y++)
                    if (const int wy = origin.y + y * step; wy < h && wy >= h - 2 * CAVE_STEP) cave[y + 1] = 1;
            }
            int crystal_roll = 0;
            const int crystal_h = (lod == 0 && !cav.empty()) ? rocks_.crystal_height(wx, wz, crystal_roll) : 0;
            uint16_t *colp = w + sy * (x + sx * z);
            ValueNoiseCell sheet_cell;   // the stringer sheet's noise cell, shared up the column
            for (int y = 0; y < sy; y++) {
                const int wy = origin.y + y * step;
                uint16_t mat = Mat::AIR;
                if (rp && technosim_ruin::in_reach(rp->r, floor_div(wy, FS))) {
                    const uint16_t rm = technosim_ruin::voxel(*rp, FS, wx, wy, wz, h, top_m, lod == 0);   // pots at LOD 0 (ADR 0057)
                    if (rm != technosim_ruin::PASS) {
                        colp[y] = rm;
                        if (rm != Mat::AIR) any = true;
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
                if (mat == Mat::STONE) mat = rocks_.at(rc, wx, wy, wz);   // boulders of the local rock (ADR 0057)
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
                        mat = (wy < sea - 180 * FS) ? Mat::LAVA_STATIC : Mat::AIR;   // the deep lava sea: still until opened (ADR 0069 C)
                        if (mat == Mat::AIR && crystal_h > 0) {
                            // cave crystals (ADR 0057) on the floor below, when the floor and the whole spike lie in
                            // this block's 16-voxel band (a neighbour block never draws half of one)
                            int k = y;
                            while (k >= 0 && cave[k]) k--;   // cave[k] = voxel y = k - 1
                            const int floor_wy = origin.y + (k - 1) * step, dy = (wy - floor_wy) / step - 1;
                            if (k >= 0 && dy < crystal_h && ((floor_wy + 1) >> 4) == ((floor_wy + crystal_h) >> 4)) {
                                const float cn = g.n_cavebiome_.GetNoise((float)mx, (float)wy / FS, (float)mz);
                                const int di = technosim_biome::band_of(technosim_biome::DEPTH_BAND_MAX, (float)depth / FS, technosim_biome::DEPTH_BAND_COUNT);
                                const int ni = technosim_biome::band_of(technosim_biome::CAVE_NOISE_BAND_MAX, cn, technosim_biome::CAVE_NOISE_BAND_COUNT);
                                if (technosim_rock::RockTypes::crystal_allowed(crystal_roll, depth, technosim_biome::cave_biome(ti, hi, di, ni))) mat = (uint16_t)technosim_rock::CAVE_CRYSTAL;
                            }
                        }
                    } else if (d_top > 1 && n_soil * 2 < n_solid) {
                        if (d_top <= sandstone_depth) mat = strata ? g.strata_at(mx, floor_div(wy, FS), mz) : Mat::SANDSTONE;
                        else if (vein_ht > 0.0f) {
                            // GregTech layers in the lens as bunched lumps (ADR 0064)
                            const float dy = (float)wy / FS - (float)vein.y;
                            mat = (dy <= vein_ht && dy >= -vein_ht)
                                ? vein_voxel(vein, vein_ht, wx, wy, wz, rocks_.ore_bias(rc, wy, vein.type), g.seed_) : (uint16_t)Mat::STONE;
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
                            if (pool >= 0 && cave[y] == 0 && n_pool_.GetNoise((float)wx, (float)wz) > POOL_THR && enclosed(wx, wy, wz)) mat = (uint16_t)pool;
                        } else if (y + 3 < sy + 2 && cave[y + 3] && !cave[y + 2] && mat == Mat::STONE && depth >= technosim_biome::SOIL_DEPTH * FS) {
                            // the second voxel of a pool, under its surface voxel: pools are 25 cm deep
                            const float cn = g.n_cavebiome_.GetNoise((float)mx, (float)(wy + step) / FS, (float)mz);
                            const int di = technosim_biome::band_of(technosim_biome::DEPTH_BAND_MAX, (float)(depth - step) / FS, technosim_biome::DEPTH_BAND_COUNT);
                            const int ni = technosim_biome::band_of(technosim_biome::CAVE_NOISE_BAND_MAX, cn, technosim_biome::CAVE_NOISE_BAND_COUNT);
                            const int pool = technosim_biome::SURFACE_POOL[technosim_biome::cave_biome(ti, hi, di, ni)];
                            if (pool >= 0 && cave[y] == 0 && n_pool_.GetNoise((float)wx, (float)wz) > POOL_THR && enclosed(wx, wy + step, wz) && enclosed(wx, wy, wz)) mat = (uint16_t)pool;
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
                    for (int q = 0; q < ngeo && mat != Mat::AIR; q++) {
                        // a hollow lined with crystal (spikes pointing in), in a basalt shell (fine voxels)
                        const WorldGen::Geode &gd = geodes[q];
                        const float dx = (float)wx + 0.5f - gd.x * FS, dy = (float)wy + 0.5f - gd.y * FS, dz = (float)wz + 0.5f - gd.z * FS;
                        const float R = gd.r * FS, d = std::sqrt(dx * dx + dy * dy + dz * dz);
                        if (d > R) continue;
                        if (d > R - 3.0f) mat = Mat::BASALT;
                        else if (d > R - 6.0f) mat = Mat::CRYSTAL;
                        else {
                            const uint32_t sh = h3(wx >> 1, wy >> 1, (wz >> 1) + (int)gd.seed);
                            mat = (d > R - 6.0f - (float)(sh & 7) * 0.9f && ((sh >> 3) & 3) != 0) ? Mat::CRYSTAL : Mat::AIR;
                        }
                        break;
                    }
                } else if ((water_level - wy) * 2 >= step) {
                    const bool inland = hy.lake || hy.river > 0.0f || hy.marsh || side_fill;
                    if (wy + step >= water_level && cold && (hy.lake || hy.marsh)) mat = Mat::ICE;
                    else if (wy + step >= water_level && !inland && technosim_biome::SURFACE_ICE[col.biome] >= 0) mat = (uint16_t)technosim_biome::SURFACE_ICE[col.biome];
                    else mat = Mat::WATER_STATIC;   // rivers, lakes and the sea: still until disturbed, then finite (ADR 0068)
                } else if (far_canopy && wy < far_top) {
                    mat = far_leaves;
                } else if (veg != Mat::AIR && wy < h + veg_h) {
                    mat = veg;
                } else if (veg_head != Mat::AIR && wy == h + veg_h) {
                    mat = veg_head;
                }
                if (mat == Mat::STONE && stringer >= 0 && h - wy >= STRINGER_MIN_DEPTH) {
                    // Ore stringers (ADR 0077): thin wavy sheets of ore through the rock, in the cell's ore, that show
                    // where they cross a cave wall; only within STRINGER_REACH of cave air so the lenses stay the prize.
                    const float s = value_noise3((float)wx / 16.0f, (float)wy / 11.0f, (float)wz / 16.0f, g.seed_ + 7801, sheet_cell);
                    if (std::fabs(s - 0.5f) < STRINGER_HALF) {
                        bool near = false;
                        for (int k = std::max(0, y + 1 - STRINGER_REACH / step); k <= std::min(sy + 1, y + 1 + STRINGER_REACH / step) && !near; k++) near = cave[k];
                        if (!near) near = cave_at(wx - STRINGER_REACH, wy, wz) || cave_at(wx + STRINGER_REACH, wy, wz) || cave_at(wx, wy, wz - STRINGER_REACH) || cave_at(wx, wy, wz + STRINGER_REACH);
                        if (near) {
                            // richer deeper: 8 % of the sheet in clumps at 20 m down, 18 % from 200 m (threshold
                            // from the value noise's spread, measured)
                            const float thr = 0.765f - 0.085f * smoothstep(80.0f * FS, 400.0f * FS, (float)(h - wy));
                            constexpr float C = STRINGER_CLUMP;
                            if (value_noise3((float)wx / C, (float)wy / C, (float)wz / C, g.seed_ + 7811) > thr) {
                                // one ore per clump-sized cell, so a clump digs out mostly as one ore
                                const uint32_t hs = h3(floor_div(wx, 4), floor_div(wz, 4), floor_div(wy, 4) * 1031 + g.seed_ + 7807);
                                mat = (uint16_t)((hs & 3) == 0 ? technosim_ore::SECONDARY[stringer] : technosim_ore::PRIMARY[stringer]);
                            }
                        }
                    }
                }
                if (mat == Mat::STONE) mat = rocks_.at(rc, wx, wy, wz);   // the local rock kind (ADR 0057)
                if (mat == Mat::AIR && clut_base != INT32_MIN && wy >= clut_base && wy < clut_base + CLUTTER_H) mat = clut[wy - clut_base];
                else if (mat == Mat::WATER_STATIC && aqua_h > 0 && wy >= h && wy < h + aqua_h) mat = aqua;
                colp[y] = mat;
                if (mat != Mat::AIR) any = true;
            }
        }
    }
    if (!any) return out;
    // No lone liquid cells (ADR 0069 C): a still liquid cell with air above, no liquid of its kind on any side and
    // open air beside it (a one-cell puddle on a cave wall's step, a scrap at a bank) is dropped; it drew as a spike
    // or a film and woke the simulation for nothing. Cells on the block's edge keep (their neighbours are unknown).
    {
        auto still = [](uint16_t m) { return m == Mat::WATER_STATIC || m == Mat::TOXIC_SLUDGE_STATIC || m == Mat::LAVA_STATIC; };
        for (int z = 1; z < sz - 1; z++)
            for (int x = 1; x < sx - 1; x++) {
                uint16_t *cp = w + sy * (x + sx * z);
                for (int y = 0; y < sy - 1; y++) {
                    const uint16_t m = cp[y];
                    if (!still(m) || cp[y + 1] != Mat::AIR) continue;
                    const uint16_t nxm = w[y + sy * (x - 1 + sx * z)], pxm = w[y + sy * (x + 1 + sx * z)];
                    const uint16_t nzm = w[y + sy * (x + sx * (z - 1))], pzm = w[y + sy * (x + sx * (z + 1))];
                    if (nxm == m || pxm == m || nzm == m || pzm == m) continue;
                    if (nxm == Mat::AIR || pxm == Mat::AIR || nzm == Mat::AIR || pzm == Mat::AIR) cp[y] = Mat::AIR;
                }
            }
    }
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

PackedInt32Array FineGen::surface_map(int x0, int z0, int n, int step) const {
    PackedInt32Array out;
    if (n <= 0 || step <= 0) return out;
    out.resize(n * n * 3);
    int32_t *o = out.ptrw();
    for (int j = 0; j < n; j++)
        for (int i = 0; i < n; i++) {
            const int x = x0 + i * step, z = z0 + j * step;
            const int h = height_at(x, z);
            const MacroPt m = macro_point(floor_div(x, FS), floor_div(z, FS), nullptr, 0);
            const int wl = m.water_level * FS;
            int cover = 0;
            if (m.col.snow) cover = 2;
            else if (m.col.alpine || m.col.slope > 1.1f) cover = 1;
            else if (technosim_biome::SURFACE_TOP[m.col.biome] == Mat::SAND || technosim_biome::SURFACE_TOP[m.col.biome] == Mat::SAND_STATIC) cover = 3;
            else if (technosim_biome::TREE_DENSITY[m.col.biome] >= 0.2f) cover = 4;
            int32_t *c = o + (j * n + i) * 3;
            c[0] = h;
            c[1] = wl > h ? wl : -1;
            c[2] = cover;
        }
    return out;
}

PackedByteArray FineGen::cave_map(int x0, int z0, int n, int step) const {
    PackedByteArray out;
    if (n <= 0 || step <= 0) return out;
    out.resize(n * n * 2);
    uint8_t *o = out.ptrw();
    const WorldGen &g = *g_.ptr();
    for (int j = 0; j < n; j++)
        for (int i = 0; i < n; i++) {
            const int mx = floor_div(x0 + i * step, FS), mz = floor_div(z0 + j * step, FS);
            const MacroPt m = macro_point(mx, mz, nullptr, 0);
            const int h = (int)std::floor(m.hc);
            const bool wet = m.hy.lake || m.hy.river > 0.0f || m.hy.marsh || h < g.sea_ + 2;
            int air = 0, total = 0;
            bool open = false;
            for (int y = 12; y < h; y += 2) {
                total++;
                if (wet || y >= h - m.roof) continue;
                if (g.cave_value(mx, y, mz, m.col.height) > 0.0f) {
                    air++;
                    if (y >= h - 3) open = true;
                }
            }
            o[(j * n + i) * 2] = (uint8_t)(total > 0 ? air * 255 / total : 0);
            o[(j * n + i) * 2 + 1] = open ? 1 : 0;
        }
    return out;
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
    // No waterfalls (the developer, 2026-10-26, ADR 0068): the hanging still-water sheet is gone, and with it the
    // spray, the roar and the landmark. The river still drops over its cliff (world_gen keeps the lip and the
    // channel); a waterfall of real falling water waits for flowing rivers.
    if (true) return Array();
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

Dictionary FineGen::ruin_at(int x, int z) const {
    Dictionary d = g_->ruin_at(floor_div(x, FS), floor_div(z, FS));
    if (d.is_empty()) return d;
    d["x"] = (int)d["x"] * FS + 2;
    d["y"] = (int)d["y"] * FS;
    d["z"] = (int)d["z"] * FS + 2;
    d["half_x"] = (int)d["half_x"] * FS;
    d["half_z"] = (int)d["half_z"] * FS;
    return d;
}

Array FineGen::ruin_pots_near(int x, int z, int radius) const {
    Array a = g_->ruin_pots_near(floor_div(x, FS), floor_div(z, FS), std::max(1, radius / FS));
    for (int i = 0; i < a.size(); i++) {
        Dictionary d = a[i];
        d["x"] = (int)d["x"] * FS + 2;
        d["y"] = (int)d["y"] * FS;
        d["z"] = (int)d["z"] * FS + 2;
    }
    return a;
}
