#include "world_gen.h"

#include <godot_cpp/core/class_db.hpp>

#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstring>
#include <queue>

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
    ClassDB::bind_method(D_METHOD("climate_map", "x0", "z0", "n", "step"), &WorldGen::climate_map);
    ClassDB::bind_method(D_METHOD("water_floor_map", "x0", "z0", "n", "step"), &WorldGen::water_floor_map);
    ClassDB::bind_static_method("WorldGen", D_METHOD("biome_name", "biome"), &WorldGen::biome_name);
    ClassDB::bind_method(D_METHOD("find_nearest", "kind", "x", "z", "max_radius"), &WorldGen::find_nearest);
    ClassDB::bind_method(D_METHOD("waterfalls_near", "x", "z", "radius"), &WorldGen::waterfalls_near);
    ClassDB::bind_method(D_METHOD("hydrology_stats", "x", "z"), &WorldGen::hydrology_stats);
    ClassDB::bind_method(D_METHOD("vein_at", "x", "z"), &WorldGen::vein_at);
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
    BIND_ENUM_CONSTANT(BIOME_LAKE);
    BIND_ENUM_CONSTANT(BIOME_ALPINE);
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
    setup(n_cont_, seed_ + 1, 0.00033f, 3);
    setup(n_eros_, seed_ + 2, 0.0008f, 3);
    setup(n_pv_, seed_ + 3, 0.0025f, 3, FastNoiseLite::FractalType_Ridged);
    setup(n_detail_, seed_ + 4, 0.013f, 2); // ~75 voxel period: fewer, longer steps on flats
    setup(n_temp_, seed_ + 5, 0.0004f, 2);
    setup(n_hum_, seed_ + 6, 0.0005f, 2);
    setup(n_cheese_, seed_ + 7, 0.011f, 2);
    setup(n_cavebiome_, seed_ + 13, 1.0f / (technosim_biome::CAVE_NOISE_PERIOD_M * 2.0f), 1); // 3D, slow: deep dark / crystal / magma pockets
    setup(n_spag1_, seed_ + 8, 0.018f, 1);
    setup(n_spag2_, seed_ + 9, 0.018f, 1);
    setup(n_ore_, seed_ + 10, 0.09f, 1); // ore vein lens edge wobble
    setup(n_beach_, seed_ + 11, 0.01f, 1);
    setup(n_jitter_, seed_ + 13, 0.02f, 2);
    setup(n_pool_, seed_ + 17, 0.045f, 1); // ~11 m blobs: where cave floors hold their biome's pool
    std::lock_guard<std::mutex> lk(hydro_mutex_);
    hydro_cache_.clear();
}

static inline int floor_div(int a, int b) { return a >= 0 ? a / b : -((-a + b - 1) / b); }

uint32_t WorldGen::hash3(int a, int b, int c) {
    uint32_t h = (uint32_t)a * 374761393u + (uint32_t)b * 668265263u + (uint32_t)c * 2246822519u;
    h ^= h >> 13;
    h *= 1274126177u;
    h ^= h >> 16;
    return h;
}

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

// ---------- terrain ----------

WorldGen::Column WorldGen::column_base(int x, int z, bool with_slope) const {
    const float fx = (float)x, fz = (float)z;
    Column col;
    col.c = n_cont_.GetNoise(fx, fz);
    col.e = n_eros_.GetNoise(fx, fz);
    col.pv = n_pv_.GetNoise(fx, fz);
    const float jit = n_jitter_.GetNoise(fx, fz);
    const float jit2 = n_jitter_.GetNoise(fz + 1000.0f, fx - 1000.0f);
    const float t_noise = n_temp_.GetNoise(fx, fz) + jit * technosim_biome::JITTER;
    col.t_noise = t_noise;
    col.h = n_hum_.GetNoise(fx, fz) + jit2 * technosim_biome::JITTER;

    // Sea floor (2026-10-23): a shallow shelf off the beach (-1.5 .. -18 voxels), a continental
    // slope down to the deep-ocean line (c -0.55, ~-95), then an abyssal plain that keeps sinking
    // to 100 m below the surface: deep enough to go dark and room for boats and sea life.
    static const float cx[] = {-1.0f, -0.75f, -0.6f, -0.45f, -0.3f, -0.15f, -0.05f, 0.05f, 0.3f, 0.55f, 0.8f, 1.0f};
    static const float cy[] = {-200.0f, -160.0f, -120.0f, -55.0f, -18.0f, -7.0f, -1.5f, 4.0f, 24.0f, 80.0f, 180.0f, 240.0f};
    float base = spline(col.c, cx, cy, 12);
    static const float ex[] = {-1.0f, -0.3f, 0.4f, 1.0f};
    static const float ey[] = {1.25f, 1.0f, 0.45f, 0.15f};
    float relief_scale = spline(col.e, ex, ey, 4);
    // Lowlands roll gently (walkable with a 1-voxel step); mountains keep their
    // ridges and fine roughness so they stay hard to traverse.
    const float mountainness = smoothstep(0.25f, 0.85f, col.c);
    float mountain_amp = 18.0f + 272.0f * mountainness;
    float relief = (col.pv * 0.5f + 0.5f) * mountain_amp * relief_scale;
    if (base < 0.0f) relief *= 0.25f;
    float detail = n_detail_.GetNoise(fx, fz) * (1.2f + 3.5f * mountainness);
    col.hf = std::clamp((float)sea_ + base + relief + detail, 8.0f, (float)(world_height_ - 8));
    int h = (int)std::lround((float)sea_ + base + relief + detail);
    col.height = std::clamp(h, 8, world_height_ - 8);
    col.slope = 0.0f;
    col.alpine = false;
    col.snow = false;

    const int above = col.height - sea_;
    const int hj = (int)(jit * 6.0f);
    // Climate: noise + latitude (north = -z colder, 16 km to saturate) + lapse
    // above 70 voxels (biomes.md). Altitude lines from the same tables, jittered
    // so the tree line and snow line wander instead of following a contour.
    col.t = technosim_biome::temperature(t_noise, fz, above);
    const int tree_line = technosim_biome::tree_line(col.t) + hj * 2;
    const int snow_line = technosim_biome::snow_line(col.t) + hj * 2;
    col.alpine = above > tree_line;
    col.snow = above > snow_line;
    if (with_slope && (above > 120 || (above >= -1 && above <= technosim_biome::COAST_MAX + 2))) {
        // Slope matters on mountains (rock faces, scree) and at the coast (stony
        // shores); two extra samples keep the cost local.
        Column cxp = column_base(x + 2, z, false);
        Column czp = column_base(x, z + 2, false);
        col.slope = std::max(std::fabs((float)(cxp.height - col.height)), std::fabs((float)(czp.height - col.height))) * 0.5f;
    }
    // Band thresholds see the jittered altitude (dithered borders); the coast
    // and the sea use the real one so no dry land is called ocean.
    const int above_j = above <= technosim_biome::COAST_MAX ? above : std::max(technosim_biome::COAST_MAX + 1, above - hj * 2);
    col.biome = technosim_biome::classify(col.t, col.h, above_j, col.c, col.e, col.slope, 0.0f, false);
    return col;
}

WorldGen::Column WorldGen::column(int x, int z) const { return column_base(x, z, true); }

// n x n columns from (x0, z0) every `step` voxels: where open, unfrozen sea or lake
// water stands over the ground, floor | water_level << 16 (the first water voxel and
// the first air voxel above it), else -1. For kelp and fish (OceanLife, ADR 0027).
PackedInt32Array WorldGen::water_floor_map(int x0, int z0, int n, int step) const {
    PackedInt32Array out;
    if (n <= 0 || step <= 0) return out;
    out.resize(n * n);
    int32_t *o = out.ptrw();
    for (int j = 0; j < n; j++)
        for (int i = 0; i < n; i++) {
            const int x = x0 + i * step, z = z0 + j * step;
            const Column c = column_base(x, z, false);
            int floor = c.height, level = sea_;
            const Hydro hy = hydro_at(x, z, c.height);
            if (hy.lake) level = std::max(level, hy.water_level);
            if (hy.river > 0.0f) { floor = std::min(floor, hy.bed); level = std::max(level, hy.water_level); }
            const bool frozen = technosim_biome::SURFACE_ICE[c.biome] >= 0 || ((hy.lake || hy.marsh) && (c.t < -0.45f || c.snow));
            o[j * n + i] = (floor < level && !frozen && !hy.marsh) ? (floor | (level << 16)) : -1;
        }
    return out;
}

// n x n final temperatures (lapse included) from (x0, z0) every `step` voxels,
// as bytes (t + 1) * 127.5, row-major in z: the terrain shader's season map
// (snow cover where it is cold enough, ADR 0025). Thread-safe like generate_block.
PackedByteArray WorldGen::climate_map(int x0, int z0, int n, int step) const {
    PackedByteArray out;
    if (n <= 0 || step <= 0) return out;
    out.resize(n * n);
    uint8_t *o = out.ptrw();
    for (int j = 0; j < n; j++)
        for (int i = 0; i < n; i++) {
            const Column c = column_base(x0 + i * step, z0 + j * step, false);
            o[j * n + i] = (uint8_t)std::clamp((int)std::lround((c.t + 1.0f) * 127.5f), 0, 255);
        }
    return out;
}

int WorldGen::height_at(int x, int z) const {
    Column c = column_base(x, z, false);
    Hydro hy = hydro_at(x, z, c.height);
    if (hy.river > 0.0f) return std::min(c.height, hy.bed);
    if (hy.marsh) return marsh_floor(c.height, hy);
    return c.height;
}

// Marsh floor: the hollow is filled flat to marsh_level (wet ground); puddle
// columns are cut 1-2 voxels below it and hold static water.
int WorldGen::marsh_floor(int h, const Hydro &hy) {
    if (hy.puddle) return std::clamp(h, hy.marsh_level - 2, hy.marsh_level - 1);
    return std::max(h, hy.marsh_level);
}

int WorldGen::biome_at(int x, int z) const {
    Column c = column_base(x, z, false);
    Hydro hy = hydro_at(x, z, c.height);
    if (hy.lake && c.height < hy.water_level) return BIOME_LAKE;
    if (hy.river > 0.5f) return BIOME_RIVER;
    return c.biome;
}

Dictionary WorldGen::column_info(int x, int z) const {
    Column c = column(x, z);
    Hydro hy = hydro_at(x, z, c.height);
    Dictionary d;
    d["height"] = c.height;
    d["biome"] = biome_at(x, z);
    d["biome_name"] = biome_name((int)d["biome"]);
    d["continentalness"] = c.c;
    d["erosion"] = c.e;
    d["peaks_valleys"] = c.pv;
    d["temperature"] = c.t;
    d["temperature_noise"] = c.t_noise;
    d["humidity"] = c.h;
    d["slope"] = c.slope;
    d["alpine"] = c.alpine;
    d["snow"] = c.snow;
    d["river"] = hy.river;
    d["water_level"] = hy.water_level;
    d["river_bed"] = hy.river > 0.0f ? hy.bed : -1;
    d["lake"] = hy.lake;
    d["spring"] = hy.spring;
    d["marsh"] = hy.marsh;
    d["puddle"] = hy.puddle;
    d["shore"] = hy.shore;
    d["fall_top"] = hy.fall_top;
    const Dictionary vein = vein_at(x, z);
    d["vein"] = vein.has("name") ? vein["name"] : Variant(String());
    d["vein_y"] = vein.has("y") ? vein["y"] : Variant(-1);
    d["in_vein_column"] = vein.has("half_thickness") && (float)vein["half_thickness"] > 0.0f;
    return d;
}

String WorldGen::biome_name(int b) {
    if (b < 0 || b >= BIOME_COUNT) return "unknown";
    return technosim_biome::NAMES[b];
}

// ---------- hydrology ----------

static const int D8X[8] = {1, 1, 0, -1, -1, -1, 0, 1};
static const int D8Z[8] = {0, 1, 1, 1, 0, -1, -1, -1};

std::shared_ptr<WorldGen::HydroTile> WorldGen::hydro_tile(int tx, int tz) const {
    const uint64_t key = ((uint64_t)(uint32_t)tx << 32) | (uint32_t)tz;
    std::promise<std::shared_ptr<HydroTile>> mine;
    std::shared_future<std::shared_ptr<HydroTile>> f;
    bool builder = false;
    {
        std::lock_guard<std::mutex> lk(hydro_mutex_);
        auto it = hydro_cache_.find(key);
        if (it != hydro_cache_.end()) {
            it->second.used = ++hydro_tick_;
            f = it->second.tile;
        } else {
            if (hydro_cache_.size() >= HYDRO_CACHE_MAX) {
                // evict the least recently used quarter among finished tiles
                std::vector<std::pair<uint64_t, uint64_t>> order; // (used, key)
                for (auto &kv : hydro_cache_)
                    if (kv.second.tile.wait_for(std::chrono::seconds(0)) == std::future_status::ready)
                        order.push_back({kv.second.used, kv.first});
                std::sort(order.begin(), order.end());
                for (size_t i = 0; i < order.size() / 4; i++) hydro_cache_.erase(order[i].second);
            }
            f = mine.get_future().share();
            hydro_cache_[key] = HydroEntry{f, ++hydro_tick_};
            builder = true;
        }
    }
    if (!builder) return f.get();   // built by another thread, or being built: wait for it
    auto t = std::make_shared<HydroTile>();
    t->tx = tx;
    t->tz = tz;
    build_hydro_tile(*t);
    mine.set_value(t);
    return t;
}

void WorldGen::build_hydro_tile(HydroTile &t) const {
    const int n = HWIN * HWIN;
    t.h.resize(n);
    t.filled.resize(n);
    t.acc.assign(n, 1.0f);
    t.dir.assign(n, -1);
    t.lake.assign(n, 0);
    t.jx.resize(n);
    t.jz.resize(n);
    t.fall_t0.assign(n, -1.0f);
    t.fall_t1.assign(n, -1.0f);
    t.fall_top.assign(n, -1);
    const int wx0 = (t.tx * HTILE - HMARGIN) * HCELL;
    const int wz0 = (t.tz * HTILE - HMARGIN) * HCELL;
    for (int j = 0; j < HWIN; j++)
        for (int i = 0; i < HWIN; i++) {
            const int wx = wx0 + i * HCELL + HCELL / 2, wz = wz0 + j * HCELL + HCELL / 2;
            Column c = column_base(wx, wz, false);
            t.h[j * HWIN + i] = (float)c.height;
            // jittered centre so rivers are not grid-aligned
            t.jx[j * HWIN + i] = n_detail_.GetNoise((float)wx * 0.5f, (float)wz * 0.5f) * 10.0f;
            t.jz[j * HWIN + i] = n_detail_.GetNoise((float)wz * 0.5f + 777.0f, (float)wx * 0.5f) * 10.0f;
        }
    // Priority-flood with epsilon (Barnes 2014): outlets are the window border
    // and every ocean cell; the result is a surface that descends to an outlet
    // from everywhere, with depressions filled to their spill level.
    const float EPS = 0.02f;
    std::vector<uint8_t> done(n, 0);
    using Item = std::pair<float, int>;
    std::priority_queue<Item, std::vector<Item>, std::greater<Item>> pq;
    for (int k = 0; k < n; k++) {
        const int i = k % HWIN, j = k / HWIN;
        const bool border = i == 0 || j == 0 || i == HWIN - 1 || j == HWIN - 1;
        if (border || t.h[k] < (float)sea_) {
            t.filled[k] = t.h[k];
            done[k] = 1;
            pq.push({t.h[k], k});
        }
    }
    while (!pq.empty()) {
        Item it = pq.top();
        pq.pop();
        const int k = it.second;
        const int i = k % HWIN, j = k / HWIN;
        for (int d = 0; d < 8; d++) {
            const int ni = i + D8X[d], nj = j + D8Z[d];
            if (ni < 0 || nj < 0 || ni >= HWIN || nj >= HWIN) continue;
            const int nk = nj * HWIN + ni;
            if (done[nk]) continue;
            done[nk] = 1;
            t.filled[nk] = std::max(t.h[nk], t.filled[k] + EPS);
            pq.push({t.filled[nk], nk});
        }
    }
    // D8 flow direction on the filled surface (steepest descent), lakes where filled.
    for (int k = 0; k < n; k++) {
        const int i = k % HWIN, j = k / HWIN;
        t.lake[k] = (t.filled[k] - t.h[k] > LAKE_MIN_DEPTH && t.h[k] >= (float)sea_) ? 1 : 0; // shallower hollows: marsh or dry
        if (t.h[k] < (float)sea_) { t.dir[k] = -1; continue; }
        float best = 0.0f;
        int bd = -1;
        for (int d = 0; d < 8; d++) {
            const int ni = i + D8X[d], nj = j + D8Z[d];
            if (ni < 0 || nj < 0 || ni >= HWIN || nj >= HWIN) continue;
            const float drop = (t.filled[k] - t.filled[nj * HWIN + ni]) / ((d & 1) ? 1.4142f : 1.0f);
            if (drop > best) { best = drop; bd = d; }
        }
        t.dir[k] = (int8_t)bd;
    }
    // Flow accumulation: visit cells from high to low filled, pass flow downstream.
    std::vector<int> order(n);
    for (int k = 0; k < n; k++) order[k] = k;
    std::sort(order.begin(), order.end(), [&](int a, int b) { return t.filled[a] > t.filled[b]; });
    for (int k : order) {
        if (t.dir[k] < 0) continue;
        const int i = k % HWIN, j = k / HWIN;
        const int nk = (j + D8Z[t.dir[k]]) * HWIN + (i + D8X[t.dir[k]]);
        t.acc[nk] += t.acc[k];
    }
    // Waterfalls: for stream segments that drop >= FALL_MIN between cells, sample
    // the real ground along the jittered segment and hang the sheet over the
    // steepest 2/12 of it, if that part alone is a cliff (>= 6 voxels).
    for (int k = 0; k < n; k++) {
        if (t.dir[k] < 0 || t.lake[k] || t.acc[k] < STREAM_ACC) continue;
        const int i = k % HWIN, j = k / HWIN;
        const int nk = (j + D8Z[t.dir[k]]) * HWIN + (i + D8X[t.dir[k]]);
        if (t.filled[k] - t.filled[nk] < FALL_MIN) continue;
        const float ax = (float)(wx0 + i * HCELL + HCELL / 2) + t.jx[k];
        const float az = (float)(wz0 + j * HCELL + HCELL / 2) + t.jz[k];
        const float bx = (float)(wx0 + (nk % HWIN) * HCELL + HCELL / 2) + t.jx[nk];
        const float bz = (float)(wz0 + (nk / HWIN) * HCELL + HCELL / 2) + t.jz[nk];
        const int S = 12;
        int g[S + 1];
        for (int q = 0; q <= S; q++) {
            const float f = (float)q / S;
            g[q] = column_base((int)std::lround(ax + (bx - ax) * f), (int)std::lround(az + (bz - az) * f), false).height;
        }
        // Near-vertical only: >= 6 voxels down over one sample step (~2.7 voxels),
        // i.e. a rock face, not a stair of 1-voxel steps. The sheet is one step thin.
        int best = 0, bq = -1;
        for (int q = 0; q + 1 <= S; q++) {
            const int d = g[q] - g[q + 1];
            if (d > best) { best = d; bq = q; }
        }
        if (bq < 0 || best < 6) continue;
        t.fall_t0[k] = (float)bq / S + 0.25f / S;
        t.fall_t1[k] = (float)(bq + 1) / S;
        t.fall_top[k] = (int16_t)(g[bq] - 1);
    }
}
static inline int floordiv(int a, int b) { return (int)std::floor((float)a / (float)b); }

const WorldGen::HydroTile *WorldGen::tile_cached(int tx, int tz) const {
    // Each generator thread touches at most a few tiles at a time (its own and
    // the neighbours of the seam band); avoid the global mutex for those.
    // LRU over 4 slots: one hydro_at() needs up to 3 tiles (owner + 2 seam
    // neighbours) and holds raw pointers to them, so eviction must never hit a
    // tile fetched in the same call (round-robin did, after ~64 tiles: crash).
    struct Slot { int tx = 0, tz = 0; uint64_t used = 0; std::shared_ptr<HydroTile> t; };
    thread_local Slot slots[4];
    thread_local uint64_t tick = 0;
    Slot *victim = &slots[0];
    for (auto &s : slots) {
        if (s.t && s.tx == tx && s.tz == tz) { s.used = ++tick; return s.t.get(); }
        if (s.used < victim->used) victim = &s;
    }
    victim->t = hydro_tile(tx, tz);
    victim->tx = tx;
    victim->tz = tz;
    victim->used = ++tick;
    return victim->t.get();
}

bool WorldGen::hydro_ctx(int x, int z, HydroCtx &ctx) const {
    const int cx = floordiv(x, HCELL), cz = floordiv(z, HCELL);
    const int tx = floordiv(cx, HTILE), tz = floordiv(cz, HTILE);
    ctx.t = tile_cached(tx, tz);
    const int i = cx - (tx * HTILE - HMARGIN), j = cz - (tz * HTILE - HMARGIN);
    if (i < 1 || j < 1 || i >= HWIN - 1 || j >= HWIN - 1) return false;
    ctx.ci = j * HWIN + i;
    // Seam blending: within HBLEND cells of the core border the accumulation is
    // mixed with the neighbour tile value, 50/50 exactly at the border, so the
    // same column gives the same river width from both sides.
    const float fx = (float)x / HCELL - (float)(tx * HTILE);
    const float fz = (float)z / HCELL - (float)(tz * HTILE);
    ctx.bx = ctx.bz = nullptr;
    ctx.wx = ctx.wz = 0.0f;
    if (fx < HBLEND) { ctx.bx = tile_cached(tx - 1, tz); ctx.wx = 0.5f * (1.0f - fx / HBLEND); }
    else if (HTILE - fx < HBLEND) { ctx.bx = tile_cached(tx + 1, tz); ctx.wx = 0.5f * (1.0f - (HTILE - fx) / HBLEND); }
    if (fz < HBLEND) { ctx.bz = tile_cached(tx, tz - 1); ctx.wz = 0.5f * (1.0f - fz / HBLEND); }
    else if (HTILE - fz < HBLEND) { ctx.bz = tile_cached(tx, tz + 1); ctx.wz = 0.5f * (1.0f - (HTILE - fz) / HBLEND); }
    return true;
}

// River size from drainage strength sqrt(acc / STREAM_ACC) (1 at a spring .. ~10 for big rivers), in voxels. Sized
// for boats (2026-10-23): a brook is 4.5 m wide and 1.5 m deep, a strength-2 stream 7 m and
// 2 m, the largest rivers 28 m wide and 6 m deep (tools/water_probe.gd measures them).
static float river_half_width(float strength) {
    return std::clamp(2.0f + 2.5f * strength, 3.5f, 28.0f);
}

static float river_depth(float strength) {
    return std::clamp(2.0f + 1.0f * strength, 3.0f, 12.0f);
}

float WorldGen::HydroCtx::acc(int k) const {
    float a = t->acc[k];
    if (!bx && !bz) return a;
    const int cx = t->tx * HTILE - HMARGIN + k % HWIN;
    const int cz = t->tz * HTILE - HMARGIN + k / HWIN;
    auto other = [&](const HydroTile *o, float w) {
        const int oi = cx - (o->tx * HTILE - HMARGIN), oj = cz - (o->tz * HTILE - HMARGIN);
        if (oi < 0 || oj < 0 || oi >= HWIN || oj >= HWIN) return;
        a = a * (1.0f - w) + o->acc[oj * HWIN + oi] * w;
    };
    if (bx) other(bx, wx);
    if (bz) other(bz, wz);
    return a;
}

static inline float seg_dist(float px, float pz, float ax, float az, float bx, float bz, float &tout) {
    const float dx = bx - ax, dz = bz - az;
    const float len2 = dx * dx + dz * dz;
    float t = len2 > 0.0f ? ((px - ax) * dx + (pz - az) * dz) / len2 : 0.0f;
    t = std::clamp(t, 0.0f, 1.0f);
    tout = t;
    const float qx = ax + t * dx - px, qz = az + t * dz - pz;
    return std::sqrt(qx * qx + qz * qz);
}

WorldGen::Hydro WorldGen::hydro_at(int x, int z, int base_height) const {
    Hydro out{0.0f, 0, -1, false, false, false, false, -1, -1, false};
    HydroCtx c;
    if (!hydro_ctx(x, z, c)) return out;
    const HydroTile *t = c.t;
    const int ci = c.ci;
    const int i = ci % HWIN, j = ci / HWIN;
    const int wx0 = (t->tx * HTILE - HMARGIN) * HCELL;
    const int wz0 = (t->tz * HTILE - HMARGIN) * HCELL;
    auto centre_x = [&](int k) { return (float)(wx0 + (k % HWIN) * HCELL + HCELL / 2) + t->jx[k]; };
    auto centre_z = [&](int k) { return (float)(wz0 + (k / HWIN) * HCELL + HCELL / 2) + t->jz[k]; };
    auto downstream = [&](int k) { return (k / HWIN + D8Z[t->dir[k]]) * HWIN + (k % HWIN + D8X[t->dir[k]]); };
    // Lakes: the filled level of this cell or a neighbouring lake cell.
    float lake_level = -1.0f;
    for (int dj = -1; dj <= 1; dj++)
        for (int di = -1; di <= 1; di++) {
            const int k = (j + dj) * HWIN + (i + di);
            if (t->lake[k]) lake_level = std::max(lake_level, t->filled[k]);
        }
    if (lake_level > 0.0f && (float)base_height >= lake_level - 0.5f && (float)base_height <= lake_level + 2.5f) out.shore = true;
    if (lake_level > 0.0f && (float)base_height < lake_level - 0.5f) {
        out.lake = true;
        out.water_level = (int)std::floor(lake_level);
    }
    // Marsh: a humid, warm lowland hollow too shallow for a lake. Only the
    // lowest columns flood (puddles); the rest is wet ground.
    const float dd = t->filled[ci] - t->h[ci];
    if (!out.lake && !t->lake[ci] && t->h[ci] >= (float)sea_ && dd > MARSH_MIN_DEPTH && dd <= LAKE_MIN_DEPTH && t->h[ci] - (float)sea_ < 48.0f) {
        const float hum = n_hum_.GetNoise((float)x, (float)z), tmp = n_temp_.GetNoise((float)x, (float)z);
        if (hum > 0.05f && tmp > -0.35f) {
            out.marsh = true;
            out.marsh_level = (int)std::floor(t->filled[ci]) - 2;
            // puddles are coherent blobs (~20 m) of the flat floor
            out.puddle = n_detail_.GetNoise((float)x * 1.7f + 3000.0f, (float)z * 1.7f + 3000.0f) > 0.12f;
            if (out.puddle) out.water_level = std::max(out.water_level, out.marsh_level);
        }
    }
    // Rivers: segments from each nearby cell with enough drainage to its downstream cell.
    float best_d = 1e9f;
    int best_k = -1;
    float best_t = 0.0f;
    for (int dj = -1; dj <= 1; dj++)
        for (int di = -1; di <= 1; di++) {
            const int k = (j + dj) * HWIN + (i + di);
            if (t->dir[k] < 0 || t->lake[k] || c.acc(k) < STREAM_ACC) continue;
            const int nk = downstream(k);
            float tt;
            float d = seg_dist((float)x, (float)z, centre_x(k), centre_z(k), centre_x(nk), centre_z(nk), tt);
            if (d < best_d) { best_d = d; best_k = k; best_t = tt; }
        }
    if (best_k >= 0) {
        const float strength = std::sqrt(c.acc(best_k) / STREAM_ACC);
        const float half_w = river_half_width(strength);
        const float depth = river_depth(strength);
        const float bank_w = half_w * 1.5f + 6.0f; // gentle banks: valley floor widens with the river
        if (best_d < half_w + bank_w) {
            const int nk = downstream(best_k);
            const float up = t->filled[best_k], down = t->filled[nk];
            float surf = up + (down - up) * best_t;
            surf = std::min(surf, (float)base_height); // never above the local ground
            // Waterfall sheet over the cliff part of this segment (precomputed per cell).
            if (t->fall_t0[best_k] >= 0.0f && best_t >= t->fall_t0[best_k] && best_t <= t->fall_t1[best_k] && best_d < half_w * 0.6f)
                out.fall_top = t->fall_top[best_k];
            const float mask = 1.0f - smoothstep(half_w, half_w + bank_w, best_d);
            out.river = mask;
            const int water = (int)std::floor(surf) - 1;
            // flat-bottomed channel: full depth over the inner half, rising to the banks
            out.bed = (int)std::floor(surf) - 1 - (int)std::lround(depth * (1.0f - smoothstep(0.45f * half_w, half_w, best_d)));
            if (best_d < half_w && water > out.bed) out.water_level = std::max(out.water_level, water);
            if (best_d >= half_w * 0.9f && best_d < half_w * 1.8f && base_height <= water + 2 && base_height >= water - 1) out.shore = true;
            if (out.fall_top >= 0 && out.fall_top <= out.water_level) out.fall_top = -1;
            // Springs: first cell of a stream (no upstream stream cells).
            bool upstream = false;
            for (int d = 0; d < 8 && !upstream; d++) {
                const int uk = (best_k / HWIN + D8Z[d]) * HWIN + (best_k % HWIN + D8X[d]);
                upstream = t->dir[uk] >= 0 && c.acc(uk) >= STREAM_ACC && downstream(uk) == best_k;
            }
            out.spring = !upstream && best_t < 0.15f;
        }
    }
    return out;
}

Dictionary WorldGen::hydrology_stats(int x, int z) const {
    HydroCtx c;
    Dictionary d;
    if (!hydro_ctx(x, z, c)) return d;
    const HydroTile *t = c.t;
    int streams = 0, lakes = 0, falls = 0;
    float max_acc = 0.0f;
    for (int k = 0; k < HWIN * HWIN; k++) {
        if (t->lake[k]) lakes++;
        if (t->acc[k] >= STREAM_ACC && t->dir[k] >= 0) {
            streams++;
            max_acc = std::max(max_acc, t->acc[k]);
            if (t->fall_t0[k] >= 0.0f) falls++;
        }
    }
    d["tile"] = Vector2i(t->tx, t->tz);
    d["window_cells"] = HWIN * HWIN;
    d["stream_cells"] = streams;
    d["lake_cells"] = lakes;
    d["waterfall_segments"] = falls;
    d["max_accumulation"] = max_acc;
    d["cell_acc"] = t->acc[c.ci];
    d["cell_acc_blended"] = c.acc(c.ci);
    d["cell_filled"] = t->filled[c.ci];
    d["cell_h"] = t->h[c.ci];
    d["seam_blend_x"] = c.wx;
    d["seam_blend_z"] = c.wz;
    return d;
}

// ---------- decorations ----------

bool WorldGen::tree_at_cell(int cx, int cz, Tree &out) const {
    uint32_t hsh = hash3(cx, cz, seed_);
    int jx = (int)(hsh & 7), jz = (int)((hsh >> 3) & 7);
    int tx = cx * TREE_CELL + jx, tz = cz * TREE_CELL + jz;
    Column col = column_base(tx, tz, false);
    if (col.height <= sea_ + 2 || col.alpine) return false;
    Hydro hy = hydro_at(tx, tz, col.height);
    if (hy.river > 0.05f || hy.lake || hy.puddle) return false;
    float density = technosim_biome::TREE_DENSITY[col.biome];
    if (density <= 0.0f) return false;
    // Thin out toward the tree line (stunted, sparse).
    const int above = col.height - sea_;
    const int tree_line = technosim_biome::tree_line(col.t);
    if (above > tree_line - 60) density *= std::clamp((tree_line - above) / 60.0f, 0.1f, 1.0f);
    float roll = (float)((hsh >> 6) & 1023) / 1023.0f;
    if (roll >= density) return false;
    out.x = tx;
    out.z = tz;
    out.base = hy.marsh ? marsh_floor(col.height, hy) : col.height;
    out.kind = technosim_biome::TREE_KIND[col.biome];
    if ((float)((hsh >> 21) & 1023) / 1024.0f < technosim_biome::TREE_MIX[col.biome]) out.kind = technosim_biome::TREE_MIX_KIND[col.biome];
    const int vary = (int)((hsh >> 16) % 6), wide = (int)((hsh >> 20) & 1);
    switch (out.kind) {
        case TREE_SPRUCE: out.trunk_h = 10 + vary; out.radius = 3; break;       // 5-8 m cones
        case TREE_ACACIA: out.trunk_h = 6 + vary / 2; out.radius = 3 + wide; break; // short trunk, flat wide crown
        case TREE_JUNGLE: out.trunk_h = 15 + vary + 2 * wide; out.radius = 3 + wide; break; // tall emergent trees
        case TREE_BIRCH: out.trunk_h = 9 + vary; out.radius = 2; break;                     // slender, a narrow crown
        default:
            out.trunk_h = 8 + vary;
            out.radius = 2 + wide;
            if (col.biome == technosim_biome::OLD_GROWTH_FOREST) { out.trunk_h += 4; out.radius = 3; }
            break;
    }
    if (above > tree_line - 60) { out.trunk_h = 5 + (int)((hsh >> 16) % 3); out.radius = 2; }
    return true;
}

// Crown shapes per kind, as a leaf radius per voxel layer (the column loop
// compares it with the column's squared distance to the trunk):
//   oak    5 layers around the trunk top: r-1, r, r, r-1, r-2
//   spruce a cone from 3 voxels above the base to 1 above the trunk top with
//          alternating fringes (0, 0, 1, 1, 2, 1, 3, 2, 3, 2 ... from the tip down)
//   acacia a flat umbrella: r at the trunk top, r-2 above it
//   jungle 6 layers over a tall trunk: r-2, r-1, r, r, r-1, r-2
//   birch  a narrow column: r-1, r, r, r, r-1, r-2 from 4 below the trunk top
int WorldGen::canopy_radius(const Tree &t, int wy) {
    const int top = t.base + t.trunk_h, r = t.radius;
    switch (t.kind) {
        case TREE_SPRUCE: {
            const int hi = top + 1;
            if (wy > hi || wy < t.base + 3) return -1;
            const int u = hi - wy;
            int lr = std::min(r, u / 2);
            if (u >= 3 && (u & 1)) lr = std::max(1, lr - 1);
            return lr;
        }
        case TREE_ACACIA:
            if (wy == top) return r;
            if (wy == top + 1) return r - 2;
            return -1;
        case TREE_JUNGLE: {
            static const int J[6] = {-2, -1, 0, 0, -1, -2};
            const int k = wy - (top - 3);
            return (k < 0 || k > 5) ? -1 : r + J[k];
        }
        case TREE_BIRCH: {
            static const int B[6] = {-1, 0, 0, 0, -1, -2};
            const int k = wy - (top - 4);
            return (k < 0 || k > 5) ? -1 : r + B[k];
        }
        default: {
            static const int O[5] = {-1, 0, 0, -1, -2};
            const int k = wy - (top - 2);
            return (k < 0 || k > 4) ? -1 : r + O[k];
        }
    }
}

int WorldGen::trunk_top(const Tree &t) {
    const int top = t.base + t.trunk_h;
    if (t.kind == TREE_SPRUCE) return top - 1;   // the cone's tip is needles
    if (t.kind == TREE_ACACIA || t.kind == TREE_BIRCH) return top;
    return top + 1;
}

uint16_t WorldGen::tree_log(int kind) {
    static const uint16_t L[5] = {WOOD, SPRUCE_LOG, ACACIA_LOG, JUNGLE_LOG, BIRCH_LOG};
    return L[std::clamp(kind, 0, 4)];
}

uint16_t WorldGen::tree_leaves(int kind) {
    static const uint16_t L[5] = {LEAVES, SPRUCE_LEAVES, ACACIA_LEAVES, JUNGLE_LEAVES, BIRCH_LEAVES};
    return L[std::clamp(kind, 0, 4)];
}

uint16_t WorldGen::tree_bark(int kind) {
    static const uint16_t L[5] = {OAK_BARK, SPRUCE_BARK, ACACIA_BARK, JUNGLE_BARK, BIRCH_BARK};
    return L[std::clamp(kind, 0, 4)];
}

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

// ---------- ground decorations ----------

bool WorldGen::stamp_at_cell(int cx, int cz, Stamp &out) const {
    using namespace technosim_biome;
    const uint32_t hs = hash3(cx, cz, seed_ + 401);
    out.x = cx * STAMP_CELL + 2 + (int)((hs >> 10) % (STAMP_CELL - 4));
    out.z = cz * STAMP_CELL + 2 + (int)((hs >> 15) % (STAMP_CELL - 4));
    Column col = column_base(out.x, out.z, false);
    if (col.height <= sea_ + 1) return false;
    const float roll = (float)(hs & 1023) / 1024.0f;
    const float bush = DECOR_BUSH[col.biome], boulder = DECOR_BOULDER[col.biome], dead = DECOR_DEAD_TREE[col.biome];
    const float fallen = DECOR_FALLEN_LOG[col.biome];
    if (roll < bush) out.kind = STAMP_BUSH;
    else if (roll < bush + boulder) out.kind = STAMP_BOULDER;
    else if (roll < bush + boulder + dead) out.kind = STAMP_DEAD_TREE;
    else if (roll < bush + boulder + dead + fallen) out.kind = STAMP_FALLEN_LOG;
    else return false;
    Hydro hy = hydro_at(out.x, out.z, col.height);
    if (hy.river > 0.05f || hy.lake || hy.puddle) return false;
    out.seed = hash3(out.x, out.z, seed_ + 409);
    out.tree_kind = TREE_KIND[col.biome];
    if ((float)((out.seed >> 12) & 1023) / 1024.0f < TREE_MIX[col.biome]) out.tree_kind = TREE_MIX_KIND[col.biome];
    out.mossy = TREE_DENSITY[col.biome] >= 0.25f;   // forests: moss on boulders
    switch (out.kind) {
        case STAMP_BUSH: out.size = 1 + (int)((out.seed >> 4) & 1); break;       // radius 1-2
        case STAMP_BOULDER: out.size = 1 + (int)((out.seed >> 4) % 3); break;    // radius 1-3
        case STAMP_FALLEN_LOG: out.size = 4 + (int)((out.seed >> 4) % 4); break; // length 4-7, centred on the anchor
        default: out.size = 3 + (int)((out.seed >> 4) % 4); break;               // dead trunk 3-6
    }
    return true;
}

int WorldGen::stamps_near(int x0, int z0, int x1, int z1, Stamp *out, int max_out) const {
    int n = 0;
    const int cx0 = floor_div(x0 - STAMP_REACH, STAMP_CELL), cx1 = floor_div(x1 + STAMP_REACH, STAMP_CELL);
    const int cz0 = floor_div(z0 - STAMP_REACH, STAMP_CELL), cz1 = floor_div(z1 + STAMP_REACH, STAMP_CELL);
    for (int cz = cz0; cz <= cz1 && n < max_out; cz++)
        for (int cx = cx0; cx <= cx1 && n < max_out; cx++) {
            Stamp s;
            if (stamp_at_cell(cx, cz, s)) out[n++] = s;
        }
    return n;
}

int WorldGen::stamp_height(const Stamp &s) {
    switch (s.kind) {
        case STAMP_BUSH: return s.size + 1;
        case STAMP_BOULDER: return s.size + 2;   // + a moss carpet
        case STAMP_FALLEN_LOG: return 2;         // the log + moss or a mushroom on it
        default: return s.size;
    }
}

// Shapes, per column around the anchor, draped on each column's own ground:
//   bush     a leaf mound of the biome's tree kind over a log core (Minecraft's jungle bush),
//            radius 1-2, a ragged edge from the seed;
//   boulder  a stone dome of radius 1-3, moss carpet on top in forests;
//   dead     a bare trunk of the biome's wood, 3-6 tall, with one stub branch;
//   fallen   a log of bark blocks lying along x or z, 4-7 long, moss and the odd mushroom on top.
uint16_t WorldGen::stamp_voxel(const Stamp &s, int x, int z, int dy) {
    const int dx = x - s.x, dz = z - s.z, d2 = dx * dx + dz * dz;
    if (dy < 0) return AIR;
    switch (s.kind) {
        case STAMP_BUSH: {
            if (d2 == 0) return dy == 0 ? tree_log(s.tree_kind) : (dy <= s.size ? tree_leaves(s.tree_kind) : AIR);
            const int r = s.size;
            if (d2 > r * r + 1) return AIR;
            const int hgt = d2 <= (r - 1) * (r - 1) + 1 ? r : r - 1;   // lower towards the edge
            if (hash3(x, z, (int)s.seed) % 5 == 0 && d2 > 1) return AIR; // ragged edge
            return dy <= hgt ? tree_leaves(s.tree_kind) : AIR;
        }
        case STAMP_BOULDER: {
            const float d = std::sqrt((float)d2), r = (float)s.size + 0.4f;
            if (d > r) return AIR;
            const int hgt = std::max(1, (int)std::lround((r - d) * 1.3f));
            if (dy < hgt) return STONE;
            if (dy == hgt && s.mossy && hash3(x, z, (int)s.seed + 1) % 3 != 0) return MOSS;
            return AIR;
        }
        case STAMP_FALLEN_LOG: {
            const bool along_x = (s.seed >> 9) & 1;
            const int a = along_x ? dx : dz, b = along_x ? dz : dx;
            const int lo = -s.size / 2, hi = lo + s.size - 1;
            if (b != 0 || a < lo || a > hi) return AIR;
            if (dy == 0) return tree_bark(s.tree_kind);
            if (dy == 1) {
                const uint32_t h = hash3(x, z, (int)s.seed + 3) % 8;
                if (h == 0) return MUSHROOM_BROWN;
                if (s.mossy && h < 4) return MOSS;
            }
            return AIR;
        }
        default: {
            if (d2 == 0) return dy < s.size ? tree_log(s.tree_kind) : AIR;
            // one stub branch near the top, on a side picked by the seed
            static const int BX[4] = {1, -1, 0, 0}, BZ[4] = {0, 0, 1, -1};
            const int b = (int)((s.seed >> 8) & 3);
            if (dx == BX[b] && dz == BZ[b]) return (dy == s.size - 2 || dy == s.size - 1) ? tree_log(s.tree_kind) : AIR;
            return AIR;
        }
    }
}

bool WorldGen::ruin_at_cell(int cx, int cz, Ruin &out) const {
    uint32_t hsh = hash3(cx, cz, seed_ + 991);
    if ((hsh & 0xFF) > 30) return false;
    int hw = 5 + (int)((hsh >> 8) % 6);
    int hd = 5 + (int)((hsh >> 12) % 6);
    int margin = RUIN_MAX_HALF + 4;
    int span = RUIN_CELL - 2 * margin;
    int x = cx * RUIN_CELL + margin + (int)((hsh >> 16) % (uint32_t)span);
    int z = cz * RUIN_CELL + margin + (int)((hsh >> 24) % (uint32_t)span);
    Column c = column_base(x, z, false);
    if (c.height < sea_ + 5 || c.alpine) return false;
    Hydro hy = hydro_at(x, z, c.height);
    if (hy.river > 0.05f || hy.lake || hy.marsh) return false;
    if (c.alpine || c.snow || c.height - sea_ > technosim_biome::HIGHLAND_MAX) return false; // lowland and highland biomes only
    int hmin = c.height, hmax = c.height;
    const int cxs[4] = {x - hw, x + hw, x - hw, x + hw};
    const int czs[4] = {z - hd, z - hd, z + hd, z + hd};
    for (int k = 0; k < 4; k++) {
        int hh = column_base(cxs[k], czs[k], false).height;
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

inline bool WorldGen::is_cave(int x, int y, int z, int height) const {
    if (y < 12 || y > height - 5) return false;
    const float fx = (float)x, fy = (float)y, fz = (float)z;
    float depth = (float)(height - y);
    float cheese_threshold = 0.62f - 0.12f * smoothstep(20.0f, 200.0f, depth);
    if (n_cheese_.GetNoise(fx, fy * 1.6f, fz) > cheese_threshold) return true;
    float s1 = n_spag1_.GetNoise(fx, fy * 1.3f, fz);
    float s2 = n_spag2_.GetNoise(fx, fy * 1.3f, fz);
    return (s1 * s1 + s2 * s2) < 0.0115f;
}

// ---------- ore veins (GregTech style, ore_veins.json) ----------


// One vein candidate per cell: filled with chance FILL, type by weight, centre
// jittered by at most JITTER so the whole lens stays inside the cell (a column
// only ever looks at its own cell).
bool WorldGen::vein_at_cell(int cx, int cz, Vein &out) const {
    using namespace technosim_ore;
    const uint32_t h = hash3(cx, cz, seed_ + 7717);
    if ((float)(h & 0xFFFF) >= FILL * 65536.0f) return false;
    int pick = (int)(hash3(cz, cx, seed_ + 7723) % (uint32_t)WEIGHT_TOTAL), t = 0;
    while (t < COUNT - 1 && pick >= WEIGHT[t]) { pick -= WEIGHT[t]; t++; }
    const uint32_t hj = hash3(cx + 31, cz - 17, seed_ + 7741);
    const uint32_t span = (uint32_t)(2 * JITTER[t] + 1);
    out.type = t;
    out.x = cx * CELL + CELL / 2 + (int)(hj % span) - JITTER[t];
    out.z = cz * CELL + CELL / 2 + (int)((hj >> 12) % span) - JITTER[t];
    out.y = Y_MIN[t] + (int)(hash3(cx - 5, cz + 11, seed_ + 7753) % (uint32_t)(Y_MAX[t] - Y_MIN[t] + 1));
    return true;
}

// A flat lens: radius wobbles between 60 % and 100 % of SIZE / 2 (never past
// the jitter margin), thickness THICKNESS at the centre thinning to the rim.
float WorldGen::vein_half_thickness(const Vein &v, int x, int z) const {
    using namespace technosim_ore;
    const float a = 0.5f * (float)SIZE[v.type] * (0.8f + 0.2f * n_ore_.GetNoise((float)x * 0.5f, (float)z * 0.5f));
    const float dx = (float)(x - v.x), dz = (float)(z - v.z);
    const float r2 = (dx * dx + dz * dz) / (a * a);
    if (r2 >= 1.0f) return 0.0f;
    return 0.5f * (float)THICKNESS[v.type] * std::sqrt(1.0f - r2);
}

// GregTech layers: primary in the top third, between in the middle, secondary
// at the bottom; DENSITY of the lens is ore, SPORADIC_CHANCE of that is the
// sporadic ore.
inline uint16_t WorldGen::vein_ore(const Vein &v, float half_t, int x, int y, int z) const {
    using namespace technosim_ore;
    const float dy = (float)(y - v.y);
    if (dy > half_t || dy < -half_t) return STONE;
    const uint32_t hh = hash3(x, z, y * 1031 + seed_ + 7759);
    if ((float)(hh & 1023) >= DENSITY[v.type] * 1024.0f) return STONE;
    if ((float)((hh >> 10) & 1023) < SPORADIC_CHANCE * 1024.0f) return (uint16_t)SPORADIC[v.type];
    const float t = (dy + half_t) / (2.0f * half_t); // 0 bottom .. 1 top
    return (uint16_t)(t > 0.6667f ? PRIMARY[v.type] : (t < 0.3333f ? SECONDARY[v.type] : BETWEEN[v.type]));
}

// Ore-bearing stones on the ground near a vein's centre (Vintage Story style):
// the chance per column fades from INDICATOR_CHANCE with the vein top at the
// surface to 0 at INDICATOR_MAX_DEPTH below it.
uint16_t WorldGen::indicator_at(const Vein &v, int x, int z, int ground, uint16_t top) const {
    using namespace technosim_ore;
    const int ind = INDICATOR[v.type];
    if (ind < 0) return AIR;
    if (!(top == GRASS || top == DIRT || top == GRAVEL_STATIC || top == STONE || top == SAND_STATIC || top == SNOW || top == CLAY)) return AIR;
    const int dx = x - v.x, dz = z - v.z;
    if (dx * dx + dz * dz > INDICATOR_RADIUS * INDICATOR_RADIUS) return AIR;
    const int depth = ground - (v.y + THICKNESS[v.type] / 2);
    if (depth > INDICATOR_MAX_DEPTH) return AIR;
    const float p = INDICATOR_CHANCE * (1.0f - (float)std::max(depth, 0) / (float)INDICATOR_MAX_DEPTH);
    return (float)(hash3(x, z, seed_ + 97) % 100000u) < p * 100000.0f ? (uint16_t)ind : AIR;
}

Dictionary WorldGen::vein_at(int x, int z) const {
    using namespace technosim_ore;
    Dictionary d;
    Vein v;
    if (!vein_at_cell(floor_div(x, CELL), floor_div(z, CELL), v)) return d;
    d["name"] = String(NAMES[v.type]);
    d["type"] = v.type;
    d["x"] = v.x;
    d["y"] = v.y;
    d["z"] = v.z;
    d["size"] = SIZE[v.type];
    d["thickness"] = THICKNESS[v.type];
    d["half_thickness"] = vein_half_thickness(v, x, z);
    d["indicator"] = INDICATOR[v.type];
    return d;
}
// Per-column surface vegetation (LOD 0 only): Minecraft-style tufts, flower
// patches, cacti and dead bushes. One hash per column; flowers cluster where a
// 20 m noise is high. Returns AIR for none; height is 1 except for cacti.
// Badlands strata (biomes.json: under/steep "terracotta"): horizontal bands 3
// voxels thick, chosen per band from a fixed mix, that wobble up and down by
// a few voxels over tens of metres so terraces do not look ruled.
uint16_t WorldGen::strata_at(int x, int y, int z) const {
    static const uint16_t MIX[8] = {TERRACOTTA, TERRACOTTA_RED, TERRACOTTA, TERRACOTTA_CREAM, TERRACOTTA_RED, TERRACOTTA, SANDSTONE, TERRACOTTA_RED};
    const int wob = (int)std::lround(n_detail_.GetNoise((float)x * 0.05f + 900.0f, (float)z * 0.05f - 900.0f) * 3.0f);
    return MIX[hash3(floor_div(y + wob, 3), 0, seed_ + 71) & 7];
}

// Spires (biomes.json decor.spire): a 12 m cell holds one with the biome's chance,
// SPIRE_HEIGHT tall, 3 or 5 voxels across at the base and tapering to a point:
// ice fields' packed-ice spikes, badlands' terracotta spires, desert hoodoos.
int WorldGen::spire_at(int biome, int x, int z) const {
    using namespace technosim_biome;
    constexpr int C = 24;
    const int cx = floor_div(x, C), cz = floor_div(z, C);
    const uint32_t hs = hash3(cx, cz, seed_ + 307);
    if ((float)(hs & 1023) / 1024.0f >= DECOR_SPIRE[biome]) return 0;
    const int px = cx * C + 4 + (int)((hs >> 10) % (C - 8));
    const int pz = cz * C + 4 + (int)((hs >> 15) % (C - 8));
    const int lo = SPIRE_HEIGHT[biome][0], hi = std::max(lo, SPIRE_HEIGHT[biome][1]);
    const int height = lo + (int)((hs >> 20) % (uint32_t)(hi - lo + 1));
    const float r = height > 15 ? 2.0f : 1.0f;
    const float d = std::sqrt((float)((x - px) * (x - px) + (z - pz) * (z - pz)));
    if (d > r + 0.5f) return 0;
    return std::max(1, (int)((float)height * (1.0f - d / (r + 1.0f))));
}

uint16_t WorldGen::vegetation_at(const Column &col, int x, int z, uint16_t top, const Hydro &hy, int &height) const {
    height = 1;
    const bool marsh = hy.marsh;
    const uint32_t v = hash3(x, z, seed_ + 31);
    const int r = (int)(v & 1023);
    using namespace technosim_biome;
    // Reeds on lake and river shores (not in cold biomes: no reeds in snow).
    if (hy.shore && !col.snow && col.t > -0.42f && (top == GRASS || top == DIRT || top == CLAY || top == SAND_STATIC) && r < 330) {
        height = 2;
        return REED;
    }
    if (col.biome == DESERT || col.biome == BADLANDS) {
        if (top != SAND_STATIC && top != GRAVEL_STATIC) return AIR;
        if (col.biome == DESERT && r < 5) { height = 2 + (int)((v >> 10) % 3); return CACTUS; }
        if (r < 18) return DEAD_BUSH;
        return AIR;
    }
    // Boulders: 2x2 columns of stone (1-2 high) on rocky open ground, from a coarser hash.
    const bool rocky = col.biome == HIGHLANDS || col.biome == MOUNTAINS || col.biome == ALPINE || col.biome == SHRUBLAND;
    if (!marsh && rocky && top != SAND_STATIC) {
        const uint32_t bh = hash3(x >> 1, z >> 1, seed_ + 47);
        if ((bh & 1023) < 3) { height = 1 + (int)((bh >> 10) & 1); return STONE; }
    }
    // Reeds at marsh puddle edges (the puddle noise just below its threshold), 1 m tall.
    if (marsh && (top == DIRT || top == CLAY || top == GRASS)) {
        const float pn = n_detail_.GetNoise((float)x * 1.7f + 3000.0f, (float)z * 1.7f + 3000.0f);
        if (pn > 0.02f && pn <= 0.12f && r < 420) { height = 2; return REED; }
    }
    // Pebbles on bare rock and gravel, flint on gravel (ground pickups, ADR 0019).
    if ((top == GRAVEL_STATIC || top == STONE) && r < 50 && !marsh) return PEBBLES;
    if (top == GRAVEL_STATIC && r >= 50 && r < 66 && !marsh) return FLINT_STONES;
    if (top != GRASS && !(marsh && top == DIRT)) return AIR;
    // Wild crops (ADR 0022): rare patches, a good reason to start a farm. A 24 m cell
    // holds a patch with a small chance (plains: wheat every ~140 m, carrots every
    // ~170 m); the patch is a 1.5 m disc around a jittered centre, a quarter of its
    // grass columns carrying the plant (about 6 plants).
    if (!marsh) {
        int wheat = 0, carrot = 0, pumpkin = 0;   // per 1000 cells
        switch (col.biome) {
            case PLAINS: wheat = 30; carrot = 20; pumpkin = 12; break;
            case SAVANNA: wheat = 15; break;
            case FOREST: carrot = 12; pumpkin = 10; break;
            default: break;
        }
        if (wheat + carrot + pumpkin > 0) {
            constexpr int PC = 48;
            const int pcx = floor_div(x, PC), pcz = floor_div(z, PC);
            const uint32_t ph = hash3(pcx, pcz, seed_ + 211);
            const int roll = (int)(ph % 1000u);
            if (roll < wheat + carrot + pumpkin) {
                const int px = pcx * PC + 8 + (int)((ph >> 10) % (PC - 16));
                const int pz = pcz * PC + 8 + (int)((ph >> 20) % (PC - 16));
                const int dx = x - px, dz = z - pz;
                if (dx * dx + dz * dz <= 10 && ((v >> 16) & 3u) == 0) {
                    if (roll >= wheat + carrot) return ((v >> 18) & 1u) ? PUMPKIN : AIR;   // pumpkins: fewer, bigger
                    return roll < wheat ? WILD_WHEAT : WILD_CARROT;
                }
            }
        }
    }
    // Per mille of grass columns; pebbles, twigs and flint are the starting pickups.
    int grass = 0, flower = 0, bush = 0, shroom = 0, pebble = 0, twig = 0, flint = 0;
    int fern = 0;   // ferns take this share of the grass columns (shady, wet biomes)
    int berry = 0;  // blueberry bushes, per mille
    if (marsh) grass = 560;
    else switch (col.biome) {
        case PLAINS: grass = 300; flower = 36; pebble = 6; twig = 2; flint = 2; break;
        case FOREST: grass = 170; flower = 14; shroom = 10; pebble = 4; twig = 12; flint = 1; fern = 15; berry = 6; break;
        case OLD_GROWTH_FOREST: grass = 200; flower = 10; shroom = 22; pebble = 3; twig = 16; fern = 70; berry = 4; break;
        case HIGHLANDS: grass = 110; flower = 10; pebble = 20; twig = 2; flint = 4; break;
        case MOUNTAINS: grass = 50; flower = 4; pebble = 30; flint = 4; break;
        case ALPINE: grass = 25; flower = 6; pebble = 30; flint = 3; break;
        case TAIGA: grass = 90; shroom = 12; pebble = 8; twig = 12; flint = 1; fern = 50; berry = 10; break;
        case SHRUBLAND: grass = 120; bush = 40; pebble = 14; twig = 4; flint = 3; break;
        case SAVANNA: grass = 420; bush = 12; twig = 3; flint = 2; break;
        case JUNGLE: grass = 350; flower = 20; shroom = 14; twig = 10; fern = 110; break;
        case SWAMP: grass = 450; shroom = 18; twig = 6; fern = 40; break;
        default: break;
    }
    if (bush > 0 && r >= 1000 - bush) return DEAD_BUSH;
    // the remains of an old kill (ADR 0027): rare, where predators and grazers meet
    if (!marsh && (col.biome == PLAINS || col.biome == FOREST || col.biome == TAIGA || col.biome == SAVANNA || col.biome == SHRUBLAND)
            && (hash3(x, z, seed_ + 97) % 20000u) < 3u) return CARCASS;
    if (shroom > 0 && r >= 1000 - bush - shroom) {
        // red caps in the wet, dark biomes, brown elsewhere
        const bool red = (col.biome == SWAMP || col.biome == OLD_GROWTH_FOREST || col.biome == JUNGLE) && ((v >> 14) & 3) == 0;
        return red ? MUSHROOM_RED : MUSHROOM_BROWN;
    }
    if (pebble > 0 && r >= 1000 - bush - shroom - pebble) return PEBBLES;
    if (twig > 0 && r >= 1000 - bush - shroom - pebble - twig) return TWIGS;
    if (flint > 0 && r >= 1000 - bush - shroom - pebble - twig - flint) return FLINT_STONES;
    if (berry > 0 && r >= 1000 - bush - shroom - pebble - twig - flint - berry) return BLUEBERRY_BUSH;
    if (flower > 0) {
        const float patch = n_detail_.GetNoise((float)x * 2.0f + 5000.0f, (float)z * 2.0f - 5000.0f);
        flower = patch > 0.3f ? flower * 5 : flower / 4;
    }
    if (r < fern) return FERN;
    if (r < grass) return TALL_GRASS;
    if (r < grass + flower) {
        // each biome its own mix of the five flowers (dandelion-yellow, poppy-red, cornflower, daisy, allium)
        static const uint16_t MEADOW[4] = {FLOWER, FLOWER_RED, OXEYE_DAISY, CORNFLOWER};
        static const uint16_t WOODS[4] = {FLOWER, FLOWER_RED, ALLIUM, OXEYE_DAISY};
        static const uint16_t UPLAND[4] = {CORNFLOWER, OXEYE_DAISY, FLOWER, CORNFLOWER};
        static const uint16_t WET[4] = {FLOWER_RED, ALLIUM, FLOWER_RED, ALLIUM};
        const uint16_t *mix = MEADOW;
        if (col.biome == FOREST || col.biome == OLD_GROWTH_FOREST) mix = WOODS;
        else if (col.biome == HIGHLANDS || col.biome == MOUNTAINS || col.biome == ALPINE) mix = UPLAND;
        else if (col.biome == JUNGLE || col.biome == SWAMP) mix = WET;
        // flowers grow in patches of one kind: the kind follows a coarse cell, not the column
        const uint32_t fk = hash3(x >> 3, z >> 3, seed_ + 223);
        return mix[fk & 3];
    }
    return AIR;
}

// ---------- blocks ----------

// Soil voxels per column counted from the surface (depth 1 is the top material,
// 2..SOIL_VOXELS the biome's under material, deeper is rock, sandstone or ore).
static const int SOIL_VOXELS = 4;
// Cave floors hold their biome's pool (biomes.json surface.pool) where this 2D noise is above it.
static constexpr float POOL_NOISE = 0.35f;

// At lod 1 caves are carved only this close to the surface (cave mouths).
static const int CAVE_MOUTH_DEPTH = 24;

PackedByteArray WorldGen::generate_block(Vector3i origin, Vector3i size, int lod, int bytes_per_voxel) const {
    PackedByteArray out;
    const int step = 1 << lod;
    const int sx = size.x, sy = size.y, sz = size.z;
    if (sx <= 0 || sy <= 0 || sz <= 0) return out;
    if (origin.y > world_height_) return out;
    const int n = sx * sy * sz;
    std::vector<uint16_t> tmp(n, 0);
    uint16_t *w = tmp.data();
    bool any = false;
    // Caves at lod 0, and at lod 1 only within CAVE_MOUTH_DEPTH of the surface:
    // cave mouths on hillsides then show to the LOD1 edge instead of popping
    // in at the LOD0 edge (ADR 0020); deep caves stay lod 0 (nobody sees them).
    const bool do_caves = lod <= 1;
    const int cave_band = lod == 0 ? 1 << 30 : CAVE_MOUTH_DEPTH;
    const bool do_ores = lod == 0;
    // Micro vegetation up to lod 1: sampling every other column keeps a quarter
    // of the tufts, drawn at twice the size, so cover thins instead of stopping
    // at the LOD0 edge (ADR 0020).
    const bool do_veg = lod <= 1;
    const int y_top = origin.y + sy * step;
    const bool above_sea = y_top > sea_ && origin.y < world_height_;
    // Foliage thins with LOD (ADR 0020): micro vegetation to lod 1, trees as
    // trunk + canopy up to lod 2 (sampling already drops most trunks there), and
    // from lod 3 on a tree is just a leaf-coloured surface cell under its canopy,
    // so far forests keep their colour without any tree geometry.
    Tree trees[256];
    Ruin ruins[4];
    int ntrees = 0, nruins = 0, nstamps = 0;
    Stamp stamps[128];   // ground decorations (bushes, boulders, dead trees), lod <= 1 like micro vegetation
    if (above_sea) {
        nruins = ruins_near(origin.x, origin.z, origin.x + sx * step, origin.z + sz * step, ruins, 4);
        if (lod <= 2) ntrees = trees_near(origin.x, origin.z, origin.x + sx * step, origin.z + sz * step, trees, 256);
        if (lod <= 1) nstamps = stamps_near(origin.x, origin.z, origin.x + sx * step, origin.z + sz * step, stamps, 128);
    }
    const bool far_trees = lod >= 3 && above_sea;
    for (int z = 0; z < sz; z++) {
        const int wz = origin.z + z * step;
        for (int x = 0; x < sx; x++) {
            const int wx = origin.x + x * step;
            Column col = column(wx, wz);
            if (col.biome == technosim_biome::BADLANDS && col.slope == 0.0f) {
                // column() measures slope only on mountains and coasts; badlands need it for their strata faces
                const int hx = column_base(wx + 2, wz, false).height, hz = column_base(wx, wz + 2, false).height;
                col.slope = std::max(std::abs(hx - col.height), std::abs(hz - col.height)) * 0.5f;
            }
            // Ore vein of this column's cell (one cell lookup); the lens thickness
            // here only matters at lod 0 where ores are placed.
            Vein vein;
            const bool has_vein = vein_at_cell(floor_div(wx, technosim_ore::CELL), floor_div(wz, technosim_ore::CELL), vein);
            const float vein_ht = (has_vein && do_ores) ? vein_half_thickness(vein, wx, wz) : 0.0f;
            int h = col.height;
            int water_level = sea_;
            bool riverbed = false, lakebed = false, marshbed = false;
            Hydro hy = hydro_at(wx, wz, col.height);
            if (hy.river > 0.0f) {
                // carve the channel: bed in the middle, banks blend outward
                const int target = hy.bed;
                if (h > target) h = (int)std::lround((float)h + ((float)target - (float)h) * hy.river);
                riverbed = hy.river > 0.6f;
            }
            if (hy.water_level > water_level) water_level = hy.water_level;
            if (hy.lake && h < hy.water_level) lakebed = true;
            if (hy.river > 0.0f && h < hy.water_level) riverbed = true;
            if (hy.marsh && hy.river < 0.3f) {
                h = marsh_floor(h, hy);
                marshbed = hy.puddle;
            }
            int no_cave_above = -1;
            if (hy.lake || hy.river > 0.0f || hy.marsh) no_cave_above = water_level - 40;

            const Ruin *ruin = nullptr;
            for (int i = 0; i < nruins; i++)
                if (in_ruin(ruins[i], wx, wz, 2)) { ruin = &ruins[i]; break; }
            if (ruin) {
                const int ground = ruin->base;
                if (in_ruin(*ruin, wx, wz, 0)) h = ground; else if (h > ground + 1) h = ground + 1;
            }
            int spike = 0;   // spire voxels on top of the ground (decor.spire: ice spikes, hoodoos)
            if (technosim_biome::DECOR_SPIRE[col.biome] > 0.0f && !ruin && !lakebed && !riverbed && !marshbed && h > water_level) {
                spike = spire_at(col.biome, wx, wz);
                h += spike;
            }
            const uint16_t spire_m = spike > 0 ? (uint16_t)technosim_biome::SPIRE_MATERIAL[col.biome] : AIR;
            const bool beach_sand = col.biome == BIOME_BEACH || (col.biome == BIOME_OCEAN && col.c > -0.4f);
            const bool steep = col.slope > 1.1f;      // > ~48 degrees: bare rock
            const bool scree = col.slope > 0.75f && col.slope <= 1.1f && col.alpine;
            const bool cold = col.t < -0.45f || col.snow;
            // Surface layers for this column: top voxel and the soil under it.
            uint16_t top_m, under_m;
            if (lakebed || riverbed || marshbed) {
                const uint32_t bh = hash3(wx, wz, 5) & 7;
                top_m = bh < 3 ? CLAY : (bh < 5 ? SAND_STATIC : GRAVEL_STATIC);
                under_m = (bh & 1) ? CLAY : GRAVEL_STATIC;
            } else if (hy.river > 0.08f && h <= water_level + 2) {
                top_m = under_m = SAND_STATIC; // river bank
            } else if (col.snow && !steep) {
                top_m = SNOW; under_m = STONE; // permanent snow over rock
            } else if (steep) {
                // bare rock face; badlands faces show their strata
                top_m = under_m = technosim_biome::SURFACE_UNDER[col.biome] == TERRACOTTA ? TERRACOTTA : STONE;
            } else if (scree) {
                top_m = under_m = GRAVEL_STATIC; // scree slopes below the cliffs
            } else if (col.alpine) {
                // alpine zone: rock with patches of thin grass/gravel
                const uint32_t ah = hash3(wx, wz, 19) & 7;
                top_m = ah < 3 ? GRASS : (ah < 5 ? GRAVEL_STATIC : STONE);
                under_m = ah < 3 ? DIRT : STONE;
            } else if (hy.marsh) {
                const uint32_t mh = hash3(wx, wz, 29) & 7;
                top_m = mh < 2 ? CLAY : (mh < 5 ? DIRT : GRASS);
                under_m = mh < 4 ? CLAY : DIRT;
            } else {
                // Surface materials from biomes.json (ocean floors near the coast are sand).
                top_m = (uint16_t)technosim_biome::SURFACE_TOP[col.biome];
                under_m = (uint16_t)technosim_biome::SURFACE_UNDER[col.biome];
                if (col.biome == BIOME_OCEAN && !beach_sand) top_m = under_m = GRAVEL_STATIC;
                if (technosim_biome::SURFACE_STEEP[col.biome] >= 0 && col.slope > 0.6f) top_m = (uint16_t)technosim_biome::SURFACE_STEEP[col.biome];
            }
            if (spike > 0) top_m = under_m = spire_m;
            const bool strata = under_m == TERRACOTTA;   // TERRACOTTA in the tables stands for banded strata
            // Sedimentary layer under deserts, badlands and beaches (no ores in it).
            const int sandstone_depth = col.biome == technosim_biome::BADLANDS ? 24 : col.biome == BIOME_DESERT ? 16
                : ((col.biome == BIOME_BEACH || col.biome == technosim_biome::SNOWY_BEACH) ? 8 : 0);
            // Cave biome bands for this column (3D layer): floors of carved caves get the
            // cave biome's top material (lush: grass, fungal: dirt, frozen: snow, ...).
            const int ti = technosim_biome::band_of(technosim_biome::TEMP_BAND_MAX, col.t);
            const int hi = technosim_biome::band_of(technosim_biome::HUM_BAND_MAX, col.h);
            // Vegetation standing on the surface (not in water, not on the sheet).
            uint16_t veg = AIR;
            int veg_h = 0;
            if (do_veg && !ruin && h >= water_level && hy.fall_top < 0 && h > sea_) {
                veg = vegetation_at(col, wx, wz, top_m, hy, veg_h);
                if (has_vein && (veg == AIR || veg == TALL_GRASS)) {
                    const uint16_t ind = indicator_at(vein, wx, wz, h, top_m);
                    if (ind != AIR) { veg = ind; veg_h = 1; }
                }
            }
            uint16_t *colp = w + sy * (x + sx * z);
            // Far LOD: is this column under a tree's canopy? (one tree cell lookup)
            bool far_canopy = false;
            uint16_t far_leaves = LEAVES;
            if (far_trees && !ruin && h > water_level && technosim_biome::TREE_DENSITY[col.biome] > 0.0f) {
                Tree t;
                const int tcx = (int)std::floor((float)wx / TREE_CELL), tcz = (int)std::floor((float)wz / TREE_CELL);
                if (tree_at_cell(tcx, tcz, t)) {
                    const int dx = wx - t.x, dz = wz - t.z;
                    far_canopy = dx * dx + dz * dz <= t.radius * t.radius + 1;
                    far_leaves = tree_leaves(t.kind);
                }
            }
            // Ground decorations touching this column: dry open ground only (no ruins,
            // water, waterfalls or spires); a decoration on the ground voxel replaces its plant.
            int col_st[4];
            int ncs = 0, stamp_hi = 0;
            if (nstamps > 0 && !ruin && spike == 0 && h >= water_level && hy.fall_top < 0 && !lakebed && !riverbed) {
                for (int i = 0; i < nstamps && ncs < 4; i++) {
                    if (std::abs(wx - stamps[i].x) > STAMP_REACH || std::abs(wz - stamps[i].z) > STAMP_REACH) continue;
                    col_st[ncs++] = i;
                    stamp_hi = std::max(stamp_hi, stamp_height(stamps[i]));
                    if (stamp_voxel(stamps[i], wx, wz, 0) != AIR) veg = AIR;
                }
            }
            // Trees touching this column
            int trunk_lo = 0, trunk_hi = -1;
            uint16_t trunk_m = WOOD;
            int can_t = -1, can_d2 = 0;   // nearest tree whose crown covers this column
            for (int t = 0; t < ntrees; t++) {
                // A coarse cell holds the trunk when the tree's base lies in its
                // footprint (lod 0: the column itself); canopy distance is measured
                // from the cell centre, so lod 1-2 trees keep trunk and crown instead
                // of the one in 2^(2 lod) whose base hits the sampled column (ADR 0020).
                const bool trunk_here = step == 1 ? (wx == trees[t].x && wz == trees[t].z)
                    : (trees[t].x >= wx && trees[t].x < wx + step && trees[t].z >= wz && trees[t].z < wz + step);
                const int dx = wx + step / 2 - trees[t].x, dz = wz + step / 2 - trees[t].z;
                const int d2 = trunk_here ? 0 : dx * dx + dz * dz;
                if (d2 > trees[t].radius * trees[t].radius + 1) continue;
                bool blocked = false;
                for (int i = 0; i < nruins && !blocked; i++) blocked = in_ruin(ruins[i], trees[t].x, trees[t].z, 3);
                if (blocked) continue;
                if (trunk_here) { trunk_lo = trees[t].base; trunk_hi = trunk_top(trees[t]); trunk_m = tree_log(trees[t].kind); veg = AIR; }
                if (can_t < 0 || d2 < can_d2) { can_t = t; can_d2 = d2; }
            }
            // Ruin walls for this column
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
                    int hgt = corner ? 6 + (int)(sh % 4) : (int)(sh % 7);
                    if (!corner && ((sh >> 8) & 7) == 0) hgt = 0;
                    wall_top = ruin->base + hgt;
                    wall_seed = hash3(wx, wz, (int)ruin->seed);
                } else if (!in_ruin(*ruin, wx, wz, 0)) {
                    rubble = (hash3(wx, wz, (int)ruin->seed + 5) & 15) == 0;
                }
            }
            // Cave mask for the column (one voxel above and below the block too, so
            // cave floors and ceilings are known at block borders).
            bool cave[sy + 2];
            if (do_caves) {
                for (int y = -1; y <= sy; y++) {
                    const int wy = origin.y + y * step;
                    const bool cave_ok = no_cave_above < 0 || wy < no_cave_above;
                    cave[y + 1] = wy < h && h - wy <= cave_band && cave_ok && is_cave(wx, wy, wz, h) && !(wy < sea_ && h < sea_ + 2);
                }
            }
            for (int y = 0; y < sy; y++) {
                const int wy = origin.y + y * step;
                uint16_t m = AIR;
                if (ruin && wy >= ruin->base && wy <= ruin->base + 10 && in_ruin(*ruin, wx, wz, 2)) {
                    if (wy == ruin->base && in_ruin(*ruin, wx, wz, 0)) m = (hash3(wx, wz, 3) & 3) ? STONE : GRAVEL_STATIC;
                    else if (wall_top >= 0 && wy > ruin->base && wy <= wall_top) {
                        uint32_t vh = hash3(wy, (int)wall_seed, 11);
                        int miss = (vh & 15) < (uint32_t)(2 + (wy - ruin->base)) ? 1 : 0;
                        m = miss ? AIR : STONE;
                    } else if (rubble && wy == ruin->base + 1) m = STONE;
                    colp[y] = m;
                    if (m != AIR) any = true;
                    continue;
                }
                if (wy >= trunk_lo && wy <= trunk_hi) {
                    m = trunk_m;
                } else if (can_t >= 0 && wy >= h) {
                    const int lr = canopy_radius(trees[can_t], wy);
                    if (lr >= 0 && can_d2 <= lr * lr + (lr > 1 ? 1 : 0)) m = tree_leaves(trees[can_t].kind);
                }
                if (m == AIR && ncs > 0 && wy >= h && wy < h + stamp_hi)
                    for (int i = 0; i < ncs && m == AIR; i++) m = stamp_voxel(stamps[col_st[i]], wx, wz, wy - h);
                if (m != AIR) { colp[y] = m; any = true; continue; }
                if (wy < 3) {
                    m = BEDROCK;
                } else if (wy < h) {
                    // This cell covers world y in [wy, wy + step). At lod > 0 it is
                    // classified by what it contains, not by the voxel at wy: a cell
                    // holding the surface voxel (h - 1) is the top material, a buried
                    // cell is soil or rock by majority (ADR 0017: sampling only wy made
                    // most far hillsides dirt and stone under a thin top). lod 0 is
                    // unchanged: d_top == depth, n_solid == 1.
                    const int cell_top = std::min(h - 1, wy + step - 1);
                    const int d_top = h - cell_top;          // 1 when the cell holds the surface
                    const int n_solid = cell_top - wy + 1;
                    const int n_soil = std::clamp(SOIL_VOXELS + 1 - d_top, 0, n_solid);
                    const int depth = h - wy;
                    if (do_caves && cave[y + 1]) {
                        m = (wy < sea_ - 180) ? LAVA : AIR;
                        // Cave detail (deep enough that it is a real cave, not a soil overhang):
                        // stalagmites, pebbles and mushrooms on floors, stalactites on ceilings.
                        if (m == AIR && depth > 6 && lod == 0) {
                            const uint32_t ch = hash3(wx, wz, wy * 7919 + 13) % 1000;
                            if (!cave[y]) {
                                const int cbi = technosim_biome::cave_biome(ti, hi,
                                    technosim_biome::band_of(technosim_biome::DEPTH_BAND_MAX, (float)depth, technosim_biome::DEPTH_BAND_COUNT),
                                    technosim_biome::band_of(technosim_biome::CAVE_NOISE_BAND_MAX, n_cavebiome_.GetNoise((float)wx, (float)wy, (float)wz), technosim_biome::CAVE_NOISE_BAND_COUNT));
                                const bool fungal = technosim_biome::SURFACE_TOP[cbi] == DIRT;
                                const bool lush = technosim_biome::SURFACE_TOP[cbi] == GRASS;
                                const int pool = technosim_biome::SURFACE_POOL[cbi];
                                bool pooled = false;
                                if (pool >= 0 && n_pool_.GetNoise((float)wx, (float)wz) > POOL_NOISE) {
                                    // a pool only where the floor is level: no neighbour at this height may
                                    // open onto a drop, or the static liquid would hang over the edge
                                    pooled = true;
                                    static const int PX[4] = {1, -1, 0, 0}, PZ[4] = {0, 0, 1, -1};
                                    for (int k = 0; k < 4 && pooled; k++)
                                        if (is_cave(wx + PX[k], wy, wz + PZ[k], h) && is_cave(wx + PX[k], wy - 1, wz + PZ[k], h)) pooled = false;
                                }
                                if (pooled) m = (uint16_t)pool;   // static pool, one voxel deep
                                else if (ch < 25) m = STALAGMITE;
                                else if (ch < 85) m = PEBBLES;
                                else if (ch < (fungal ? 230 : 105)) m = (fungal && (ch & 1)) ? MUSHROOM_RED : MUSHROOM_BROWN;
                                else if (lush && ch < 420) m = MOSS;                                   // moss carpets on lush cave floors
                                else if (depth > 40 && ch >= (fungal ? 955 : 975)) m = GLOW_MUSHROOM;  // glowing fungi deep down
                            } else if (!cave[y + 2] && ch < 40) {
                                m = STALACTITE;
                            }
                        }
                    } else if (d_top > 1 && n_soil * 2 < n_solid) {
                        if (d_top <= sandstone_depth) m = strata ? strata_at(wx, wy, wz) : SANDSTONE;
                        else m = vein_ht > 0.0f ? vein_ore(vein, vein_ht, wx, wy, wz) : STONE;
                        if (spike > 0 && wy >= h - spike) m = spire_m == TERRACOTTA ? strata_at(wx, wy, wz) : spire_m;
                        if (do_caves && cave[y + 2] && m == STONE) {
                            // cave floor: the 3D biome decides what you stand on
                            const int di = technosim_biome::band_of(technosim_biome::DEPTH_BAND_MAX, (float)depth, technosim_biome::DEPTH_BAND_COUNT);
                            const int ni = technosim_biome::band_of(technosim_biome::CAVE_NOISE_BAND_MAX, n_cavebiome_.GetNoise((float)wx, (float)wy, (float)wz), technosim_biome::CAVE_NOISE_BAND_COUNT);
                            const int cb = depth < technosim_biome::SOIL_DEPTH ? col.biome : technosim_biome::cave_biome(ti, hi, di, ni);
                            m = (uint16_t)technosim_biome::SURFACE_TOP[cb];
                        } else if (do_caves && cave[y] && m == STONE && depth >= technosim_biome::SOIL_DEPTH) {
                            // cave ceiling: some cave biomes crust it over (sulfur caves)
                            const int di = technosim_biome::band_of(technosim_biome::DEPTH_BAND_MAX, (float)depth, technosim_biome::DEPTH_BAND_COUNT);
                            const int ni = technosim_biome::band_of(technosim_biome::CAVE_NOISE_BAND_MAX, n_cavebiome_.GetNoise((float)wx, (float)wy, (float)wz), technosim_biome::CAVE_NOISE_BAND_COUNT);
                            const int ceil_m = technosim_biome::SURFACE_CEILING[technosim_biome::cave_biome(ti, hi, di, ni)];
                            if (ceil_m >= 0) m = (uint16_t)ceil_m;
                        }
                    } else {
                        m = d_top == 1 ? (far_canopy ? far_leaves : top_m) : under_m;
                        if (m == TERRACOTTA) m = strata_at(wx, wy, wz);
                    }
                } else if ((water_level - wy) * 2 >= step) {
                    // water where at least half the cell is below the water level (lod 0:
                    // wy < water_level); a coarse cell no longer lifts a lake by step - 1 voxels.
                    // Generated water is static (does not drain down slopes) except the sea;
                    // cold lakes and marshes freeze over.
                    const bool inland = hy.lake || hy.river > 0.0f || hy.marsh;
                    if (wy + step >= water_level && cold && (hy.lake || hy.marsh)) m = ICE;
                    else if (wy + step >= water_level && !inland && technosim_biome::SURFACE_ICE[col.biome] >= 0) m = (uint16_t)technosim_biome::SURFACE_ICE[col.biome];   // frozen sea
                    else m = inland ? WATER_STATIC : WATER;
                } else if (hy.fall_top > wy) {
                    m = WATER_STATIC; // waterfall sheet hanging from the lip
                } else if (do_veg && hy.puddle && h < water_level && wy == water_level && !cold && (hash3(wx, wz, 53) & 7) < 2) {
                    m = LILY_PAD; // floats on the puddle's water voxel
                } else if (veg != AIR && wy < h + veg_h) {
                    m = veg;
                }
                colp[y] = m;
                if (m != AIR) any = true;
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

// ---------- debug ----------

Vector3i WorldGen::find_nearest(const String &kind, int x, int z, int max_radius) const {
    Vector3i best(0, -1, 0);
    long best_d2 = -1;
    int cave_kind = -1;   // a named cave biome ("sulfur_cave", "lush_cave", ...); "cave" and "deep_cave" are plain rooms
    for (int b = 0; b < technosim_biome::COUNT; b++)
        if (kind == technosim_biome::NAMES[b] && kind.ends_with("_cave") && kind != "deep_cave") cave_kind = b;
    if (kind == "ruin") {
        const int cx0 = (int)std::floor((float)x / RUIN_CELL), cz0 = (int)std::floor((float)z / RUIN_CELL);
        const int rings = max_radius / RUIN_CELL + 1;
        for (int ring = 0; ring <= rings; ring++) {
            for (int dz = -ring; dz <= ring; dz++)
                for (int dx = -ring; dx <= ring; dx++) {
                    if (std::max(std::abs(dx), std::abs(dz)) != ring) continue;
                    Ruin r;
                    if (!ruin_at_cell(cx0 + dx, cz0 + dz, r)) continue;
                    long d2 = (long)(r.x - x) * (r.x - x) + (long)(r.z - z) * (r.z - z);
                    if (best_d2 < 0 || d2 < best_d2) { best_d2 = d2; best = Vector3i(r.x, r.base, r.z); }
                }
            if (best_d2 >= 0) break;
        }
        return best;
    }
    // Ore veins: "<name>_vein" -> the nearest vein centre of that type under dry land
    // (above sea level), at ground height.
    if (kind.ends_with("_vein")) {
        const String vname = kind.substr(0, kind.length() - 5);
        int vt = -1;
        for (int t = 0; t < technosim_ore::COUNT; t++)
            if (vname == technosim_ore::NAMES[t]) vt = t;
        if (vt < 0) return best;
        const int C = technosim_ore::CELL;
        const int cx0 = floor_div(x, C), cz0 = floor_div(z, C);
        const int rings = max_radius / C + 1;
        for (int ring = 0; ring <= rings; ring++) {
            for (int dz = -ring; dz <= ring; dz++)
                for (int dx = -ring; dx <= ring; dx++) {
                    if (std::max(std::abs(dx), std::abs(dz)) != ring) continue;
                    Vein v;
                    if (!vein_at_cell(cx0 + dx, cz0 + dz, v) || v.type != vt) continue;
                    long d2 = (long)(v.x - x) * (v.x - x) + (long)(v.z - z) * (v.z - z);
                    if (best_d2 >= 0 && d2 >= best_d2) continue;
                    const int gy = height_at(v.x, v.z);
                    if (gy <= sea_) continue;
                    best_d2 = d2;
                    best = Vector3i(v.x, gy, v.z);
                }
            if (best_d2 >= 0 && ring >= 1) break; // one more ring: a nearer centre can sit in the next ring
        }
        return best;
    }
    // Biome names are accepted as kinds too ("desert", "forest", ...).
    int biome_kind = -1;
    for (int b = 0; b < BIOME_COUNT; b++)
        if (kind == biome_name(b)) biome_kind = b;
    // Hydrology features: scan cells in a growing square.
    const int cx0 = (int)std::floor((float)x / HCELL), cz0 = (int)std::floor((float)z / HCELL);
    const int rings = max_radius / HCELL + 1;
    for (int ring = 0; ring <= rings; ring++) {
        for (int dz = -ring; dz <= ring; dz++)
            for (int dx = -ring; dx <= ring; dx++) {
                if (std::max(std::abs(dx), std::abs(dz)) != ring) continue;
                int px = (cx0 + dx) * HCELL + HCELL / 2, pz = (cz0 + dz) * HCELL + HCELL / 2;
                Column c = column_base(px, pz, false);
                Hydro hy = hydro_at(px, pz, c.height);
                bool hit = false;
                int py = c.height;
                if (kind == "waterfall") {
                    // The sheet sits just past the lip of a steep stream segment, not at a cell centre.
                    HydroCtx hc;
                    if (!hydro_ctx(px, pz, hc)) continue;
                    const HydroTile *t = hc.t;
                    const int k = hc.ci;
                    if (t->dir[k] < 0 || t->lake[k] || hc.acc(k) < STREAM_ACC) continue;
                    const int nk = (k / HWIN + D8Z[t->dir[k]]) * HWIN + (k % HWIN + D8X[t->dir[k]]);
                    if (t->fall_t0[k] < 0.0f) continue;
                    const int wx0 = (t->tx * HTILE - HMARGIN) * HCELL, wz0 = (t->tz * HTILE - HMARGIN) * HCELL;
                    const float ax = (float)(wx0 + (k % HWIN) * HCELL + HCELL / 2) + t->jx[k];
                    const float az = (float)(wz0 + (k / HWIN) * HCELL + HCELL / 2) + t->jz[k];
                    const float bx = (float)(wx0 + (nk % HWIN) * HCELL + HCELL / 2) + t->jx[nk];
                    const float bz = (float)(wz0 + (nk / HWIN) * HCELL + HCELL / 2) + t->jz[nk];
                    const float f = t->fall_t1[k];
                    px = (int)std::lround(ax + (bx - ax) * f);
                    pz = (int)std::lround(az + (bz - az) * f);
                    hit = true;
                    py = column_base(px, pz, false).height;
                }
                else if (kind == "cave" || kind == "deep_cave") {
                    // a room at least 5 voxels tall with a solid floor, 10..120 (deep: 50..170) voxels under dry land
                    const int from = kind == "cave" ? 10 : 50, to = kind == "cave" ? 120 : 170;
                    if (c.height > sea_ + 4 && !hy.lake && hy.river < 0.05f) {
                        int run = 0;
                        for (int y = c.height - from; y >= std::max(12, c.height - to); y--) {
                            if (is_cave(px, y, pz, c.height)) { run++; continue; }
                            if (run >= 5) { hit = true; py = y + 1; break; } // y is the floor, y+1 the first air voxel
                            run = 0;
                        }
                    }
                }
                else if (cave_kind >= 0) {
                    // a cave room (5+ voxels tall) whose 3D biome at the floor is the one asked for
                    if (c.height > sea_ + 4 && !hy.lake && hy.river < 0.05f) {
                        int run = 0;
                        for (int y = c.height - technosim_biome::SOIL_DEPTH; y >= std::max(12, c.height - 260); y--) {
                            if (is_cave(px, y, pz, c.height)) { run++; continue; }
                            if (run >= 5 && technosim_biome::classify_3d(c.biome, c.t, c.h, c.height - (y + 1), n_cavebiome_.GetNoise((float)px, (float)(y + 1), (float)pz)) == cave_kind) {
                                hit = true; py = y + 1; break;
                            }
                            run = 0;
                        }
                    }
                }
                else if (kind == "lake") { hit = hy.lake; py = hy.water_level; }
                else if (kind == "marsh") { hit = hy.marsh; }
                else if (kind == "spring") { hit = hy.spring; py = hy.bed; }
                else if (kind == "river") { hit = hy.river > 0.5f && !hy.spring; py = hy.bed; }
                else if (kind == "alpine") { hit = c.alpine && !c.snow; }
                else if (kind == "snow") { hit = c.snow; }
                else if (kind == "mountain") { hit = c.height > sea_ + 200; }
                else if (biome_kind >= 0) {
                    // a real patch of that biome, not one dithered cell: 4 samples 24 voxels out agree
                    hit = c.biome == biome_kind && !hy.lake && hy.river < 0.05f;
                    for (int q = 0; q < 4 && hit; q++)
                        hit = column_base(px + (q & 1 ? 24 : -24), pz + (q & 2 ? 24 : -24), false).biome == biome_kind;
                }
                if (!hit) continue;
                long d2 = (long)dx * dx + (long)dz * dz;
                if (best_d2 < 0 || d2 < best_d2) { best_d2 = d2; best = Vector3i(px, py, pz); }
            }
        if (best_d2 >= 0) break;
    }
    return best;
}

Array WorldGen::waterfalls_near(int x, int z, int radius) const {
    Array out;
    const int cx0 = (int)std::floor((float)x / HCELL), cz0 = (int)std::floor((float)z / HCELL);
    const int r = radius / HCELL + 1;
    for (int dz = -r; dz <= r; dz++)
        for (int dx = -r; dx <= r; dx++) {
            const int px = (cx0 + dx) * HCELL + HCELL / 2, pz = (cz0 + dz) * HCELL + HCELL / 2;
            HydroCtx hc;
            if (!hydro_ctx(px, pz, hc)) continue;
            const HydroTile *t = hc.t;
            const int k = hc.ci;
            if (t->dir[k] < 0 || t->lake[k] || hc.acc(k) < STREAM_ACC || t->fall_t0[k] < 0.0f) continue;
            const int nk = (k / HWIN + D8Z[t->dir[k]]) * HWIN + (k % HWIN + D8X[t->dir[k]]);
            const int wx0 = (t->tx * HTILE - HMARGIN) * HCELL, wz0 = (t->tz * HTILE - HMARGIN) * HCELL;
            const float ax = (float)(wx0 + (k % HWIN) * HCELL + HCELL / 2) + t->jx[k];
            const float az = (float)(wz0 + (k / HWIN) * HCELL + HCELL / 2) + t->jz[k];
            const float bx = (float)(wx0 + (nk % HWIN) * HCELL + HCELL / 2) + t->jx[nk];
            const float bz = (float)(wz0 + (nk / HWIN) * HCELL + HCELL / 2) + t->jz[nk];
            const float f = std::min(1.0f, t->fall_t1[k] + 0.5f / 12.0f); // half a step past the cliff foot
            const int fx = (int)std::lround(ax + (bx - ax) * f), fz = (int)std::lround(az + (bz - az) * f);
            if ((long)(fx - x) * (fx - x) + (long)(fz - z) * (fz - z) > (long)radius * radius) continue;
            const Column c = column_base(fx, fz, false);
            const Hydro hy = hydro_at(fx, fz, c.height);
            const int foot = hy.river > 0.0f ? std::max(hy.bed, hy.water_level) : c.height;
            const float len = std::max(1.0f, std::sqrt((bx - ax) * (bx - ax) + (bz - az) * (bz - az)));
            Dictionary d;
            d["x"] = fx; d["y"] = foot; d["z"] = fz;
            d["top"] = (int)t->fall_top[k];
            d["dir_x"] = (bx - ax) / len; d["dir_z"] = (bz - az) / len;
            // sheet width in voxels: the river's half width as hydro_at computes it, sheet = 60 % of it each side
            d["width"] = 1.2f * river_half_width(std::sqrt(hc.acc(k) / STREAM_ACC));
            out.push_back(d);
        }
    return out;
}

PackedByteArray WorldGen::map_image(int center_x, int center_z, int radius, int step) const {
    const int n = std::max(1, 2 * radius / std::max(1, step));
    PackedByteArray img;
    img.resize(n * n * 3);
    uint8_t *p = img.ptrw();
    std::vector<Ruin> ruins(1024);
    const int nr = ruins_near(center_x - radius, center_z - radius, center_x + radius, center_z + radius, ruins.data(), 1024);
    for (int j = 0; j < n; j++)
        for (int i = 0; i < n; i++) {
            const int wx = center_x - radius + i * step, wz = center_z - radius + j * step;
            Column c = column_base(wx, wz, false);
            Hydro hy = hydro_at(wx, wz, c.height);
            int biome = c.biome;
            if (hy.lake && c.height < hy.water_level) biome = BIOME_LAKE;
            else if (hy.river > 0.3f) biome = BIOME_RIVER;
            float shade = 0.6f + 0.4f * std::clamp((float)(c.height - sea_) / 400.0f, -0.5f, 1.0f);
            if (c.height < sea_) shade = 0.5f + 0.5f * std::clamp((float)(c.height - sea_ + 80) / 80.0f, 0.0f, 1.0f);
            if (biome == BIOME_RIVER || biome == BIOME_LAKE) shade = 1.0f;
            const unsigned char *col = technosim_biome::COLORS[biome];
            uint8_t *q = p + (j * n + i) * 3;
            q[0] = (uint8_t)std::clamp(col[0] * shade, 0.0f, 255.0f);
            q[1] = (uint8_t)std::clamp(col[1] * shade, 0.0f, 255.0f);
            q[2] = (uint8_t)std::clamp(col[2] * shade, 0.0f, 255.0f);
            if (hy.marsh && biome != BIOME_RIVER) { q[0] = 70; q[1] = 110; q[2] = 60; } // dark olive
            if (hy.spring) { q[0] = 255; q[1] = 255; q[2] = 0; }
            if (hy.fall_top >= 0) { q[0] = 255; q[1] = 255; q[2] = 255; }
            for (int k = 0; k < nr; k++)
                if (in_ruin(ruins[k], wx, wz, 1)) { q[0] = 40; q[1] = 40; q[2] = 45; break; }
        }
    return img;
}
