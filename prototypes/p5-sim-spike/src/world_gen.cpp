#include "world_gen.h"

#include <godot_cpp/core/class_db.hpp>

#include <algorithm>
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
    col.t = n_temp_.GetNoise(fx, fz) + jit * 0.09f;
    col.h = n_hum_.GetNoise(fx, fz) + jit2 * 0.09f;

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
    // Altitude lines (voxels above sea): tree line ~ 220 (110 m), snow line ~ 330
    // (165 m), both lowered where it is cold and raised where it is warm.
    const float cold = -col.t; // -1 warm .. 1 cold
    const int tree_line = 220 + hj * 2 - (int)(cold * 60.0f);
    const int snow_line = 330 + hj * 2 - (int)(cold * 90.0f);
    col.alpine = above > tree_line;
    col.snow = above > snow_line;
    if (with_slope && above > 120) {
        // Slope only matters on mountains; two extra samples keep the cost local.
        Column cxp = column_base(x + 2, z, false);
        Column czp = column_base(x, z + 2, false);
        col.slope = std::max(std::fabs((float)(cxp.height - col.height)), std::fabs((float)(czp.height - col.height))) * 0.5f;
    }

    if (col.c < -0.55f) col.biome = BIOME_DEEP_OCEAN;
    else if (col.height < sea_ - 1) col.biome = BIOME_OCEAN;
    else if (above <= 3 + (hj > 0 ? hj / 3 : 0)) col.biome = BIOME_BEACH;
    else if (col.snow) col.biome = BIOME_SNOWY_PEAKS;
    else if (col.alpine) col.biome = BIOME_ALPINE;
    else if (above > 170 + hj) col.biome = BIOME_MOUNTAINS;
    else if (above > 70 + hj) col.biome = BIOME_HIGHLANDS;
    else if (col.t > 0.35f && col.h < -0.1f) col.biome = BIOME_DESERT;
    else if (col.t < -0.45f) col.biome = BIOME_TUNDRA;
    else if (col.h > 0.15f) col.biome = BIOME_FOREST;
    else col.biome = BIOME_PLAINS;
    return col;
}

WorldGen::Column WorldGen::column(int x, int z) const { return column_base(x, z, true); }

int WorldGen::height_at(int x, int z) const {
    Column c = column_base(x, z, false);
    Hydro hy = hydro_at(x, z, c.height);
    if (hy.river > 0.0f) return std::min(c.height, hy.bed);
    return c.height;
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
    d["humidity"] = c.h;
    d["slope"] = c.slope;
    d["alpine"] = c.alpine;
    d["snow"] = c.snow;
    d["river"] = hy.river;
    d["water_level"] = hy.water_level;
    d["lake"] = hy.lake;
    d["spring"] = hy.spring;
    return d;
}

String WorldGen::biome_name(int b) {
    static const char *names[] = {"deep_ocean", "ocean", "beach", "plains", "forest", "desert", "tundra", "highlands", "mountains", "snowy_peaks", "river", "lake", "alpine"};
    if (b < 0 || b >= BIOME_COUNT) return "unknown";
    return names[b];
}

// ---------- hydrology ----------

static const int D8X[8] = {1, 1, 0, -1, -1, -1, 0, 1};
static const int D8Z[8] = {0, 1, 1, 1, 0, -1, -1, -1};

std::shared_ptr<WorldGen::HydroTile> WorldGen::hydro_tile(int tx, int tz) const {
    const uint64_t key = ((uint64_t)(uint32_t)tx << 32) | (uint32_t)tz;
    std::lock_guard<std::mutex> lk(hydro_mutex_);
    auto it = hydro_cache_.find(key);
    if (it != hydro_cache_.end()) return it->second;
    auto t = std::make_shared<HydroTile>();
    t->tx = tx;
    t->tz = tz;
    build_hydro_tile(*t);
    if (hydro_cache_.size() > 48) hydro_cache_.clear(); // crude bound; tiles are cheap to rebuild
    hydro_cache_[key] = t;
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
        t.lake[k] = (t.filled[k] - t.h[k] > 3.0f && t.h[k] >= (float)sea_) ? 1 : 0; // puddles < 1.5 m deep are not lakes
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
}

bool WorldGen::hydro_cell(int x, int z, const HydroTile *&tile, int &ci) const {
    // Which tile owns this column, and the window cell index inside it.
    const int cx = (int)std::floor((float)x / HCELL), cz = (int)std::floor((float)z / HCELL);
    const int tx = (int)std::floor((float)cx / HTILE), tz = (int)std::floor((float)cz / HTILE);
    thread_local std::shared_ptr<HydroTile> cached;
    if (!cached || cached->tx != tx || cached->tz != tz) cached = hydro_tile(tx, tz);
    tile = cached.get();
    const int i = cx - (tx * HTILE - HMARGIN), j = cz - (tz * HTILE - HMARGIN);
    if (i < 1 || j < 1 || i >= HWIN - 1 || j >= HWIN - 1) return false;
    ci = j * HWIN + i;
    return true;
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
    Hydro out{0.0f, 0, -1, false, false};
    const HydroTile *t = nullptr;
    int ci = 0;
    if (!hydro_cell(x, z, t, ci)) return out;
    const int i = ci % HWIN, j = ci / HWIN;
    const int wx0 = (t->tx * HTILE - HMARGIN) * HCELL;
    const int wz0 = (t->tz * HTILE - HMARGIN) * HCELL;
    auto centre_x = [&](int k) { return (float)(wx0 + (k % HWIN) * HCELL + HCELL / 2) + t->jx[k]; };
    auto centre_z = [&](int k) { return (float)(wz0 + (k / HWIN) * HCELL + HCELL / 2) + t->jz[k]; };
    // Lakes: the filled level of this cell or a neighbouring lake cell.
    float lake_level = -1.0f;
    for (int dj = -1; dj <= 1; dj++)
        for (int di = -1; di <= 1; di++) {
            const int k = (j + dj) * HWIN + (i + di);
            if (t->lake[k]) lake_level = std::max(lake_level, t->filled[k]);
        }
    if (lake_level > 0.0f && (float)base_height < lake_level - 0.5f) {
        out.lake = true;
        out.water_level = (int)std::floor(lake_level);
    }
    // Rivers: segments from each nearby cell with enough drainage to its downstream cell.
    float best_d = 1e9f;
    int best_k = -1;
    float best_t = 0.0f;
    for (int dj = -1; dj <= 1; dj++)
        for (int di = -1; di <= 1; di++) {
            const int k = (j + dj) * HWIN + (i + di);
            if (t->acc[k] < STREAM_ACC || t->dir[k] < 0 || t->lake[k]) continue;
            const int nk = (k / HWIN + D8Z[t->dir[k]]) * HWIN + (k % HWIN + D8X[t->dir[k]]);
            float tt;
            float d = seg_dist((float)x, (float)z, centre_x(k), centre_z(k), centre_x(nk), centre_z(nk), tt);
            if (d < best_d) { best_d = d; best_k = k; best_t = tt; }
        }
    if (best_k >= 0) {
        const float acc = t->acc[best_k];
        const float strength = std::sqrt(acc / STREAM_ACC);           // 1 at a spring .. ~10 for big rivers
        const float half_w = std::clamp(1.2f + 1.3f * strength, 1.5f, 18.0f);
        const float depth = std::clamp(1.0f + 0.6f * strength, 1.0f, 7.0f);
        const float bank_w = half_w * 1.5f + 6.0f; // gentle banks: valley floor widens with the river
        if (best_d < half_w + bank_w) {
            const int nk = (best_k / HWIN + D8Z[t->dir[best_k]]) * HWIN + (best_k % HWIN + D8X[t->dir[best_k]]);
            float surf = t->filled[best_k] + (t->filled[nk] - t->filled[best_k]) * best_t;
            surf = std::min(surf, (float)base_height); // never above the local ground
            const float mask = 1.0f - smoothstep(half_w, half_w + bank_w, best_d);
            out.river = mask;
            const int water = (int)std::floor(surf) - 1;
            out.bed = (int)std::floor(surf) - 1 - (int)std::lround(depth * (1.0f - smoothstep(0.0f, half_w, best_d)));
            if (best_d < half_w && water > out.bed) out.water_level = std::max(out.water_level, water);
            // Springs: first cell of a stream (no upstream stream cells).
            bool upstream = false;
            for (int d = 0; d < 8 && !upstream; d++) {
                const int uk = (best_k / HWIN + D8Z[d]) * HWIN + (best_k % HWIN + D8X[d]);
                upstream = t->dir[uk] >= 0 && t->acc[uk] >= STREAM_ACC && ((uk / HWIN + D8Z[t->dir[uk]]) * HWIN + (uk % HWIN + D8X[t->dir[uk]])) == best_k;
            }
            out.spring = !upstream && best_t < 0.15f;
        }
    }
    return out;
}

Dictionary WorldGen::hydrology_stats(int x, int z) const {
    const HydroTile *t = nullptr;
    int ci = 0;
    Dictionary d;
    if (!hydro_cell(x, z, t, ci)) return d;
    int streams = 0, lakes = 0, springs = 0;
    float max_acc = 0.0f;
    for (int k = 0; k < HWIN * HWIN; k++) {
        if (t->lake[k]) lakes++;
        if (t->acc[k] >= STREAM_ACC && t->dir[k] >= 0) {
            streams++;
            max_acc = std::max(max_acc, t->acc[k]);
        }
    }
    d["tile"] = Vector2i(t->tx, t->tz);
    d["window_cells"] = HWIN * HWIN;
    d["stream_cells"] = streams;
    d["lake_cells"] = lakes;
    d["max_accumulation"] = max_acc;
    d["cell_acc"] = t->acc[ci];
    d["cell_filled"] = t->filled[ci];
    d["cell_h"] = t->h[ci];
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
    if (hy.river > 0.05f || hy.lake) return false;
    float density;
    switch (col.biome) {
        case BIOME_FOREST: density = 0.55f; break;
        case BIOME_PLAINS: density = 0.06f; break;
        case BIOME_HIGHLANDS: density = 0.18f; break;
        case BIOME_TUNDRA: density = 0.08f; break;
        case BIOME_MOUNTAINS: density = 0.12f; break;
        default: return false;
    }
    // Thin out toward the tree line (stunted, sparse).
    const int above = col.height - sea_;
    if (above > 160) density *= std::clamp((220 - above) / 60.0f, 0.1f, 1.0f);
    float roll = (float)((hsh >> 6) & 1023) / 1023.0f;
    if (roll >= density) return false;
    out.x = tx;
    out.z = tz;
    out.base = col.height;
    out.trunk_h = 8 + (int)((hsh >> 16) % 6);
    out.radius = 2 + (int)((hsh >> 20) & 1);
    if (above > 160) { out.trunk_h = 5 + (int)((hsh >> 16) % 3); out.radius = 2; }
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
    if (hy.river > 0.05f || hy.lake) return false;
    if (!(c.biome == BIOME_PLAINS || c.biome == BIOME_FOREST || c.biome == BIOME_HIGHLANDS || c.biome == BIOME_TUNDRA || c.biome == BIOME_DESERT)) return false;
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

// ---------- blocks ----------

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
    const bool do_caves = lod == 0;
    const bool do_ores = lod == 0;
    const int y_top = origin.y + sy * step;
    const bool above_sea = y_top > sea_ && origin.y < world_height_;
    Tree trees[64];
    Ruin ruins[4];
    int ntrees = 0, nruins = 0;
    if (above_sea) {
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
            bool riverbed = false, lakebed = false;
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
            int no_cave_above = -1;
            if (hy.lake || hy.river > 0.0f) no_cave_above = water_level - 40;

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
            uint16_t *colp = w + sy * (x + sx * z);
            // Trees touching this column
            int trunk_lo = 0, trunk_hi = -1;
            int can_lo = 0, can_hi = -1, can_base = 0, can_r = 0, can_d2 = 0;
            for (int t = 0; t < ntrees; t++) {
                const int dx = wx - trees[t].x, dz = wz - trees[t].z;
                const int d2 = dx * dx + dz * dz;
                if (d2 > trees[t].radius * trees[t].radius + 1) continue;
                bool blocked = false;
                for (int i = 0; i < nruins && !blocked; i++) blocked = in_ruin(ruins[i], trees[t].x, trees[t].z, 3);
                if (blocked) continue;
                const int top = trees[t].base + trees[t].trunk_h;
                if (dx == 0 && dz == 0) { trunk_lo = trees[t].base; trunk_hi = top + 1; }
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
                    const int depth = h - wy;
                    const bool cave_ok = no_cave_above < 0 || wy < no_cave_above;
                    if (do_caves && cave_ok && is_cave(wx, wy, wz, h) && !(wy < sea_ && h < sea_ + 2)) {
                        m = (wy < sea_ - 180) ? LAVA : AIR;
                    } else if (depth > 4 + (int)(step > 1)) {
                        m = do_ores ? ore_at(wx, wy, wz) : STONE;
                    } else {
                        const bool top = depth == 1;
                        if (lakebed || riverbed) {
                            m = (hash3(wx, wz, 5) & 3) ? GRAVEL_STATIC : SAND_STATIC;
                        } else if (hy.river > 0.08f && h <= water_level + 2) {
                            m = SAND_STATIC; // river bank
                        } else if (col.snow && !steep) {
                            m = top ? SNOW : STONE; // permanent snow over rock
                        } else if (steep) {
                            m = STONE;              // bare rock face
                        } else if (scree) {
                            m = GRAVEL_STATIC;      // scree slopes below the cliffs
                        } else if (col.alpine) {
                            // alpine zone: rock with patches of thin grass/gravel
                            uint32_t ah = hash3(wx, wz, 19) & 7;
                            m = top ? (ah < 3 ? GRASS : (ah < 5 ? GRAVEL_STATIC : STONE)) : (ah < 3 ? DIRT : STONE);
                        } else switch (col.biome) {
                            case BIOME_DEEP_OCEAN: m = GRAVEL_STATIC; break;
                            case BIOME_OCEAN: m = beach_sand ? SAND_STATIC : GRAVEL_STATIC; break;
                            case BIOME_BEACH: m = SAND_STATIC; break;
                            case BIOME_DESERT: m = SAND_STATIC; break;
                            case BIOME_TUNDRA: m = top ? SNOW : DIRT; break;
                            case BIOME_MOUNTAINS: m = (top && col.slope > 0.6f) ? STONE : (top ? GRASS : DIRT); break;
                            default: m = top ? GRASS : DIRT; break;
                        }
                    }
                } else if (wy < water_level) {
                    // generated water is static (does not drain down slopes) except the sea
                    m = (hy.lake || hy.river > 0.0f) ? WATER_STATIC : WATER;
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
    // Hydrology features: scan cells in a growing square.
    const int cx0 = (int)std::floor((float)x / HCELL), cz0 = (int)std::floor((float)z / HCELL);
    const int rings = max_radius / HCELL + 1;
    for (int ring = 0; ring <= rings; ring++) {
        for (int dz = -ring; dz <= ring; dz++)
            for (int dx = -ring; dx <= ring; dx++) {
                if (std::max(std::abs(dx), std::abs(dz)) != ring) continue;
                const int px = (cx0 + dx) * HCELL + HCELL / 2, pz = (cz0 + dz) * HCELL + HCELL / 2;
                Column c = column_base(px, pz, false);
                Hydro hy = hydro_at(px, pz, c.height);
                bool hit = false;
                int py = c.height;
                if (kind == "lake") { hit = hy.lake; py = hy.water_level; }
                else if (kind == "spring") { hit = hy.spring; py = hy.bed; }
                else if (kind == "river") { hit = hy.river > 0.5f && !hy.spring; py = hy.bed; }
                else if (kind == "alpine") { hit = c.alpine && !c.snow; }
                else if (kind == "snow") { hit = c.snow; }
                else if (kind == "mountain") { hit = c.height > sea_ + 200; }
                if (!hit) continue;
                long d2 = (long)dx * dx + (long)dz * dz;
                if (best_d2 < 0 || d2 < best_d2) { best_d2 = d2; best = Vector3i(px, py, pz); }
            }
        if (best_d2 >= 0) break;
    }
    return best;
}

PackedByteArray WorldGen::map_image(int center_x, int center_z, int radius, int step) const {
    const int n = std::max(1, 2 * radius / std::max(1, step));
    PackedByteArray img;
    img.resize(n * n * 3);
    uint8_t *p = img.ptrw();
    std::vector<Ruin> ruins(1024);
    const int nr = ruins_near(center_x - radius, center_z - radius, center_x + radius, center_z + radius, ruins.data(), 1024);
    static const uint8_t colors[BIOME_COUNT][3] = {
        {20, 40, 120}, {40, 80, 200}, {230, 215, 150}, {110, 190, 80}, {40, 130, 50},
        {225, 200, 110}, {200, 210, 220}, {130, 150, 90}, {120, 120, 125}, {245, 245, 250},
        {70, 140, 230}, {90, 160, 240}, {150, 140, 120},
    };
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
            const uint8_t *col = colors[biome];
            uint8_t *q = p + (j * n + i) * 3;
            q[0] = (uint8_t)std::clamp(col[0] * shade, 0.0f, 255.0f);
            q[1] = (uint8_t)std::clamp(col[1] * shade, 0.0f, 255.0f);
            q[2] = (uint8_t)std::clamp(col[2] * shade, 0.0f, 255.0f);
            if (hy.spring) { q[0] = 255; q[1] = 255; q[2] = 0; }
            for (int k = 0; k < nr; k++)
                if (in_ruin(ruins[k], wx, wz, 1)) { q[0] = 40; q[1] = 40; q[2] = 45; break; }
        }
    return img;
}
