#include "world_gen.h"

#include <godot_cpp/core/class_db.hpp>

#include <algorithm>
#include <cmath>
#include <cstring>
#include <vector>

using namespace godot;

WorldGen::WorldGen() { configure(1337, 256, 768); }

void WorldGen::_bind_methods() {
    ClassDB::bind_method(D_METHOD("configure", "seed", "sea_level", "world_height"), &WorldGen::configure);
    ClassDB::bind_method(D_METHOD("get_seed"), &WorldGen::get_seed);
    ClassDB::bind_method(D_METHOD("height_at", "x", "z"), &WorldGen::height_at);
    ClassDB::bind_method(D_METHOD("biome_at", "x", "z"), &WorldGen::biome_at);
    ClassDB::bind_method(D_METHOD("column_info", "x", "z"), &WorldGen::column_info);
    ClassDB::bind_method(D_METHOD("generate_block", "origin", "size", "lod", "bytes_per_voxel"), &WorldGen::generate_block, DEFVAL(2));
    ClassDB::bind_method(D_METHOD("map_image", "center_x", "center_z", "radius", "step"), &WorldGen::map_image);
    ClassDB::bind_static_method("WorldGen", D_METHOD("biome_name", "biome"), &WorldGen::biome_name);
    ClassDB::bind_method(D_METHOD("find_nearest", "kind", "x", "z", "max_radius"), &WorldGen::find_nearest);
    BIND_ENUM_CONSTANT(BIOME_DEEP_OCEAN);
    BIND_ENUM_CONSTANT(BIOME_OCEAN);
    BIND_ENUM_CONSTANT(BIOME_BEACH);
    BIND_ENUM_CONSTANT(BIOME_PLAINS);
    BIND_ENUM_CONSTANT(BIOME_FOREST);
    BIND_ENUM_CONSTANT(BIOME_DESERT);
    BIND_ENUM_CONSTANT(BIOME_TUNDRA);
    BIND_ENUM_CONSTANT(BIOME_HIGHLANDS);
    BIND_ENUM_CONSTANT(BIOME_MOUNTAINS);
    BIND_ENUM_CONSTANT(BIOME_SNOWY_PEAKS);
    BIND_ENUM_CONSTANT(BIOME_RIVER);
}

static void setup(FastNoiseLite &n, int seed, float freq, int octaves, FastNoiseLite::FractalType ft = FastNoiseLite::FractalType_FBm) {
    n.SetSeed(seed);
    n.SetNoiseType(FastNoiseLite::NoiseType_OpenSimplex2);
    n.SetFrequency(freq);
    n.SetFractalType(octaves > 1 ? ft : FastNoiseLite::FractalType_None);
    n.SetFractalOctaves(octaves);
    n.SetFractalLacunarity(2.0f);
    n.SetFractalGain(0.5f);
}

void WorldGen::configure(int p_seed, int p_sea_level, int p_world_height) {
    seed_ = p_seed;
    sea_ = p_sea_level;
    world_height_ = p_world_height;
    // Frequencies in 1/voxels (0.5 m voxels): continents ~3 km, erosion ~1.2 km,
    // peaks ~400 m, climate ~2.5 km, caves ~50 m.
    setup(n_cont_, seed_ + 1, 0.00033f, 3);
    setup(n_eros_, seed_ + 2, 0.0008f, 3);
    setup(n_pv_, seed_ + 3, 0.0025f, 3, FastNoiseLite::FractalType_Ridged);
    setup(n_detail_, seed_ + 4, 0.02f, 2);
    setup(n_temp_, seed_ + 5, 0.0004f, 2);
    setup(n_hum_, seed_ + 6, 0.0005f, 2);
    setup(n_cheese_, seed_ + 7, 0.011f, 2);
    setup(n_spag1_, seed_ + 8, 0.018f, 1);
    setup(n_spag2_, seed_ + 9, 0.018f, 1);
    setup(n_ore_, seed_ + 10, 0.09f, 1);
    setup(n_beach_, seed_ + 11, 0.01f, 1);
    setup(n_river_, seed_ + 12, 0.0009f, 2);
    setup(n_jitter_, seed_ + 13, 0.02f, 2);
}

uint32_t WorldGen::hash3(int a, int b, int c) {
    uint32_t h = (uint32_t)a * 374761393u + (uint32_t)b * 668265263u + (uint32_t)c * 2246822519u;
    h ^= h >> 13;
    h *= 1274126177u;
    h ^= h >> 16;
    return h;
}

// Piecewise-linear spline through (xs, ys), clamped at the ends.
float WorldGen::spline(float v, const float *xs, const float *ys, int n) {
    if (v <= xs[0]) return ys[0];
    for (int i = 1; i < n; i++) {
        if (v <= xs[i]) {
            float t = (v - xs[i - 1]) / (xs[i] - xs[i - 1]);
            return ys[i - 1] + t * (ys[i] - ys[i - 1]);
        }
    }
    return ys[n - 1];
}

static inline float smoothstep(float a, float b, float x) {
    float t = std::clamp((x - a) / (b - a), 0.0f, 1.0f);
    return t * t * (3.0f - 2.0f * t);
}

WorldGen::Column WorldGen::column(int x, int z) const {
    const float fx = (float)x, fz = (float)z;
    Column col;
    col.c = n_cont_.GetNoise(fx, fz);
    col.e = n_eros_.GetNoise(fx, fz);
    float pv_raw = n_pv_.GetNoise(fx, fz); // ridged: around -1..1, sharp ridges near 1
    col.pv = pv_raw;
    // Biome blending: jitter the climate values with a fine noise so biome
    // borders dither over ~20 m instead of a hard line.
    const float jit = n_jitter_.GetNoise(fx, fz);
    const float jit2 = n_jitter_.GetNoise(fz + 1000.0f, fx - 1000.0f);
    col.t = n_temp_.GetNoise(fx, fz) + jit * 0.09f;
    col.h = n_hum_.GetNoise(fx, fz) + jit2 * 0.09f;

    // Base height from continentalness (voxels relative to sea level).
    static const float cx[] = {-1.0f, -0.6f, -0.25f, -0.08f, 0.05f, 0.3f, 0.55f, 0.8f, 1.0f};
    static const float cy[] = {-80.0f, -50.0f, -12.0f, -2.0f, 4.0f, 24.0f, 80.0f, 180.0f, 240.0f};
    float base = spline(col.c, cx, cy, 9);
    // Erosion: high erosion flattens relief.
    static const float ex[] = {-1.0f, -0.3f, 0.4f, 1.0f};
    static const float ey[] = {1.25f, 1.0f, 0.45f, 0.15f};
    float relief_scale = spline(col.e, ex, ey, 4);
    // Peaks & valleys: ridged noise adds mountains where continentalness is high.
    // Max height ≈ sea + 240 + 290*1.25 + 4 ≈ sea + 606 < 768 ceiling.
    float mountain_amp = 30.0f + 260.0f * smoothstep(0.25f, 0.85f, col.c);
    float relief = (col.pv * 0.5f + 0.5f) * mountain_amp * relief_scale;
    // Under water, damp relief so the sea floor is gentle.
    if (base < 0.0f) relief *= 0.25f;
    float detail = n_detail_.GetNoise(fx, fz) * 4.0f;
    float hf = (float)sea_ + base + relief + detail;

    // Rivers: a narrow band where the river noise crosses zero. Surface at sea
    // level (Minecraft style), bed 3 below, banks blend over the mask. Rivers
    // cut valleys through hills; deep canyons through mountains.
    col.river = 0.0f;
    if (hf > (float)sea_ - 4.0f && col.c > -0.3f) {
        float rn = std::fabs(n_river_.GetNoise(fx, fz));
        float width = 0.035f + 0.02f * smoothstep(0.1f, 0.7f, col.c); // wider rivers inland
        float mask = 1.0f - smoothstep(0.0f, width, rn);
        if (mask > 0.0f) {
            float bank = smoothstep(0.0f, 1.0f, mask);
            float target = (float)sea_ - 3.0f;
            hf = hf + (target - hf) * bank;
            col.river = mask;
        }
    }
    int h = (int)std::lround(hf);
    col.height = std::clamp(h, 8, world_height_ - 8);

    // Biome (height thresholds jittered too, so beach/highland edges dither)
    int above = col.height - sea_;
    const int hj = (int)(jit * 6.0f);
    if (col.c < -0.55f) col.biome = BIOME_DEEP_OCEAN;
    else if (col.river > 0.55f && col.height < sea_) col.biome = BIOME_RIVER;
    else if (col.height < sea_ - 1) col.biome = BIOME_OCEAN;
    else if (above <= 3 + (hj > 0 ? hj / 3 : 0)) col.biome = BIOME_BEACH;
    else if (above > 300 + hj) col.biome = BIOME_SNOWY_PEAKS;
    else if (above > 170 + hj) col.biome = BIOME_MOUNTAINS;
    else if (above > 70 + hj) col.biome = BIOME_HIGHLANDS;
    else if (col.t > 0.35f && col.h < -0.1f) col.biome = BIOME_DESERT;
    else if (col.t < -0.45f) col.biome = BIOME_TUNDRA;
    else if (col.h > 0.15f) col.biome = BIOME_FOREST;
    else col.biome = BIOME_PLAINS;
    return col;
}

int WorldGen::height_at(int x, int z) const { return column(x, z).height; }
int WorldGen::biome_at(int x, int z) const { return column(x, z).biome; }

Dictionary WorldGen::column_info(int x, int z) const {
    Column c = column(x, z);
    Dictionary d;
    d["height"] = c.height;
    d["biome"] = c.biome;
    d["biome_name"] = biome_name(c.biome);
    d["continentalness"] = c.c;
    d["erosion"] = c.e;
    d["peaks_valleys"] = c.pv;
    d["temperature"] = c.t;
    d["humidity"] = c.h;
    d["river"] = c.river;
    return d;
}

String WorldGen::biome_name(int b) {
    static const char *names[] = {"deep_ocean", "ocean", "beach", "plains", "forest", "desert", "tundra", "highlands", "mountains", "snowy_peaks", "river"};
    if (b < 0 || b >= BIOME_COUNT) return "unknown";
    return names[b];
}

// ---------- trees ----------

bool WorldGen::tree_at_cell(int cx, int cz, Tree &out) const {
    uint32_t hsh = hash3(cx, cz, seed_);
    int jx = (int)(hsh & 7), jz = (int)((hsh >> 3) & 7);
    int tx = cx * TREE_CELL + jx, tz = cz * TREE_CELL + jz;
    Column col = column(tx, tz);
    if (col.height <= sea_ + 2 || col.river > 0.25f) return false;
    float density;
    switch (col.biome) {
        case BIOME_FOREST: density = 0.55f; break;
        case BIOME_PLAINS: density = 0.06f; break;
        case BIOME_HIGHLANDS: density = 0.18f; break;
        case BIOME_TUNDRA: density = 0.08f; break;
        case BIOME_MOUNTAINS: density = 0.05f; break;
        default: return false;
    }
    float roll = (float)((hsh >> 6) & 1023) / 1023.0f;
    if (roll >= density) return false;
    out.x = tx;
    out.z = tz;
    out.base = col.height;
    out.trunk_h = 8 + (int)((hsh >> 16) % 6);   // 4..6.5 m trunks (0.5 m voxels)
    out.radius = 2 + (int)((hsh >> 20) & 1);   // canopy radius 2..3
    return true;
}

// ---------- lakes ----------

bool WorldGen::lake_at_cell(int cx, int cz, Lake &out) const {
    uint32_t hsh = hash3(cx, cz, seed_ + 77);
    if ((hsh & 0xFF) > 90) return false; // ~35 % of cells
    int radius = 24 + (int)((hsh >> 8) % 33);  // 24..56 voxels (12..28 m)
    int margin = radius + 4;
    int span = LAKE_CELL - 2 * margin;
    if (span <= 0) return false;
    int x = cx * LAKE_CELL + margin + (int)((hsh >> 14) % (uint32_t)span);
    int z = cz * LAKE_CELL + margin + (int)((hsh >> 22) % (uint32_t)span);
    Column c = column(x, z);
    if (c.height < sea_ + 8 || c.river > 0.1f || c.e < 0.05f) return false; // flat land only
    if (!(c.biome == BIOME_PLAINS || c.biome == BIOME_FOREST || c.biome == BIOME_HIGHLANDS || c.biome == BIOME_TUNDRA)) return false;
    // Surface level: just below the lowest point of the rim, so water never
    // floats; give up if the rim is too uneven for a believable pond.
    int rim_min = 1 << 30, rim_max = -1;
    for (int k = 0; k < 8; k++) {
        float a = (float)k * 0.7853982f;
        int rx = x + (int)std::lround(std::cos(a) * radius), rz = z + (int)std::lround(std::sin(a) * radius);
        int rh = column(rx, rz).height;
        rim_min = std::min(rim_min, rh);
        rim_max = std::max(rim_max, rh);
    }
    if (rim_max - rim_min > 6) return false; // no cliff-walled pits
    out.x = x;
    out.z = z;
    out.radius = radius;
    out.level = std::min(rim_min, c.height) - 1;
    out.depth = 4 + (int)((hsh >> 4) & 7); // 4..11 voxels deep at the centre
    if (out.level <= sea_ + 2) return false;
    return true;
}

int WorldGen::lakes_near(int x0, int z0, int x1, int z1, Lake *out, int max_out) const {
    int n = 0;
    int cx0 = (int)std::floor((float)(x0 - LAKE_MAX_RADIUS - 8) / LAKE_CELL);
    int cx1 = (int)std::floor((float)(x1 + LAKE_MAX_RADIUS + 8) / LAKE_CELL);
    int cz0 = (int)std::floor((float)(z0 - LAKE_MAX_RADIUS - 8) / LAKE_CELL);
    int cz1 = (int)std::floor((float)(z1 + LAKE_MAX_RADIUS + 8) / LAKE_CELL);
    for (int cz = cz0; cz <= cz1 && n < max_out; cz++)
        for (int cx = cx0; cx <= cx1 && n < max_out; cx++) {
            Lake l;
            if (lake_at_cell(cx, cz, l)) out[n++] = l;
        }
    return n;
}

void WorldGen::apply_lakes(int x, int z, const Lake *lakes, int n, int &height, int &water_level, bool &lakebed) {
    for (int i = 0; i < n; i++) {
        const Lake &l = lakes[i];
        float dx = (float)(x - l.x), dz = (float)(z - l.z);
        float d = std::sqrt(dx * dx + dz * dz);
        float outer = (float)l.radius + 4.0f;
        if (d > outer) continue;
        float r = (float)l.radius;
        if (d <= r) {
            // basin: bowl profile, deepest at the centre
            float t = 1.0f - smoothstep(0.0f, 1.0f, d / r); // 1 centre .. 0 rim
            int bed = l.level - (int)std::lround((float)l.depth * (0.3f + 0.7f * t));
            if (d > r * 0.85f) {
                // inner rim: blend from bed up to the rim level
                float u = (d - r * 0.85f) / (r * 0.15f);
                bed = (int)std::lround((float)bed + u * (float)(l.level - bed));
            }
            if (bed < height) height = bed;
            if (height <= l.level) { water_level = std::max(water_level, l.level); lakebed = true; }
        } else {
            // berm: never let the ground right outside the rim sit below the water
            if (height < l.level + 1) height = l.level + 1;
        }
    }
}

// ---------- ruins ----------

bool WorldGen::ruin_at_cell(int cx, int cz, Ruin &out) const {
    uint32_t hsh = hash3(cx, cz, seed_ + 991);
    if ((hsh & 0xFF) > 30) return false; // ~12 % of cells
    int hw = 5 + (int)((hsh >> 8) % 6);   // half width 5..10
    int hd = 5 + (int)((hsh >> 12) % 6);
    int margin = RUIN_MAX_HALF + 4;
    int span = RUIN_CELL - 2 * margin;
    int x = cx * RUIN_CELL + margin + (int)((hsh >> 16) % (uint32_t)span);
    int z = cz * RUIN_CELL + margin + (int)((hsh >> 24) % (uint32_t)span);
    Column c = column(x, z);
    if (c.height < sea_ + 5 || c.river > 0.1f) return false;
    if (!(c.biome == BIOME_PLAINS || c.biome == BIOME_FOREST || c.biome == BIOME_HIGHLANDS || c.biome == BIOME_TUNDRA || c.biome == BIOME_DESERT)) return false;
    // Only on flat ground: the footprint corners must sit within a few voxels of the centre.
    int hmin = c.height, hmax = c.height;
    const int cxs[4] = {x - hw, x + hw, x - hw, x + hw};
    const int czs[4] = {z - hd, z - hd, z + hd, z + hd};
    for (int k = 0; k < 4; k++) {
        int hh = column(cxs[k], czs[k]).height;
        hmin = std::min(hmin, hh);
        hmax = std::max(hmax, hh);
    }
    if (hmax - hmin > 4) return false;
    out.x = x;
    out.z = z;
    out.hw = hw;
    out.hd = hd;
    out.base = (hmin + hmax) / 2;
    out.seed = hsh;
    return true;
}

int WorldGen::ruins_near(int x0, int z0, int x1, int z1, Ruin *out, int max_out) const {
    int n = 0;
    int cx0 = (int)std::floor((float)(x0 - RUIN_MAX_HALF - 4) / RUIN_CELL);
    int cx1 = (int)std::floor((float)(x1 + RUIN_MAX_HALF + 4) / RUIN_CELL);
    int cz0 = (int)std::floor((float)(z0 - RUIN_MAX_HALF - 4) / RUIN_CELL);
    int cz1 = (int)std::floor((float)(z1 + RUIN_MAX_HALF + 4) / RUIN_CELL);
    for (int cz = cz0; cz <= cz1 && n < max_out; cz++)
        for (int cx = cx0; cx <= cx1 && n < max_out; cx++) {
            Ruin r;
            if (ruin_at_cell(cx, cz, r)) out[n++] = r;
        }
    return n;
}

bool WorldGen::in_ruin(const Ruin &r, int x, int z, int margin) {
    return std::abs(x - r.x) <= r.hw + margin && std::abs(z - r.z) <= r.hd + margin;
}

// Trees whose canopy can touch the horizontal range [x0,x1] x [z0,z1].
int WorldGen::trees_near(int x0, int z0, int x1, int z1, Tree *out, int max_out) const {
    int n = 0;
    int cx0 = (int)std::floor((float)(x0 - TREE_MAX_RADIUS) / TREE_CELL);
    int cx1 = (int)std::floor((float)(x1 + TREE_MAX_RADIUS) / TREE_CELL);
    int cz0 = (int)std::floor((float)(z0 - TREE_MAX_RADIUS) / TREE_CELL);
    int cz1 = (int)std::floor((float)(z1 + TREE_MAX_RADIUS) / TREE_CELL);
    for (int cz = cz0; cz <= cz1 && n < max_out; cz++)
        for (int cx = cx0; cx <= cx1 && n < max_out; cx++) {
            Tree t;
            if (tree_at_cell(cx, cz, t)) out[n++] = t;
        }
    return n;
}

inline bool WorldGen::is_cave(int x, int y, int z, int height) const {
    if (y < 12 || y > height - 5) return false;
    const float fx = (float)x, fy = (float)y, fz = (float)z;
    // Cheese caves: big rooms, more common deeper down.
    float depth = (float)(height - y);
    float cheese_threshold = 0.62f - 0.12f * smoothstep(20.0f, 200.0f, depth);
    if (n_cheese_.GetNoise(fx, fy * 1.6f, fz) > cheese_threshold) return true;
    // Spaghetti caves: tunnels where two noises are both near zero.
    float s1 = n_spag1_.GetNoise(fx, fy * 1.3f, fz);
    float s2 = n_spag2_.GetNoise(fx, fy * 1.3f, fz);
    return (s1 * s1 + s2 * s2) < 0.0115f;
}

inline uint16_t WorldGen::ore_at(int x, int y, int z) const {
    float o = n_ore_.GetNoise((float)x, (float)y, (float)z);
    if (o < 0.66f) return STONE;
    if (y > sea_ - 160 && o < 0.74f) return COAL_ORE;
    if (y < sea_ + 60 && o >= 0.74f && o < 0.80f) return IRON_ORE;
    if (y < sea_ - 20 && o >= 0.80f && o < 0.85f) return COPPER_ORE;
    if (y < sea_ - 80 && o >= 0.85f) return TIN_ORE;
    return STONE;
}

PackedByteArray WorldGen::generate_block(Vector3i origin, Vector3i size, int lod, int bytes_per_voxel) const {
    PackedByteArray out;
    const int step = 1 << lod;
    const int sx = size.x, sy = size.y, sz = size.z;
    if (sx <= 0 || sy <= 0 || sz <= 0) return out;
    // Fast reject: region entirely above the world.
    if (origin.y > world_height_) return out;
    const int n = sx * sy * sz;
    std::vector<uint16_t> tmp(n, 0);
    uint16_t *w = tmp.data();
    bool any = false;
    const bool do_caves = lod == 0;
    const bool do_ores = lod == 0;
    // Decorations are deterministic per grid cell, so every block stamps the
    // same trees / lakes / ruins without neighbour data. Lakes and ruins at all
    // LODs (they change the height field); trees at LOD 0..2.
    const int y_top = origin.y + sy * step;
    const bool above_sea = y_top > sea_ && origin.y < world_height_;
    Tree trees[64];
    Lake lakes[8];
    Ruin ruins[4];
    int ntrees = 0, nlakes = 0, nruins = 0;
    if (above_sea) {
        nlakes = lakes_near(origin.x, origin.z, origin.x + sx * step, origin.z + sz * step, lakes, 8);
        nruins = ruins_near(origin.x, origin.z, origin.x + sx * step, origin.z + sz * step, ruins, 4);
        if (lod <= 2) ntrees = trees_near(origin.x, origin.z, origin.x + sx * step, origin.z + sz * step, trees, 64);
    }
    for (int z = 0; z < sz; z++) {
        const int wz = origin.z + z * step;
        for (int x = 0; x < sx; x++) {
            const int wx = origin.x + x * step;
            const Column col = column(wx, wz);
            int h = col.height;
            int water_level = sea_;
            bool lakebed = false;
            int no_cave_above = -1; // no caves above this y (keeps lakes from draining)
            if (nlakes) {
                apply_lakes(wx, wz, lakes, nlakes, h, water_level, lakebed);
                for (int i = 0; i < nlakes; i++) {
                    float ddx = (float)(wx - lakes[i].x), ddz = (float)(wz - lakes[i].z);
                    float lim = (float)(lakes[i].radius + 8);
                    if (ddx * ddx + ddz * ddz <= lim * lim) no_cave_above = std::max(no_cave_above, lakes[i].level - 40);
                }
            }
            // Ruin footprint: flatten to the ruin base and open the air above it.
            const Ruin *ruin = nullptr;
            for (int i = 0; i < nruins; i++)
                if (in_ruin(ruins[i], wx, wz, 2)) { ruin = &ruins[i]; break; }
            if (ruin) {
                const int ground = ruin->base;
                if (in_ruin(*ruin, wx, wz, 0)) h = ground; else if (h > ground + 1) h = ground + 1;
            }
            const bool beach_sand = col.biome == BIOME_BEACH || (col.biome == BIOME_OCEAN && col.c > -0.4f);
            const bool river_bank = col.river > 0.08f && h <= sea_ + 2;
            uint16_t *colp = w + sy * (x + sx * z); // y is fastest (VoxelBuffer order)
            // Trees touching this column: trunk (dx=dz=0) and canopy layers.
            int trunk_lo = 0, trunk_hi = -1;
            int can_lo = 0, can_hi = -1, can_base = 0, can_r = 0, can_d2 = 0;
            for (int t = 0; t < ntrees; t++) {
                const int dx = wx - trees[t].x, dz = wz - trees[t].z;
                const int d2 = dx * dx + dz * dz;
                if (d2 > trees[t].radius * trees[t].radius + 1) continue;
                // no trees in lakes or on ruins
                bool blocked = false;
                for (int i = 0; i < nlakes && !blocked; i++) {
                    float ddx = (float)(trees[t].x - lakes[i].x), ddz = (float)(trees[t].z - lakes[i].z);
                    blocked = ddx * ddx + ddz * ddz <= (float)((lakes[i].radius + 2) * (lakes[i].radius + 2));
                }
                for (int i = 0; i < nruins && !blocked; i++) blocked = in_ruin(ruins[i], trees[t].x, trees[t].z, 3);
                if (blocked) continue;
                const int top = trees[t].base + trees[t].trunk_h;
                if (dx == 0 && dz == 0) { trunk_lo = trees[t].base; trunk_hi = top + 1; }
                if (can_hi < 0 || d2 < can_d2) {
                    can_base = top - 2; can_lo = can_base; can_hi = top + 2; can_r = trees[t].radius; can_d2 = d2;
                }
            }
            // Ruin walls for this column: perimeter segments with gaps, taller corner
            // pillars, rubble just outside.
            int wall_top = -1;
            uint32_t wall_seed = 0;
            bool rubble = false;
            if (ruin) {
                const int ax = std::abs(wx - ruin->x), az = std::abs(wz - ruin->z);
                const bool on_perimeter = (ax == ruin->hw && az <= ruin->hd) || (az == ruin->hd && ax <= ruin->hw);
                const bool corner = ax == ruin->hw && az == ruin->hd;
                if (on_perimeter) {
                    const int seg = corner ? 99 : ((ax == ruin->hw) ? (wz - ruin->z + ruin->hd) / 4 + 100 * (wx > ruin->x) : (wx - ruin->x + ruin->hw) / 4 + 200 * (wz > ruin->z));
                    uint32_t sh = hash3((int)ruin->seed, seg, 7);
                    int hgt = corner ? 6 + (int)(sh % 4) : (int)(sh % 7); // 0..6, corners 6..9
                    if (!corner && ((sh >> 8) & 7) == 0) hgt = 0;            // collapsed segment
                    wall_top = ruin->base + hgt;
                    wall_seed = hash3(wx, wz, (int)ruin->seed);
                } else if (!in_ruin(*ruin, wx, wz, 0)) {
                    rubble = (hash3(wx, wz, (int)ruin->seed + 5) & 15) == 0;
                }
            }
            for (int y = 0; y < sy; y++) {
                const int wy = origin.y + y * step;
                uint16_t m = AIR;
                if (ruin && wy >= ruin->base && wy <= ruin->base + 10 && in_ruin(*ruin, wx, wz, 2)) {
                    // inside the ruin volume: floor, walls, rubble, otherwise open air
                    if (wy == ruin->base && in_ruin(*ruin, wx, wz, 0)) m = (hash3(wx, wz, 3) & 3) ? STONE : GRAVEL_STATIC;
                    else if (wall_top >= 0 && wy > ruin->base && wy <= wall_top) {
                        // weathered: random missing stones, more missing near the top
                        uint32_t vh = hash3(wy, (int)wall_seed, 11);
                        int miss = (vh & 15) < (uint32_t)(2 + (wy - ruin->base)) ? 1 : 0;
                        m = miss ? AIR : STONE;
                    } else if (rubble && wy == ruin->base + 1) m = STONE;
                    colp[y] = m;
                    if (m != AIR) any = true;
                    continue;
                }
                if (wy >= trunk_lo && wy <= trunk_hi) {
                    m = WOOD;
                } else if (can_hi >= 0 && wy >= can_lo && wy <= can_hi && wy >= h) {
                    // canopy: layer radius shrinks toward top and bottom
                    const int k = wy - can_base;              // 0..4
                    const int lr = (k == 0 || k == 3) ? can_r - 1 : (k == 4 ? can_r - 2 : can_r);
                    if (lr >= 0 && can_d2 <= lr * lr + (lr > 1 ? 1 : 0)) m = LEAVES;
                }
                if (m != AIR) { colp[y] = m; any = true; continue; }
                if (wy < 3) {
                    m = BEDROCK;
                } else if (wy < h) {
                    const int depth = h - wy;
                    if (do_caves && wy < no_cave_above - 0 + (no_cave_above < 0 ? 1 << 20 : 0) && is_cave(wx, wy, wz, h) && !(wy < sea_ && h < sea_ + 2)) {
                        m = (wy < sea_ - 180) ? LAVA : AIR; // deep caves hold lava pools
                    } else if (depth > 4 + (int)(step > 1)) {
                        m = do_ores ? ore_at(wx, wy, wz) : STONE;
                    } else {
                        // top layers by biome
                        const bool top = depth == 1;
                        if (lakebed) {
                            m = (hash3(wx, wz, 5) & 3) ? GRAVEL_STATIC : SAND_STATIC;
                        } else if (river_bank) {
                            m = (col.river > 0.5f) ? GRAVEL_STATIC : SAND_STATIC;
                        } else switch (col.biome) {
                            case BIOME_RIVER: m = GRAVEL_STATIC; break;
                            case BIOME_DEEP_OCEAN: m = GRAVEL_STATIC; break;
                            case BIOME_OCEAN: m = beach_sand ? SAND_STATIC : GRAVEL_STATIC; break;
                            case BIOME_BEACH: m = SAND_STATIC; break;
                            case BIOME_DESERT: m = SAND_STATIC; break;
                            case BIOME_TUNDRA: m = top ? SNOW : DIRT; break;
                            case BIOME_SNOWY_PEAKS: m = top ? SNOW : STONE; break;
                            case BIOME_MOUNTAINS: m = (top && col.pv > 0.2f) ? STONE : (top ? GRASS : DIRT); break;
                            default: m = top ? GRASS : DIRT; break;
                        }
                    }
                } else if (wy < water_level) {
                    m = WATER;
                }
                colp[y] = m;
                if (m != AIR) any = true;
            }
        }
    }
    if (!any) return out;
    // Pack to the buffer's channel depth (1, 2 or 4 bytes per voxel, little-endian).
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

Vector3i WorldGen::find_nearest(const String &kind, int x, int z, int max_radius) const {
    const bool want_lake = kind == "lake";
    const int cell = want_lake ? LAKE_CELL : RUIN_CELL;
    const int cx0 = (int)std::floor((float)x / cell), cz0 = (int)std::floor((float)z / cell);
    const int rings = max_radius / cell + 1;
    Vector3i best(0, -1, 0);
    long best_d2 = -1;
    for (int ring = 0; ring <= rings; ring++) {
        for (int dz = -ring; dz <= ring; dz++)
            for (int dx = -ring; dx <= ring; dx++) {
                if (std::max(std::abs(dx), std::abs(dz)) != ring) continue;
                int px, pz, py;
                if (want_lake) {
                    Lake l;
                    if (!lake_at_cell(cx0 + dx, cz0 + dz, l)) continue;
                    px = l.x; pz = l.z; py = l.level;
                } else {
                    Ruin r;
                    if (!ruin_at_cell(cx0 + dx, cz0 + dz, r)) continue;
                    px = r.x; pz = r.z; py = r.base;
                }
                long d2 = (long)(px - x) * (px - x) + (long)(pz - z) * (pz - z);
                if (best_d2 < 0 || d2 < best_d2) { best_d2 = d2; best = Vector3i(px, py, pz); }
            }
        if (best_d2 >= 0) break; // nearest ring with a hit is good enough
    }
    return best;
}

PackedByteArray WorldGen::map_image(int center_x, int center_z, int radius, int step) const {
    const int n = std::max(1, 2 * radius / std::max(1, step));
    PackedByteArray img;
    img.resize(n * n * 3);
    uint8_t *p = img.ptrw();
    std::vector<Lake> lakes(1024);
    std::vector<Ruin> ruins(1024);
    const int nl = lakes_near(center_x - radius, center_z - radius, center_x + radius, center_z + radius, lakes.data(), 1024);
    const int nr = ruins_near(center_x - radius, center_z - radius, center_x + radius, center_z + radius, ruins.data(), 1024);
    static const uint8_t colors[BIOME_COUNT][3] = {
        {20, 40, 120}, {40, 80, 200}, {230, 215, 150}, {110, 190, 80}, {40, 130, 50},
        {225, 200, 110}, {200, 210, 220}, {130, 150, 90}, {120, 120, 125}, {245, 245, 250},
        {70, 140, 230},
    };
    for (int j = 0; j < n; j++)
        for (int i = 0; i < n; i++) {
            Column c = column(center_x - radius + i * step, center_z - radius + j * step);
            float shade = 0.6f + 0.4f * std::clamp((float)(c.height - sea_) / 400.0f, -0.5f, 1.0f);
            if (c.height < sea_) shade = 0.5f + 0.5f * std::clamp((float)(c.height - sea_ + 80) / 80.0f, 0.0f, 1.0f);
            const uint8_t *col = colors[c.biome];
            uint8_t *q = p + (j * n + i) * 3;
            q[0] = (uint8_t)std::clamp(col[0] * shade, 0.0f, 255.0f);
            q[1] = (uint8_t)std::clamp(col[1] * shade, 0.0f, 255.0f);
            q[2] = (uint8_t)std::clamp(col[2] * shade, 0.0f, 255.0f);
            const int wx = center_x - radius + i * step, wz = center_z - radius + j * step;
            for (int k = 0; k < nl; k++) {
                long dx = wx - lakes[k].x, dz = wz - lakes[k].z;
                if (dx * dx + dz * dz <= (long)lakes[k].radius * lakes[k].radius) { q[0] = 120; q[1] = 190; q[2] = 255; break; }
            }
            for (int k = 0; k < nr; k++)
                if (in_ruin(ruins[k], wx, wz, 1)) { q[0] = 40; q[1] = 40; q[2] = 45; break; }
        }
    return img;
}
