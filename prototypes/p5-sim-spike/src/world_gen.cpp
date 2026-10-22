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
    ClassDB::bind_static_method("WorldGen", D_METHOD("biome_name", "biome"), &WorldGen::biome_name);
    ClassDB::bind_method(D_METHOD("find_nearest", "kind", "x", "z", "max_radius"), &WorldGen::find_nearest);
    ClassDB::bind_method(D_METHOD("waterfalls_near", "x", "z", "radius"), &WorldGen::waterfalls_near);
    ClassDB::bind_method(D_METHOD("hydrology_stats", "x", "z"), &WorldGen::hydrology_stats);
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
    setup(n_ore_, seed_ + 10, 0.09f, 1);
    setup(n_beach_, seed_ + 11, 0.01f, 1);
    setup(n_jitter_, seed_ + 13, 0.02f, 2);
    std::lock_guard<std::mutex> lk(hydro_mutex_);
    hydro_cache_.clear();
}

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

    static const float cx[] = {-1.0f, -0.6f, -0.25f, -0.08f, 0.05f, 0.3f, 0.55f, 0.8f, 1.0f};
    static const float cy[] = {-80.0f, -50.0f, -12.0f, -2.0f, 4.0f, 24.0f, 80.0f, 180.0f, 240.0f};
    float base = spline(col.c, cx, cy, 9);
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
    d["lake"] = hy.lake;
    d["spring"] = hy.spring;
    d["marsh"] = hy.marsh;
    d["puddle"] = hy.puddle;
    d["shore"] = hy.shore;
    d["fall_top"] = hy.fall_top;
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
        const float acc = c.acc(best_k);
        const float strength = std::sqrt(acc / STREAM_ACC);           // 1 at a spring .. ~10 for big rivers
        const float half_w = std::clamp(1.2f + 1.3f * strength, 1.5f, 18.0f);
        const float depth = std::clamp(1.0f + 0.6f * strength, 1.0f, 7.0f);
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
            out.bed = (int)std::floor(surf) - 1 - (int)std::lround(depth * (1.0f - smoothstep(0.0f, half_w, best_d)));
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
    out.trunk_h = 8 + (int)((hsh >> 16) % 6);
    out.radius = 2 + (int)((hsh >> 20) & 1);
    if (above > tree_line - 60) { out.trunk_h = 5 + (int)((hsh >> 16) % 3); out.radius = 2; }
    return true;
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

inline uint16_t WorldGen::ore_at(int x, int y, int z) const {
    float o = n_ore_.GetNoise((float)x, (float)y, (float)z);
    if (o < 0.66f) return STONE;
    if (y > sea_ - 160 && o < 0.74f) return COAL_ORE;
    if (y < sea_ + 60 && o >= 0.74f && o < 0.80f) return IRON_ORE;
    if (y < sea_ - 20 && o >= 0.80f && o < 0.85f) return COPPER_ORE;
    if (y < sea_ - 80 && o >= 0.85f) return TIN_ORE;
    return STONE;
}
// Per-column surface vegetation (LOD 0 only): Minecraft-style tufts, flower
// patches, cacti and dead bushes. One hash per column; flowers cluster where a
// 20 m noise is high. Returns AIR for none; height is 1 except for cacti.
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
    // Pebbles on bare rock and gravel.
    if ((top == GRAVEL_STATIC || top == STONE) && r < 50 && !marsh) return PEBBLES;
    if (top != GRASS && !(marsh && top == DIRT)) return AIR;
    int grass = 0, flower = 0, bush = 0, shroom = 0, pebble = 0;
    if (marsh) grass = 560;
    else switch (col.biome) {
        case PLAINS: grass = 300; flower = 36; pebble = 4; break;
        case FOREST: grass = 170; flower = 14; shroom = 10; break;
        case OLD_GROWTH_FOREST: grass = 200; flower = 10; shroom = 22; break;
        case HIGHLANDS: grass = 110; flower = 10; pebble = 20; break;
        case MOUNTAINS: grass = 50; flower = 4; pebble = 30; break;
        case ALPINE: grass = 25; flower = 6; pebble = 30; break;
        case TAIGA: grass = 60; shroom = 12; pebble = 8; break;
        case SHRUBLAND: grass = 120; bush = 40; pebble = 14; break;
        case SAVANNA: grass = 420; bush = 12; break;
        case JUNGLE: grass = 350; flower = 20; shroom = 14; break;
        case SWAMP: grass = 450; shroom = 18; break;
        default: break;
    }
    if (bush > 0 && r >= 1000 - bush) return DEAD_BUSH;
    if (shroom > 0 && r >= 1000 - bush - shroom) {
        // red caps in the wet, dark biomes, brown elsewhere
        const bool red = (col.biome == SWAMP || col.biome == OLD_GROWTH_FOREST || col.biome == JUNGLE) && ((v >> 14) & 3) == 0;
        return red ? MUSHROOM_RED : MUSHROOM_BROWN;
    }
    if (pebble > 0 && r >= 1000 - bush - shroom - pebble) return PEBBLES;
    if (flower > 0) {
        const float patch = n_detail_.GetNoise((float)x * 2.0f + 5000.0f, (float)z * 2.0f - 5000.0f);
        flower = patch > 0.3f ? flower * 5 : flower / 4;
    }
    if (r < grass) return TALL_GRASS;
    if (r < grass + flower) return ((v >> 12) & 1) ? FLOWER : FLOWER_RED;
    return AIR;
}

// ---------- blocks ----------

// Soil voxels per column counted from the surface (depth 1 is the top material,
// 2..SOIL_VOXELS the biome's under material, deeper is rock, sandstone or ore).
static const int SOIL_VOXELS = 4;
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
    int ntrees = 0, nruins = 0;
    if (above_sea) {
        nruins = ruins_near(origin.x, origin.z, origin.x + sx * step, origin.z + sz * step, ruins, 4);
        if (lod <= 2) ntrees = trees_near(origin.x, origin.z, origin.x + sx * step, origin.z + sz * step, trees, 256);
    }
    const bool far_trees = lod >= 3 && above_sea;
    for (int z = 0; z < sz; z++) {
        const int wz = origin.z + z * step;
        for (int x = 0; x < sx; x++) {
            const int wx = origin.x + x * step;
            const Column col = column(wx, wz);
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
                top_m = under_m = STONE;       // bare rock face
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
            // Sedimentary layer under deserts, badlands and beaches (no ores in it).
            const int sandstone_depth = (col.biome == BIOME_DESERT || col.biome == technosim_biome::BADLANDS) ? 16
                : ((col.biome == BIOME_BEACH || col.biome == technosim_biome::SNOWY_BEACH) ? 8 : 0);
            // Cave biome bands for this column (3D layer): floors of carved caves get the
            // cave biome's top material (lush: grass, fungal: dirt, frozen: snow, ...).
            const int ti = technosim_biome::band_of(technosim_biome::TEMP_BAND_MAX, col.t);
            const int hi = technosim_biome::band_of(technosim_biome::HUM_BAND_MAX, col.h);
            // Vegetation standing on the surface (not in water, not on the sheet).
            uint16_t veg = AIR;
            int veg_h = 0;
            if (do_veg && !ruin && h >= water_level && hy.fall_top < 0 && h > sea_)
                veg = vegetation_at(col, wx, wz, top_m, hy, veg_h);
            uint16_t *colp = w + sy * (x + sx * z);
            // Far LOD: is this column under a tree's canopy? (one tree cell lookup)
            bool far_canopy = false;
            if (far_trees && !ruin && h > water_level && technosim_biome::TREE_DENSITY[col.biome] > 0.0f) {
                Tree t;
                const int tcx = (int)std::floor((float)wx / TREE_CELL), tcz = (int)std::floor((float)wz / TREE_CELL);
                if (tree_at_cell(tcx, tcz, t)) {
                    const int dx = wx - t.x, dz = wz - t.z;
                    far_canopy = dx * dx + dz * dz <= t.radius * t.radius + 1;
                }
            }
            // Trees touching this column
            int trunk_lo = 0, trunk_hi = -1;
            int can_lo = 0, can_hi = -1, can_base = 0, can_r = 0, can_d2 = 0;
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
                const int top = trees[t].base + trees[t].trunk_h;
                if (trunk_here) { trunk_lo = trees[t].base; trunk_hi = top + 1; veg = AIR; }
                if (can_hi < 0 || d2 < can_d2) {
                    can_base = top - 2; can_lo = can_base; can_hi = top + 2; can_r = trees[t].radius; can_d2 = d2;
                }
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
                    m = WOOD;
                } else if (can_hi >= 0 && wy >= can_lo && wy <= can_hi && wy >= h) {
                    const int k = wy - can_base;
                    const int lr = (k == 0 || k == 3) ? can_r - 1 : (k == 4 ? can_r - 2 : can_r);
                    if (lr >= 0 && can_d2 <= lr * lr + (lr > 1 ? 1 : 0)) m = LEAVES;
                }
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
                                if (ch < 25) m = STALAGMITE;
                                else if (ch < 85) m = PEBBLES;
                                else if (ch < (fungal ? 230 : 105)) m = (fungal && (ch & 1)) ? MUSHROOM_RED : MUSHROOM_BROWN;
                                else if (lush && ch < 420) m = MOSS;                                   // moss carpets on lush cave floors
                                else if (depth > 40 && ch >= (fungal ? 955 : 975)) m = GLOW_MUSHROOM;  // glowing fungi deep down
                            } else if (!cave[y + 2] && ch < 40) {
                                m = STALACTITE;
                            }
                        }
                    } else if (d_top > 1 && n_soil * 2 < n_solid) {
                        if (d_top <= sandstone_depth) m = SANDSTONE;
                        else m = do_ores ? ore_at(wx, wy, wz) : STONE;
                        if (do_caves && cave[y + 2] && m == STONE) {
                            // cave floor: the 3D biome decides what you stand on
                            const int di = technosim_biome::band_of(technosim_biome::DEPTH_BAND_MAX, (float)depth, technosim_biome::DEPTH_BAND_COUNT);
                            const int ni = technosim_biome::band_of(technosim_biome::CAVE_NOISE_BAND_MAX, n_cavebiome_.GetNoise((float)wx, (float)wy, (float)wz), technosim_biome::CAVE_NOISE_BAND_COUNT);
                            const int cb = depth < technosim_biome::SOIL_DEPTH ? col.biome : technosim_biome::cave_biome(ti, hi, di, ni);
                            m = (uint16_t)technosim_biome::SURFACE_TOP[cb];
                        }
                    } else {
                        m = d_top == 1 ? (far_canopy ? LEAVES : top_m) : under_m;
                    }
                } else if ((water_level - wy) * 2 >= step) {
                    // water where at least half the cell is below the water level (lod 0:
                    // wy < water_level); a coarse cell no longer lifts a lake by step - 1 voxels.
                    // Generated water is static (does not drain down slopes) except the sea;
                    // cold lakes and marshes freeze over.
                    if (wy + step >= water_level && cold && (hy.lake || hy.marsh)) m = ICE;
                    else m = (hy.lake || hy.river > 0.0f || hy.marsh) ? WATER_STATIC : WATER;
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
