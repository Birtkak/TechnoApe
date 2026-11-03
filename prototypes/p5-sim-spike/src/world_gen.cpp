#include "world_gen.h"

#include <godot_cpp/core/class_db.hpp>

#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstring>
#include <queue>

using namespace godot;

// Every configure / set_variant takes a new epoch from here, unique across generators (tile_cached).
static std::atomic<uint64_t> g_epoch{1};

WorldGen::WorldGen() { configure(1337, 256, 1280); }

void WorldGen::_bind_methods() {
    ClassDB::bind_method(D_METHOD("configure", "seed", "sea_level", "world_height"), &WorldGen::configure);
    ClassDB::bind_method(D_METHOD("get_seed"), &WorldGen::get_seed);
    ClassDB::bind_method(D_METHOD("height_at", "x", "z"), &WorldGen::height_at);
    ClassDB::bind_method(D_METHOD("biome_at", "x", "z"), &WorldGen::biome_at);
    ClassDB::bind_method(D_METHOD("column_info", "x", "z"), &WorldGen::column_info);
    ClassDB::bind_method(D_METHOD("map_image", "center_x", "center_z", "radius", "step"), &WorldGen::map_image);
    ClassDB::bind_method(D_METHOD("climate_map", "x0", "z0", "n", "step"), &WorldGen::climate_map);
    ClassDB::bind_method(D_METHOD("water_floor_map", "x0", "z0", "n", "step"), &WorldGen::water_floor_map);
    ClassDB::bind_static_method("WorldGen", D_METHOD("biome_name", "biome"), &WorldGen::biome_name);
    ClassDB::bind_method(D_METHOD("find_nearest", "kind", "x", "z", "max_radius"), &WorldGen::find_nearest);
    ClassDB::bind_method(D_METHOD("waterfalls_near", "x", "z", "radius"), &WorldGen::waterfalls_near);
    ClassDB::bind_method(D_METHOD("hydrology_stats", "x", "z"), &WorldGen::hydrology_stats);
    ClassDB::bind_method(D_METHOD("vein_at", "x", "z"), &WorldGen::vein_at);
    ClassDB::bind_method(D_METHOD("set_variant", "slot", "value"), &WorldGen::set_variant);
    ClassDB::bind_method(D_METHOD("ruin_pots_near", "x", "z", "radius"), &WorldGen::ruin_pots_near);
    ClassDB::bind_method(D_METHOD("ruin_at", "x", "z"), &WorldGen::ruin_at);
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
    // Landforms (2026-10-23, the user: "huge mountains, plains and small hilly areas"): continents
    // ~8 km across, regions ~3 km across that are plains, hill country or a mountain range, ridged
    // ranges with ~1 km between ridges, hills ~70 m across.
    setup(n_cont_, seed_ + 1, 0.00006f, 4);
    setup(n_eros_, seed_ + 2, 0.0008f, 3);
    setup(n_pv_, seed_ + 3, 0.0005f, 5, FastNoiseLite::FractalType_Ridged);
    setup(n_region_, seed_ + 14, 0.00016f, 2);
    setup(n_hillmask_, seed_ + 15, 0.0004f, 2);
    setup(n_hill_, seed_ + 16, 0.007f, 3);
    setup(n_detail_, seed_ + 4, 0.013f, 2); // ~75 voxel period: fewer, longer steps on flats
    setup(n_temp_, seed_ + 5, 0.0004f, 2);
    setup(n_hum_, seed_ + 6, 0.0005f, 2);
    setup(n_cheese_, seed_ + 7, 0.0075f, 2);   // rooms ~1.5x the old size (ADR 0109)
    setup(n_cavebiome_, seed_ + 13, 1.0f / (technosim_biome::CAVE_NOISE_PERIOD_M * 2.0f), 1); // 3D, slow: deep dark / crystal / magma pockets
    setup(n_spag1_, seed_ + 8, 0.012f, 1);   // tunnels: a bend per ~40 m, so one can be followed (ADR 0109)
    setup(n_spag2_, seed_ + 9, 0.012f, 1);
    setup(n_ore_, seed_ + 10, 0.09f, 1); // ore vein lens edge wobble
    setup(n_jitter_, seed_ + 13, 0.02f, 2);
    setup(n_cavern_, seed_ + 17, 1.0f / 300.0f, 2);   // ~150 m between halls
    setup(n_ravine_, seed_ + 19, 1.0f / 900.0f, 1);   // ravine lines (|noise| near 0)
    setup(n_ravmask_, seed_ + 20, 1.0f / 1400.0f, 1); // which stretches of those lines are open
    setup(n_pillar_, seed_ + 21, 1.0f / 22.0f, 1);    // hall pillars ~2-3 m thick, ~11 m apart
    setup(n_gallery_, seed_ + 24, 1.0f / 120.0f, 1);  // gallery routes: a bend per ~30 m, a passage every ~30 m
    setup(n_galoff_, seed_ + 25, 1.0f / 400.0f, 1);   // gallery storeys rise and fall ~5 m over ~100 m
    setup(n_cliff_, seed_ + 21, 1.0f / 3000.0f, 2);  // ~1.5 km stretches of rocky coast
    setup(n_meander_, seed_ + 22, 1.0f / 90.0f, 1);    // lowland river bends (~45 m)
    setup(n_band_, seed_ + 23, 1.0f / 700.0f, 1);      // mountain cliff bands (~350 m patches)
    epoch_.store(g_epoch.fetch_add(1) + 1);
    std::lock_guard<std::mutex> lk(hydro_mutex_);
    hydro_cache_.clear();
}

static inline int floor_div(int a, int b) { return a >= 0 ? a / b : -((-a + b - 1) / b); }

void WorldGen::set_variant(int slot, int value) {
    if (slot < 0 || slot >= VAR_COUNT) return;
    variant_[slot] = value;
    epoch_.store(g_epoch.fetch_add(1) + 1);
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

    // Sea floor (bug report 2026-10-25 20-49-22, "oceans should be deeper"): a short shelf off the beach
    // (-1.5 .. -40 voxels, 20 m at its edge), a continental slope down to the deep-ocean line (c -0.55,
    // ~-160), then an abyssal plain ~110 m below the surface (18 m over the bedrock floor). Land rises
    // slowly inland: coasts and plains lie a few metres above the sea, the heart of a continent ~30 m.
    static const float cx[] = {-1.0f, -0.75f, -0.6f, -0.45f, -0.3f, -0.15f, -0.05f, 0.05f, 0.3f, 0.6f, 1.0f};
    static const float cy[] = {-220.0f, -205.0f, -175.0f, -120.0f, -64.0f, -24.0f, -1.5f, 4.0f, 14.0f, 34.0f, 60.0f};
    float base = spline(col.c, cx, cy, 11);
    // The shoreline (developer 2026-10-27: "fix beaches", "make the ocean floor and the way land meets sea
    // more realistic"). Continentalness has an ~8 km period, so the spline alone left 100-200 m of land
    // within 1.5 m of the sea (all of it beach) and as wide a plate of sea under 1 m of water. Near the coast
    // the base follows the distance to the contour where the spline crosses sea level instead, estimated
    // from the local gradient of c: a beach sloping up to a berm, then land rising gently; at sea a
    // foreshore dropping to ~3 m, a sandbar ~22 m out, then a shelf sloping down until the continental
    // slope of the spline takes over.
    if (col.c > -0.35f && col.c < 0.3f) {
        constexpr float SHORE_C = -0.05f + 1.5f / 55.0f;   // where the spline crosses 0 (between -0.05 and 0.05)
        constexpr float E = 12.0f;
        const float gx = (n_cont_.GetNoise(fx + E, fz) - col.c) / E;
        const float gz = (n_cont_.GetNoise(fx, fz + E) - col.c) / E;
        col.shore = (col.c - SHORE_C) / std::max(std::sqrt(gx * gx + gz * gz), 1e-7f);
        const float s = col.shore;
        if (s >= 0.0f) {
            const float prof = 3.5f * smoothstep(0.0f, 36.0f, s) + 0.01f * s;   // berm 1.75 m up 18 m in, then 0.5 m per 25 m
            base += (prof - base) * (1.0f - smoothstep(250.0f, 700.0f, s));
        } else {
            const float a = -s;
            float prof = a < 70.0f ? -0.09f * a : -6.3f - 0.022f * (a - 70.0f);
            prof += 1.6f * std::exp(-((a - 45.0f) * (a - 45.0f)) / 100.0f);   // the bar the waves break on
            base = std::min(base, prof);
        }
    }
    const float inland = smoothstep(0.0f, 0.25f, col.c);
    // Regions: plains where the region noise is low, hill country in the middle and where the hill
    // mask says so, mountain ranges where it is high. Ranges may reach the coast (fjords, sea cliffs).
    const float region = n_region_.GetNoise(fx, fz);
    float mountainness = smoothstep(0.04f, 0.4f, region) * smoothstep(-0.05f, 0.2f, col.c);
    const float hilly = std::clamp(smoothstep(-0.45f, -0.12f, region) + smoothstep(0.2f, 0.5f, n_hillmask_.GetNoise(fx, fz)), 0.0f, 1.0f) * inland;
    static const float ex[] = {-1.0f, -0.3f, 0.4f, 1.0f};
    static const float ey[] = {1.15f, 1.0f, 0.7f, 0.5f};
    const float relief_scale = spline(col.e, ex, ey, 4);
    // ranges: ridged noise, sharpened so the peaks stand out of broad flanks (up to ~220 m above the sea)
    const float ridge = col.pv * 0.5f + 0.5f;
    const float dn = n_detail_.GetNoise(fx, fz);   // fine relief; also wobbles the cliff bands
    col.dn = dn;
    float relief;
    if (variant_[VAR_MOUNTAINS] == 0) {
        // the ranges of ADR 0035 (sheets only)
        relief = std::pow(mountainness, 1.5f) * (50.0f + 400.0f * std::pow(ridge, 1.7f)) * relief_scale;
    } else {
        // Massif (variant 1, the world since ADR 0051: "a slower gradual increase around a huge foot that
        // becomes more and more steep") and horns (2, sheets only): one broad mask (~3 km) carries a mountain mass whose height grows
        // concavely toward its core; ridges cut valleys into its upper half instead of standing on a plain.
        // The normalised height is then raised to a power: the foot flattens into a wide apron and the top
        // steepens (horns: much more, near-vertical summits).
        const float mw = smoothstep(-0.15f, 0.5f, region) * smoothstep(-0.1f, 0.2f, col.c);
        const bool horns = variant_[VAR_MOUNTAINS] == 2;
        const float hmax = (variant_[VAR_RELIEF] > 0 ? (float)variant_[VAR_RELIEF] : 400.0f) * 2.0f * (horns ? 1.12f : 1.0f);
        const float shape = mw * mw * (0.45f + 0.55f * std::pow(ridge, horns ? 1.8f : 1.3f));
        relief = hmax * std::pow(shape, horns ? 1.9f : 1.3f) * relief_scale;
        mountainness = std::max(mountainness, mw);
        // Arêtes and cliff bands (bug report 2026-10-25 20-58-48: "mountains feel epic now but still are super
        // easy to traverse"): the auto step-up climbed any smooth slope, so the upper mountain gets walls.
        // A knife ridge stands on the crest lines of the ridged noise, and the relief breaks into tiers
        // 8-18 m tall whose risers are near vertical (a riser is 6 % of a tier's span of the smooth height,
        // so it is ~16x as steep as the slope it replaces; on the gentle foot that stays a walkable ledge).
        // Their edges wander with the detail noise, so the bands do not follow contour lines.
        if (mw > 0.05f) {
            relief += mw * mw * smoothstep(0.86f, 0.98f, ridge) * 50.0f * relief_scale;
            const float band = n_band_.GetNoise(fx, fz);
            const float wcl = smoothstep(0.12f, 0.45f, mw) * (0.5f + 0.5f * smoothstep(-0.35f, 0.25f, band));
            if (wcl > 0.0f) {
                const float T = 16.0f + 20.0f * std::clamp(band * 0.5f + 0.5f, 0.0f, 1.0f);   // macro voxels
                const float wob = dn * 7.0f;
                const float q = (relief + wob) / T, k = std::floor(q);
                const float tiered = (k + smoothstep(0.94f, 1.0f, q - k)) * T - wob;
                relief += (tiered - relief) * wcl;
            }
        }
    }
    // hills: rolling 3-11 m humps, taller where the hill country is wide
    const float hn = n_hill_.GetNoise(fx, fz) * 0.5f + 0.5f;
    relief += hilly * (1.0f - mountainness) * std::pow(hn, 1.3f) * (12.0f + 10.0f * smoothstep(0.3f, 0.8f, hilly)) * relief_scale;
    if (base < 0.0f) relief *= 0.25f;
    float detail = dn * (1.2f + 3.5f * mountainness);
    detail *= 0.3f + 0.7f * smoothstep(8.0f, 60.0f, std::fabs(col.shore));   // a clean waterline: no puddles on the beach, no sand specks offshore
    float above_f = base + relief + detail;
    // Rocky coasts (ADR 0051, developer 2026-10-25): where the cliff mask is up the land ends in a sea cliff
    // 7-27 m tall along a wobbly line just inland of the coast, the cliff top sloping back down to the normal
    // land over ~1 km; sea stacks (some of them arches) stand on the shelf in front of it.
    if (col.c > -0.05f && col.c < 0.3f) {
        const float cm = n_cliff_.GetNoise(fx, fz);
        if (cm > 0.15f) {
            const float hc = cliff_height(cm) * smoothstep(0.15f, 0.25f, cm);
            const float edge = col.c - 0.004f * n_detail_.GetNoise(fx * 3.0f, fz * 3.0f);
            above_f += hc * smoothstep(-0.006f, -0.001f, edge) * (1.0f - smoothstep(0.08f, 0.3f, col.c));
            if (col.c < -0.004f && col.c > -0.04f) {
                const int scx = floor_div(x, STACK_CELL), scz = floor_div(z, STACK_CELL);
                for (int dz = -1; dz <= 1; dz++)
                    for (int dx = -1; dx <= 1; dx++) {
                        SeaStack st;
                        if (!stack_at_cell(scx + dx, scz + dz, st)) continue;
                        const float ddx = fx - st.x, ddz = fz - st.z;
                        const float rr = st.r * (0.85f + 0.3f * n_detail_.GetNoise(fx * 5.0f, fz * 5.0f));
                        if (ddx * ddx + ddz * ddz <= rr * rr) above_f = std::max(above_f, st.h - 2.0f * (ddx * ddx + ddz * ddz) / (rr * rr));
                    }
            }
        }
    }
    // Monoliths: a lone rock dome with near-vertical sides on open lowland.
    const float mono = (mountainness < 0.05f && col.c > 0.06f) ? monolith_height(x, z) : 0.0f;
    if (mono > 0.0f) {
        above_f += mono;
        col.rock = TERRACOTTA;
    }
    if (variant_[VAR_MOUNTAINS] != 0) {
        // soft ceiling: heights past 400 bend toward the world's top instead of being cut flat
        const float a0 = 400.0f, room = (float)(world_height_ - 12 - sea_) - a0;
        if (above_f > a0 && room > 0.0f) above_f = a0 + room * std::tanh((above_f - a0) / room);
    }
    col.hf = std::clamp((float)sea_ + above_f, 8.0f, (float)(world_height_ - 8));
    int h = (int)std::lround((float)sea_ + above_f);
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
    // a beach is a strip along the sea (15-26 m wide), not every low flat
    const bool shore = col.shore < 52.0f * (0.8f + 0.2f * jit);
    col.biome = technosim_biome::classify(col.t, col.h, above_j, col.c, col.e, col.slope, 0.0f, false, shore);
    return col;
}

uint16_t WorldGen::seabed(const Column &col, float slope) const {
    const int depth = sea_ - col.height;
    if (col.shore > -90.0f || depth <= 8) return SAND_STATIC;              // foreshore, bar, shallows
    if (slope > 0.6f) return col.dn < 0.0f ? STONE : GRAVEL_STATIC;        // rock on steep slopes
    if (depth <= 50) return col.dn > 0.3f ? CLAY : (col.dn < -0.35f ? GRAVEL_STATIC : SAND_STATIC);   // the shelf
    return col.dn < -0.4f ? GRAVEL_STATIC : CLAY;                           // mud in the deep
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
// (snow cover where it is cold enough, ADR 0025). Thread-safe.
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
    return hy.berm >= 0 ? std::max(c.height, hy.berm) : c.height;
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
    d["coast_dist"] = c.shore;   // estimated distance to the coastline (voxels), 1e9 away from coasts
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
    d["lake_near"] = hy.lake_near;
    d["base_height"] = c.height;
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
    // Slots remember the generator and its epoch: a reconfigured seed or variant never reads a stale tile.
    struct Slot { int tx = 0, tz = 0; uint64_t used = 0, epoch = 0; const WorldGen *owner = nullptr; std::shared_ptr<HydroTile> t; };
    thread_local Slot slots[4];
    thread_local uint64_t tick = 0;
    const uint64_t ep = epoch_.load(std::memory_order_relaxed);
    Slot *victim = &slots[0];
    for (auto &s : slots) {
        if (s.t && s.tx == tx && s.tz == tz && s.owner == this && s.epoch == ep) { s.used = ++tick; return s.t.get(); }
        if (s.used < victim->used) victim = &s;
    }
    victim->t = hydro_tile(tx, tz);
    victim->tx = tx;
    victim->tz = tz;
    victim->owner = this;
    victim->epoch = ep;
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
    // Lakes: the filled level of this cell or a neighbouring lake cell, and how far the column lies outside
    // the nearest lake cell. Water reaches half a cell past its lake cells; where the ground further out is
    // still below the level (a shallow lake on a flat plain), a berm one voxel over the water rims it, or
    // the water ended at the edge of the 3x3 reach as a glass wall along a cell border (bug report
    // 2026-10-25 02-11-15).
    float lake_level = -1.0f, lake_dist = 1e9f;
    for (int dj = -1; dj <= 1; dj++)
        for (int di = -1; di <= 1; di++) {
            const int k = (j + dj) * HWIN + (i + di);
            if (!t->lake[k]) continue;
            lake_level = std::max(lake_level, t->filled[k]);
            const int x0 = wx0 + (i + di) * HCELL, z0 = wz0 + (j + dj) * HCELL;
            const float ex = (float)std::max({x0 - x, 0, x - (x0 + HCELL - 1)}), ez = (float)std::max({z0 - z, 0, z - (z0 + HCELL - 1)});
            lake_dist = std::min(lake_dist, std::sqrt(ex * ex + ez * ez));
        }
    out.lake_near = lake_level;
    if (lake_level > 0.0f && (float)base_height >= lake_level - 0.5f && (float)base_height <= lake_level + 2.5f) out.shore = true;
    if (lake_level > 0.0f && (float)base_height < lake_level + 1.0f && lake_dist > HCELL * 0.5f) {
        out.berm = (int)std::ceil(lake_level) + 1;
        out.shore = true;
    } else if (lake_level > 0.0f && (float)base_height < lake_level - 0.5f) {
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
    // Rivers: segments from each nearby cell with enough drainage to its downstream cell. On flat land the
    // point is warped by a slow noise (up to 6 m), so lowland rivers swing in meanders instead of running
    // from cell centre to cell centre (ADR 0051); steep stretches and waterfalls stay as routed.
    float rx = (float)x, rz = (float)z;
    {
        // the bend amplitude from the drop of the four nearest cells, blended so it never jumps at a cell border
        auto cell_amp = [&](int k) {
            if (t->dir[k] < 0) return 0.0f;
            return 12.0f * (1.0f - smoothstep(0.5f, 3.0f, t->filled[k] - t->filled[downstream(k)]));
        };
        const float gx = ((float)x - (float)wx0) / HCELL - 0.5f, gz = ((float)z - (float)wz0) / HCELL - 0.5f;
        const int i0 = std::clamp((int)std::floor(gx), 1, HWIN - 3), j0 = std::clamp((int)std::floor(gz), 1, HWIN - 3);
        const float ax = std::clamp(gx - (float)i0, 0.0f, 1.0f), az = std::clamp(gz - (float)j0, 0.0f, 1.0f);
        const int k00 = j0 * HWIN + i0;
        const float amp = (cell_amp(k00) * (1.0f - ax) + cell_amp(k00 + 1) * ax) * (1.0f - az)
                        + (cell_amp(k00 + HWIN) * (1.0f - ax) + cell_amp(k00 + HWIN + 1) * ax) * az;
        if (amp > 0.0f) {
            rx += amp * n_meander_.GetNoise((float)x, (float)z);
            rz += amp * n_meander_.GetNoise((float)z + 7000.0f, (float)x - 7000.0f);
        }
    }
    float best_d = 1e9f;
    int best_k = -1;
    float best_t = 0.0f;
    // every stream segment near this column (ADR 0069 C): the surface blends between them by distance, so
    // where the nearest segment changes (a bend, a steep reach) the level does not jump by metres along a line
    struct Cand { int k; float d, tt; };
    Cand cands[9];
    int ncand = 0;
    for (int dj = -1; dj <= 1; dj++)
        for (int di = -1; di <= 1; di++) {
            const int k = (j + dj) * HWIN + (i + di);
            if (t->dir[k] < 0 || t->lake[k] || c.acc(k) < STREAM_ACC) continue;
            const int nk = downstream(k);
            float tt;
            float d = seg_dist(rx, rz, centre_x(k), centre_z(k), centre_x(nk), centre_z(nk), tt);
            cands[ncand++] = {k, d, tt};
            if (d < best_d) { best_d = d; best_k = k; best_t = tt; }
        }
    if (best_k >= 0) {
        const float strength = std::sqrt(c.acc(best_k) / STREAM_ACC);
        // Deltas (ADR 0051): where a big river's surface reaches the sea it spreads up to three times as wide,
        // shallow, braided around sandbar islands.
        const float mouth_surf = std::min(t->filled[best_k], t->filled[downstream(best_k)]);
        const float delta = mouth_surf <= (float)sea_ + 1.5f ? smoothstep(2.5f, 5.0f, strength) : 0.0f;
        const float half_w = river_half_width(strength) * (1.0f + 2.0f * delta);
        const float depth = river_depth(strength) * (1.0f - 0.5f * delta);
        const float bank_w = half_w * 1.5f + 6.0f; // gentle banks: valley floor widens with the river
        if (best_d < half_w + bank_w) {
            const int nk = downstream(best_k);
            const float up = t->filled[best_k], down = t->filled[nk];
            float surf = up + (down - up) * best_t;
            {
                // blend the surfaces of all segments in reach, weighted by (1 - d / reach)^2 (ADR 0069 C)
                const float reach = half_w + bank_w;
                float wsum = 0.0f, ssum = 0.0f;
                for (int q = 0; q < ncand; q++) {
                    if (cands[q].d >= reach) continue;
                    const float w = (1.0f - cands[q].d / reach) * (1.0f - cands[q].d / reach);
                    const int qk = cands[q].k, qn = downstream(qk);
                    ssum += w * (t->filled[qk] + (t->filled[qn] - t->filled[qk]) * cands[q].tt);
                    wsum += w;
                }
                if (wsum > 0.0f) surf = ssum / wsum;
            }
            surf = std::min(surf, (float)base_height); // never above the local ground
            // Waterfall sheet over the cliff part of this segment (precomputed per cell).
            if (t->fall_t0[best_k] >= 0.0f && best_t >= t->fall_t0[best_k] && best_t <= t->fall_t1[best_k] && best_d < half_w * 0.6f)
                out.fall_top = t->fall_top[best_k];
            const float mask = 1.0f - smoothstep(half_w, half_w + bank_w, best_d);
            out.river = mask;
            out.delta = delta;
            const int water = (int)std::floor(surf) - 1;
            // flat-bottomed channel: full depth over the inner half, rising to the banks
            out.bed = (int)std::floor(surf) - 1 - (int)std::lround(depth * (1.0f - smoothstep(0.45f * half_w, half_w, best_d)));
            if (delta > 0.0f && best_d < half_w * 0.85f && n_meander_.GetNoise((float)x * 2.3f + 300.0f, (float)z * 2.3f) > 0.62f - 0.3f * delta) {
                out.bed = water + 1;   // a sandbar just above the water
            } else if (best_d < half_w && water > out.bed) out.water_level = std::max(out.water_level, water);
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
    // A river trench carved below a neighbouring lake's level is part of the lake (bug report 2026-10-25
    // 02-11-15: the lake stood 0.9 m above the dry trench beside it as a glass wall); in the berm ring the
    // river keeps its water channel through the rim (its level there is the lake's).
    if (out.berm >= 0 && out.water_level >= 0) out.berm = -1;
    if (!out.lake && out.berm < 0 && lake_level > 0.0f && lake_dist <= HCELL * 0.5f && out.river > 0.0f && out.bed < base_height) {
        const float carved = (float)base_height + ((float)out.bed - (float)base_height) * out.river;
        if (carved < lake_level - 0.5f) {
            out.lake = true;
            out.water_level = std::max(out.water_level, (int)std::floor(lake_level));
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
    if (col.height <= sea_ + (col.biome == technosim_biome::MANGROVE ? 0 : 2) || col.alpine) return false;
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
    if (ravine_near(tx, tz, 5)) return false;   // trunk, roots and crown footing clear of a ravine's rim
    out.x = tx;
    out.z = tz;
    out.base = hy.marsh ? marsh_floor(col.height, hy) : col.height;
    out.kind = technosim_biome::TREE_KIND[col.biome];
    out.biome = col.biome;
    out.stunted = false;
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
    if (above > tree_line - 60) { out.trunk_h = 5 + (int)((hsh >> 16) % 3); out.radius = 2; out.stunted = true; }
    return true;
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
    if (ravine_near(out.x, out.z, 3)) return false;
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

bool WorldGen::ruin_at_cell(int cx, int cz, Ruin &out) const {
    // ADR 0062: one ruin in some 80 m cells; its kind fits the biome at its centre (data/ruins.json).
    uint32_t hsh = hash3(cx, cz, seed_ + 991);
    if ((hsh & 0xFF) > 36) return false;
    int margin = 14;   // the old RUIN_MAX_HALF + 4: ruins stay where they were when stone circles grew (ADR 0086)
    int span = RUIN_CELL - 2 * margin;
    int x = cx * RUIN_CELL + margin + (int)((hsh >> 16) % (uint32_t)span);
    int z = cz * RUIN_CELL + margin + (int)((hsh >> 24) % (uint32_t)span);
    Column c = column_base(x, z, false);
    const bool coast = c.height >= sea_ + 1 && c.height <= sea_ + 4;   // a shore: wrecks
    if (!coast && c.height < sea_ + 5) return false;
    if (c.alpine || c.snow || c.height - sea_ > technosim_biome::HIGHLAND_MAX) return false; // lowland and highland biomes only
    const int kind = technosim_ruin::pick_kind(c.biome, coast, hash3(cx, cz, seed_ + 992));
    if (kind < 0) return false;
    Hydro hy = hydro_at(x, z, c.height);
    if (hy.river > 0.05f || hy.lake || hy.marsh) return false;
    Ruin r;
    r.x = x;
    r.z = z;
    r.seed = hsh;
    r.kind = kind;
    technosim_ruin::shape_ruin(r);
    // Level ground only: the centre and eight points a cell outside the footprint within 1.5 m of each
    // other, so the flattened floor and the ramp round it (FineGen::macro_point) never cut a step into a
    // slope or leave the floor hanging over it.
    int hmin = c.height, hmax = c.height;
    const int ex = r.hw + 1, ez = r.hd + 1;
    const int sx[8] = {-ex, ex, -ex, ex, 0, 0, -ex, ex};
    const int sz[8] = {-ez, -ez, ez, ez, -ez, ez, 0, 0};
    for (int k = 0; k < 8; k++) {
        const int hh = column_base(x + sx[k], z + sz[k], false).height;
        hmin = std::min(hmin, hh);
        hmax = std::max(hmax, hh);
        if (hmax - hmin > 3) return false;
    }
    // a wreck lies up on the dry beach: since beaches slope into the sea (GEN_VERSION 26) a hull across the
    // waterline had its seaward half over the falling foreshore
    if (coast && hmin < sea_ + 1) return false;
    r.base = (hmin + hmax + 1) / 2;
    if (coast) r.base = std::max(r.base, sea_ + 1);
    out = r;
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

Array WorldGen::ruin_pots_near(int x, int z, int radius) const {
    Array out;
    Ruin rs[16];
    const int n = ruins_near(x - radius, z - radius, x + radius, z + radius, rs, 16);
    for (int i = 0; i < n; i++) {
        const Ruin &r = rs[i];
        technosim_ruin::PotSpot spots[4];
        const int ns = technosim_ruin::pot_spots(r, spots);
        for (int k = 0; k < ns; k++) {
            const long dx = spots[k].mx - x, dz = spots[k].mz - z;
            if (dx * dx + dz * dz > (long)radius * radius) continue;
            Dictionary d;
            d["x"] = spots[k].mx;
            d["y"] = spots[k].y;
            d["z"] = spots[k].mz;
            d["ruin"] = (int)r.seed;
            d["seed"] = (int)(spots[k].h & 0x7fffffff);
            d["kind"] = String(technosim_ruin::KIND_NAMES[r.kind]);
            out.push_back(d);
        }
    }
    return out;
}

Dictionary WorldGen::ruin_at(int x, int z) const {
    Dictionary d;
    Ruin rs[4];
    const int n = ruins_near(x, z, x, z, rs, 4);
    for (int i = 0; i < n; i++) {
        if (!in_ruin(rs[i], x, z, 2)) continue;
        d["kind"] = String(technosim_ruin::KIND_NAMES[rs[i].kind]);
        d["x"] = rs[i].x;
        d["y"] = rs[i].base;
        d["z"] = rs[i].z;
        d["half_x"] = rs[i].hw;
        d["half_z"] = rs[i].hd;
        d["rot"] = rs[i].rot;
        break;
    }
    return d;
}

bool WorldGen::in_ruin(const Ruin &r, int x, int z, int margin) {
    return std::abs(x - r.x) <= r.hw + margin && std::abs(z - r.z) <= r.hd + margin;
}

inline bool WorldGen::is_cave(int x, int y, int z, int height) const {
    if (y < 12 || y > height - cave_roof(x, z)) return false;
    return cave_value(x, y, z, height) > 0.0f;
}

void WorldGen::cave_column(int x, int z, CaveColumn &out) const {
    out.x = x, out.z = z;
    out.galoff = n_galoff_.GetNoise((float)x, (float)z);
    out.nmouths = mouths_near(x, z, out.mouths);
}

float WorldGen::cave_value(int x, int y, int z, int height, bool small_only) const {
    CaveColumn col;
    cave_column(x, z, col);
    return cave_value(col, y, height, small_only);
}

float WorldGen::cave_value(const CaveColumn &col, int y, int height, bool small_only) const {
    const int x = col.x, z = col.z;
    const float fx = (float)x, fy = (float)y, fz = (float)z;
    const float depth = (float)(height - y);
    const bool epic = variant_[VAR_CAVES] != 0;
    const float thr = (epic ? 0.55f : 0.62f) - 0.12f * smoothstep(20.0f, 200.0f, depth);
    // Three storeys (ADR 0077): rooms squashed flat near the top, and past ~100 m down the cheese stands up into
    // tall rifts (the deep is chasms, not more rooms).
    const float ysq = epic ? 1.6f - 1.1f * smoothstep(200.0f, 340.0f, depth) : 1.6f;
    const float cheese = (n_cheese_.GetNoise(fx, fy * ysq, fz) - thr) / 0.08f;
    const float s1 = n_spag1_.GetNoise(fx, fy * 1.3f, fz), s2 = n_spag2_.GetNoise(fx, fy * 1.3f, fz);
    // big caves: tunnels 3-5 m wide, swelling and narrowing along their length (the gallery offset noise doubles
    // as the width: ~200 m between a tunnel's narrows and its widest stretch, ADR 0109)
    const float sw = epic ? 0.045f * (1.0f + 0.5f * col.galoff) : 0.0115f;
    float v = std::max(cheese, (sw - (s1 * s1 + s2 * s2)) / sw);
    if (!epic) return v;
    // Galleries (ADR 0080): a storey of walkable passages every 14 m down, each a winding network along the zero
    // lines of its own 2D noise, ~4 m wide and 3 m tall with a flat floor that tilts at most ~10 degrees, so a
    // passage can be followed for hundreds of metres; noodles, chimneys and halls join the storeys.
    if (depth > 12.0f && depth < 220.0f) {
        constexpr float SP = 28.0f, HH = 3.2f, W = 0.22f;   // ~4 m wide, 3 m tall (ADR 0109)   // storey spacing, half height (macro), route half width
        const float yo = fy - col.galoff * 10.0f;
        const float lvl = std::floor(yo / SP + 0.5f);
        const float dy = std::fabs(yo - lvl * SP);
        if (dy < HH) {
            const float r = std::fabs(n_gallery_.GetNoise(fx + lvl * 1013.0f, fz - lvl * 731.0f));
            v = std::max(v, std::min((W - r) / W, (HH - dy) / HH) * 2.0f);
        }
    }
    // Entrances and connectors (ADR 0077): winding ramps that leave the ground, sinkholes, chimneys.
    if (depth < 520.0f) {
        const CaveMouth *m = col.mouths;
        const int n = col.nmouths;
        for (int i = 0; i < n; i++) {
            if (m[i].kind == MOUTH_RAMP) {
                float t;
                seg_dist(fx, fz, m[i].x, m[i].z, m[i].x + m[i].dx * m[i].len, m[i].z + m[i].dz * m[i].len, t);
                // the tunnel winds: its centre line sways across the ramp direction, one bend per ~15 m
                const float sway = std::sin(t * m[i].len * 0.21f + m[i].slope * 9.0f) * m[i].r * 1.2f;
                const float cx = m[i].x + m[i].dx * t * m[i].len - m[i].dz * sway, cz = m[i].z + m[i].dz * t * m[i].len + m[i].dx * sway;
                const float d = std::sqrt((fx - cx) * (fx - cx) + (fz - cz) * (fz - cz));
                if (d > m[i].r) continue;
                const float want = t * m[i].len * m[i].slope + m[i].r * 0.6f;   // tunnel centre depth below the ground
                const float dy = (depth - want) / (m[i].r * 0.8f), dh = d / m[i].r;
                v = std::max(v, (1.0f - (dy * dy + dh * dh)) * 4.0f);
            } else {
                // a shaft (sinkhole or chimney) from m.top to m.top + m.len below the ground, leaning a little
                // with depth; the sinkhole flares into a funnel at the ground (twice its radius at the rim)
                const float u = (depth - m[i].top) / m[i].len;
                if (u < -0.1f || u > 1.1f) continue;
                const float lean = u * m[i].len * 0.12f;
                const float dx = fx - (m[i].x + m[i].dx * lean), dz = fz - (m[i].z + m[i].dz * lean);
                float R = m[i].r;
                if (m[i].kind == MOUTH_SINKHOLE) R *= 1.0f + 1.0f * (1.0f - smoothstep(0.0f, 3.0f * m[i].r, depth));
                const float dh2 = (dx * dx + dz * dz) / (R * R);
                const float cap = std::min(1.0f - u, u + 1.0f) * m[i].len / m[i].r;   // rounded ends
                v = std::max(v, std::min(1.0f - dh2, cap) * 4.0f);
            }
        }
    }
    // Arches: a tunnel through a sea stack at the waterline.
    if (y > sea_ && y < sea_ + 16) {
        const int scx = floor_div(x, STACK_CELL), scz = floor_div(z, STACK_CELL);
        for (int dz = -1; dz <= 1; dz++)
            for (int dx = -1; dx <= 1; dx++) {
                SeaStack st;
                if (!stack_at_cell(scx + dx, scz + dz, st) || !st.arch) continue;
                const float px = fx - st.x, pz = fz - st.z;
                const float across = std::fabs(-px * st.adz + pz * st.adx), along = std::fabs(px * st.adx + pz * st.adz);
                if (along > st.r + 2.0f) continue;
                const float ay = (fy - (float)(sea_ + 4)) / (st.r * 0.75f), az = across / (st.r * 0.55f);
                v = std::max(v, (1.0f - (ay * ay + az * az)) * 4.0f);
            }
    }
    if (small_only) return v;
    v = std::max(v, cavern_value(n_cavern_.GetNoise(fx, fy * CAVERN_Y, fz), fy, depth, n_pillar_.GetNoise(fx, fz)));
    return std::max(v, ravine_value(n_ravmask_.GetNoise(fx, fz), std::fabs(n_ravine_.GetNoise(fx, fz)), depth));
}

// Caverns: a slow 3D noise squashed in y, so halls are wide and flat-floored (40-120 m across, 15-40 m
// tall, developer 2026-10-25 "big"), from 15 m under the ground down to bedrock. raw = n_cavern_ at
// (x, y * CAVERN_Y, z); FineGen samples it on a coarse grid.
float WorldGen::cavern_value(float raw, float fy, float depth, float pillar) {
    const float v = (raw - CAVERN_THR) / 0.05f * smoothstep(30.0f, 70.0f, depth) * smoothstep(14.0f, 40.0f, fy);
    // pillars (ADR 0077): where the column noise peaks the hall keeps rock from floor to ceiling, 2-3 m thick
    const float p = smoothstep(0.5f, 0.62f, pillar);
    return v * (1.0f - p) - p;
}

// Ravines: along the zero line of a slow 2D noise (line = |n_ravine_|) where a mask (n_ravmask_) opens it,
// a gorge ~10-30 m wide at the top narrowing to nothing 10-40 m down. The width falls off as (1 - depth)^2.2,
// so the walls flare out toward the rim and steepen below; upper walls rise ~2 per run where they were 5-13
// (bug report 2026-10-25 20-18-57, "ravines seem a tad bit steep").
float WorldGen::ravine_value(float mask, float line, float depth) {
    if (mask <= 0.25f) return -1.0f;
    // Both the depth and the width fade in where the mask opens, so a ravine's ends are shallow trenches
    // instead of deep slots a few voxels wide (the old width faded to nothing while the depth stayed 30 m).
    const float open = smoothstep(0.25f, 0.45f, mask);
    const float deep = (20.0f + 60.0f * smoothstep(0.3f, 0.6f, mask)) * open;
    if (deep <= 0.0f) return -1.0f;
    const float u = 1.0f - std::clamp(depth / deep, 0.0f, 1.0f);
    const float half = 0.14f * (0.35f + 0.65f * open) * std::pow(u, 2.2f);
    return half > 0.0f ? (half - line) / 0.008f : -1.0f;
}

bool WorldGen::ravine_near(int x, int z, int r) const {
    if (variant_[VAR_CAVES] == 0) return false;
    CaveMouth m[9];
    const int n = mouths_near(x, z, m);
    for (int i = 0; i < n; i++) {
        if (m[i].kind != MOUTH_SINKHOLE) continue;
        const float dx = (float)x - m[i].x, dz = (float)z - m[i].z, reach = 2.0f * m[i].r + (float)r;
        if (dx * dx + dz * dz <= reach * reach) return true;
    }
    if (n_ravmask_.GetNoise((float)x, (float)z) <= 0.2f) return false;   // ravines open only past 0.25
    static const int OX[9] = {0, 1, -1, 0, 0, 1, 1, -1, -1}, OZ[9] = {0, 0, 0, 1, -1, 1, -1, 1, -1};
    for (int k = 0; k < 9; k++) {
        const float px = (float)(x + OX[k] * r), pz = (float)(z + OZ[k] * r);
        // a voxel under the ground: the rim of the cut, not only its middle
        if (ravine_value(n_ravmask_.GetNoise(px, pz), std::fabs(n_ravine_.GetNoise(px, pz)), 1.0f) > -1.5f) return true;
    }
    return false;
}

// Whether a cavern or a ravine may reach into the box (macro units), for the deep-rock shortcut of coarse
// LODs: the cavern noise on a 16-voxel grid with a margin for what can rise between the samples.
bool WorldGen::big_cave_possible(int x0, int y0, int z0, int x1, int y1, int z1, int ground_min) const {
    if (variant_[VAR_CAVES] == 0) return false;
    const bool ravine_deep_enough = y1 >= ground_min - 120;   // ravines end 60 m down at most
    for (int z = z0;; z = std::min(z + 16, z1)) {
        for (int x = x0;; x = std::min(x + 16, x1)) {
            if (ravine_deep_enough && n_ravmask_.GetNoise((float)x, (float)z) > 0.15f) return true;
            for (int y = y0;; y = std::min(y + 16, y1)) {
                if (n_cavern_.GetNoise((float)x, (float)y * CAVERN_Y, (float)z) > CAVERN_THR - 0.15f) return true;
                if (y == y1) break;
            }
            if (x == x1) break;
        }
        if (z == z1) break;
    }
    return false;
}

// Geodes: 1 in 9 cells of 96 m (~12 per 10 km^2), a hollow 3-6 m across, 25-90 m under the ground and
// never below the lava line. The radius covers the hollow, its crystal lining and the basalt shell.
bool WorldGen::conduit_at_cell(int cx, int cz, Conduit &out) const {
    if (variant_[VAR_CAVES] == 0) return false;
    const uint32_t h = hash3(cx, cz, seed_ + 5111);
    if (h % 5 >= 2) return false;   // 40 % of cells: ~6 per km^2
    out.kind = ((h >> 3) & 3) == 0 ? CONDUIT_MAGMA : CONDUIT_AQUIFER;   // one in four carries magma
    const int margin = 64;
    out.x[0] = (float)(cx * CONDUIT_CELL + margin) + (float)((h >> 12) & 255) / 255.0f * (float)(CONDUIT_CELL - 2 * margin);
    out.z[0] = (float)(cz * CONDUIT_CELL + margin) + (float)((h >> 20) & 255) / 255.0f * (float)(CONDUIT_CELL - 2 * margin);
    const Column c = column_base((int)out.x[0], (int)out.z[0], false);
    if (c.height <= sea_ + 4) return false;   // not under the sea
    // the chamber: aquifers 70-134 m down, magma 120-184 m, always above the lava sea (sea - 180)
    const float depth = (out.kind == CONDUIT_MAGMA ? 240.0f : 140.0f) + (float)((h >> 5) & 127);
    out.y[0] = std::max((float)c.height - depth, (float)(sea_ - 160));
    if (out.y[0] < 40.0f) return false;
    out.r = 2.0f + (float)((h >> 28) & 7) * 0.2f;      // tube 1-1.7 m
    out.src_r = 9.0f + (float)((h >> 8) & 7);          // chamber 4.5-8 m
    float a = (float)(hash3(cz, cx, seed_ + 5112) & 1023) / 1024.0f * 6.2831853f;
    for (int i = 1; i <= CONDUIT_SEGS; i++) {
        // each segment 40-72 m long, bending up to a radian, rising 4-11 m: the tail ends 4-110 m under the
        // chamber's ground, so a conduit may seep out on a valley floor (a spring, a lava seep)
        const uint32_t hs = hash3(cx, cz, seed_ + 5200 + i);
        a += ((float)(hs & 255) / 255.0f - 0.5f) * 2.0f;
        const float len = 80.0f + (float)((hs >> 8) & 63);
        out.x[i] = out.x[i - 1] + std::cos(a) * len;
        out.z[i] = out.z[i - 1] + std::sin(a) * len;
        out.y[i] = out.y[i - 1] + 8.0f + (float)((hs >> 14) & 15);
    }
    return true;
}

int WorldGen::conduits_near(int x, int z, Conduit *out, int max) const {
    const int cx = floor_div(x, CONDUIT_CELL), cz = floor_div(z, CONDUIT_CELL);
    int n = 0;
    for (int dz = -CONDUIT_RINGS; dz <= CONDUIT_RINGS && n < max; dz++)
        for (int dx = -CONDUIT_RINGS; dx <= CONDUIT_RINGS && n < max; dx++)
            if (conduit_at_cell(cx + dx, cz + dz, out[n])) n++;
    return n;
}

float WorldGen::conduit_dist(float x, float y, float z, int &kind, float &src_dist) const {
    Conduit cs[(2 * CONDUIT_RINGS + 1) * (2 * CONDUIT_RINGS + 1)];
    const int n = conduits_near((int)x, (int)z, cs, (int)(sizeof(cs) / sizeof(cs[0])));
    float best = 1e9f;
    kind = -1;
    src_dist = 1e9f;
    for (int i = 0; i < n; i++) {
        const Conduit &c = cs[i];
        const float sd = std::sqrt((x - c.x[0]) * (x - c.x[0]) + (y - c.y[0]) * (y - c.y[0]) + (z - c.z[0]) * (z - c.z[0])) - c.src_r;
        float d = sd;
        for (int s = 0; s < CONDUIT_SEGS; s++)
            d = std::min(d, seg_dist3(x, y, z, c.x[s], c.y[s], c.z[s], c.x[s + 1], c.y[s + 1], c.z[s + 1]) - c.r);
        if (d < best) { best = d; kind = c.kind; src_dist = sd; }
    }
    return best;
}

bool WorldGen::geode_at_cell(int cx, int cz, Geode &out) const {
    if (variant_[VAR_CAVES] == 0) return false;
    const uint32_t h = hash3(cx, cz, seed_ + 4111);
    if (h % 9 != 0) return false;
    const float r = 6.0f + (float)((h >> 8) & 7) * 0.9f;   // 3-6 m
    const int margin = 14;
    out.x = (float)(cx * GEODE_CELL + margin) + (float)((h >> 12) & 255) / 255.0f * (float)(GEODE_CELL - 2 * margin);
    out.z = (float)(cz * GEODE_CELL + margin) + (float)((h >> 20) & 255) / 255.0f * (float)(GEODE_CELL - 2 * margin);
    const Column c = column_base((int)out.x, (int)out.z, false);
    const float depth = 50.0f + (float)(hash3(cx, cz, seed_ + 4112) % 130);
    out.y = std::max((float)c.height - depth, (float)(sea_ - 170));
    if (out.y > (float)c.height - 30.0f || out.y < 24.0f) return false;
    out.r = r;
    out.seed = h;
    return true;
}

// Sea stacks: 30 % of the 12 m cells on the shelf in front of a cliff coast; a quarter of them are arches.
bool WorldGen::stack_at_cell(int cx, int cz, SeaStack &out) const {
    const uint32_t h = hash3(cx, cz, seed_ + 5101);
    if ((h & 1023) >= 307) return false;
    out.x = (float)(cx * STACK_CELL + 6) + (float)((h >> 10) & 15) * 0.8f;
    out.z = (float)(cz * STACK_CELL + 6) + (float)((h >> 14) & 15) * 0.8f;
    const float c = n_cont_.GetNoise(out.x, out.z);
    if (c >= -0.006f || c < -0.04f) return false;
    const float cm = n_cliff_.GetNoise(out.x, out.z);
    if (cm < 0.2f) return false;
    out.r = 3.0f + (float)((h >> 18) & 7) * 0.6f;                       // 1.5-3.6 m
    out.h = cliff_height(cm) * (0.55f + 0.45f * (float)((h >> 21) & 15) / 15.0f) + 2.0f;
    out.arch = ((h >> 25) & 3) == 0 && out.r >= 4.5f;
    const float a = (float)((h >> 27) & 31) / 31.0f * 3.14159f;
    out.adx = std::cos(a);
    out.adz = std::sin(a);
    return true;
}

// Monoliths: 1 in 5 cells of 1.5 km, a dome 150-350 m long, 0.45-0.65 as wide and 40-80 m tall, a superellipse
// footprint with near-vertical sides and a rounded top, grooved by rain.
float WorldGen::monolith_height(int x, int z) const {
    const int cx = floor_div(x, MONO_CELL), cz = floor_div(z, MONO_CELL);
    const uint32_t h = hash3(cx, cz, seed_ + 6203);
    if (h % 5 != 0) return 0.0f;
    const float L = 300.0f + (float)((h >> 3) & 255) / 255.0f * 400.0f;   // half-length x 2 below, macro
    const float W = L * (0.45f + 0.2f * (float)((h >> 11) & 15) / 15.0f);
    const float mx = (float)(cx * MONO_CELL) + L + (float)((h >> 15) & 255) / 255.0f * ((float)MONO_CELL - 2.0f * L);
    const float mz = (float)(cz * MONO_CELL) + L + (float)((h >> 23) & 255) / 255.0f * ((float)MONO_CELL - 2.0f * L);
    const float ddx = (float)x - mx, ddz = (float)z - mz;
    if (std::fabs(ddx) > L * 0.6f && std::fabs(ddz) > L * 0.6f) return 0.0f;
    const float a = (float)(hash3(cx, cz, seed_ + 6204) & 1023) / 1023.0f * 3.14159f;
    const float u = (ddx * std::cos(a) + ddz * std::sin(a)) / (L * 0.5f), v = (-ddx * std::sin(a) + ddz * std::cos(a)) / (W * 0.5f);
    const float d = std::pow(std::pow(std::fabs(u), 3.0f) + std::pow(std::fabs(v), 3.0f), 1.0f / 3.0f);
    if (d >= 1.0f) return 0.0f;
    // the centre must be open lowland (not a mountain, lake or the sea)
    const float region = n_region_.GetNoise(mx, mz), c = n_cont_.GetNoise(mx, mz);
    if (region > 0.0f || c < 0.08f) return 0.0f;
    const float H = 80.0f + (float)((h >> 7) & 255) / 255.0f * 80.0f;
    const float groove = 1.0f - 0.06f * std::fabs(n_detail_.GetNoise((float)x * 4.0f, (float)z * 4.0f));
    return H * std::sqrt(std::max(0.0f, 1.0f - std::pow(d, 8.0f))) * groove;
}

// Cave mouths on a 64 m grid (developer 2026-10-25: "big" caves, ~15-30 entrances per km^2).
int WorldGen::mouths_near(int x, int z, CaveMouth *out) const {
    const int cx = floor_div(x, MOUTH_CELL), cz = floor_div(z, MOUTH_CELL);
    int n = 0;
    for (int dz = -1; dz <= 1; dz++)
        for (int dx = -1; dx <= 1; dx++) {
            const uint32_t h = hash3(cx + dx, cz + dz, seed_ + 9133);
            const uint32_t roll = h & 1023;
            // ~10 % of cells a ramp (~24 per km^2), 6 % a sinkhole (~15), 16 % a chimney (~40)
            if (roll >= 330) continue;
            CaveMouth &m = out[n++];
            m.kind = roll < 100 ? MOUTH_RAMP : (roll < 165 ? MOUTH_SINKHOLE : MOUTH_CHIMNEY);
            m.x = (float)((cx + dx) * MOUTH_CELL) + (float)((h >> 10) & 127);
            m.z = (float)((cz + dz) * MOUTH_CELL) + (float)((h >> 17) & 127);
            const float a = (float)((h >> 24) & 255) / 255.0f * 6.2831853f;
            m.dx = std::cos(a);
            m.dz = std::sin(a);
            const uint32_t h2 = hash3(cx + dx, cz + dz, seed_ + 9134);
            m.top = 0.0f;
            if (m.kind == MOUTH_RAMP) {
                m.len = 40.0f + (float)(h2 & 63) * 0.8f;                   // 20-45 m
                m.slope = 0.45f + (float)((h2 >> 6) & 31) / 31.0f * 0.4f;  // 25-40 degrees
                m.r = 3.0f + (float)((h2 >> 11) & 15) / 15.0f * 2.0f;      // 1.5-2.5 m
            } else if (m.kind == MOUTH_SINKHOLE) {
                m.len = 60.0f + (float)(h2 & 63) * 1.2f;                   // 30-68 m down: into the hall storey
                m.slope = 0.0f;
                m.r = 5.0f + (float)((h2 >> 11) & 15) / 15.0f * 4.0f;      // 2.5-4.5 m (the funnel rim twice that)
            } else {
                m.top = 30.0f + (float)((h2 >> 6) & 63);                   // starts 15-47 m down, in the tunnels
                m.len = 160.0f + (float)(h2 & 127) * 1.6f;                 // 80-180 m: through the halls to the deep
                m.slope = 0.0f;
                m.r = 3.0f + (float)((h2 >> 11) & 15) / 15.0f * 3.0f;      // 1.5-3 m
            }
        }
    return n;
}

int WorldGen::cave_roof(int x, int z) const {
    if (variant_[VAR_CAVES] == 0) return 5;
    const float fx = (float)x, fz = (float)z;
    if (ravine_value(n_ravmask_.GetNoise(fx, fz), std::fabs(n_ravine_.GetNoise(fx, fz)), 4.0f) > 0.0f) return 0;
    CaveMouth m[9];
    const int n = mouths_near(x, z, m);
    for (int i = 0; i < n; i++) {
        if (m[i].kind == MOUTH_CHIMNEY) continue;
        if (m[i].kind == MOUTH_SINKHOLE) {
            const float dx = fx - m[i].x, dz = fz - m[i].z, reach = 2.0f * m[i].r + 1.0f;
            if (dx * dx + dz * dz <= reach * reach) return 0;
            continue;
        }
        float t;
        // the ramp sways up to 1.2 r off its line (cave_value)
        if (seg_dist(fx, fz, m[i].x, m[i].z, m[i].x + m[i].dx * m[i].len, m[i].z + m[i].dz * m[i].len, t) <= m[i].r * 2.2f + 1.0f && t * m[i].len < 16.0f) return 0;
    }
    return 5;
}

int WorldGen::open_depth(int x, int z) const {
    if (variant_[VAR_CAVES] == 0) return 0;
    const float fx = (float)x, fz = (float)z;
    int depth = 0;
    const float mask = n_ravmask_.GetNoise(fx, fz);
    if (ravine_value(mask, std::fabs(n_ravine_.GetNoise(fx, fz)), 4.0f) > 0.0f) {
        // ravine_value: deep = (20 + 60 * smoothstep(0.3, 0.6, mask)) * open, the gorge's floor
        const float open = smoothstep(0.25f, 0.45f, mask);
        depth = (int)std::ceil((20.0f + 60.0f * smoothstep(0.3f, 0.6f, mask)) * open) + 4;
    }
    CaveMouth m[9];
    const int n = mouths_near(x, z, m);
    for (int i = 0; i < n; i++) {
        if (m[i].kind == MOUTH_CHIMNEY) continue;
        if (m[i].kind == MOUTH_SINKHOLE) {
            const float dx = fx - m[i].x, dz = fz - m[i].z, reach = 2.0f * m[i].r + 1.0f;
            if (dx * dx + dz * dz <= reach * reach) depth = std::max(depth, (int)std::ceil(m[i].top + m[i].len + m[i].r) + 4);
            continue;
        }
        float t;
        if (seg_dist(fx, fz, m[i].x, m[i].z, m[i].x + m[i].dx * m[i].len, m[i].z + m[i].dz * m[i].len, t) <= m[i].r * 2.2f + 1.0f && t * m[i].len < 16.0f)
            depth = std::max(depth, (int)std::ceil(16.0f * m[i].slope + m[i].r * 1.6f) + 4);   // the mouth's first 8 m of tunnel
    }
    return depth;
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

int WorldGen::stringer_type(int cx, int cz) const {
    using namespace technosim_ore;
    Vein v;
    if (vein_at_cell(cx, cz, v)) return v.type;
    int pick = (int)(hash3(cx + 3, cz + 9, seed_ + 7731) % (uint32_t)WEIGHT_TOTAL), t = 0;
    while (t < COUNT - 1 && pick >= WEIGHT[t]) { pick -= WEIGHT[t]; t++; }
    return t;
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

// Ore-bearing stones on the ground near a vein's centre (Vintage Story style):
// the chance per column fades from INDICATOR_CHANCE with the vein top at the
// surface to 0 at INDICATOR_MAX_DEPTH below it.
uint16_t WorldGen::indicator_at(const Vein &v, int x, int z, int ground, uint16_t top) const {
    using namespace technosim_ore;
    const int ind = INDICATOR[v.type];
    if (ind < 0) return AIR;
    if (!(top == GRASS || top == DIRT || top == GRAVEL_STATIC || top == STONE || top == SAND_STATIC || top == SNOW || top == CLAY
          || top == SAVANNA_GRASS || top == RED_EARTH)) return AIR;
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
    if (hy.shore && !col.snow && col.t > -0.42f && (top == GRASS || top == SAVANNA_GRASS || top == DIRT || top == CLAY || top == SAND_STATIC) && r < 330) {
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
    if ((top == GRAVEL_STATIC || top == STONE || top == BASALT) && r < 22 && !marsh) return PEBBLES;   // about half (bug report 2026-10-27 15-16-47)
    if (top == GRAVEL_STATIC && r >= 22 && r < 32 && !marsh) return FLINT_STONES;
    if (col.biome == VOLCANIC) return r < 30 ? DEAD_BUSH : AIR;   // scorched scrub between the basalt
    if (col.biome == MANGROVE && top == DIRT) {   // the mud flats: reeds and ferns among the roots
        if (r < 140) { height = 2; return REED; }
        return r < 210 ? FERN : AIR;
    }
    if (top != GRASS && top != SAVANNA_GRASS && !(marsh && top == DIRT)) return AIR;
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
        case PLAINS: grass = 300; flower = 36; pebble = 3; twig = 2; flint = 2; break;
        case FOREST: grass = 170; flower = 14; shroom = 10; pebble = 2; twig = 12; flint = 1; fern = 15; berry = 6; break;
        case OLD_GROWTH_FOREST: grass = 200; flower = 10; shroom = 22; pebble = 2; twig = 16; fern = 70; berry = 4; break;
        case HIGHLANDS: grass = 110; flower = 10; pebble = 10; twig = 2; flint = 4; break;
        case MOUNTAINS: grass = 50; flower = 4; pebble = 15; flint = 4; break;
        case ALPINE: grass = 25; flower = 6; pebble = 15; flint = 3; break;
        case TAIGA: grass = 90; shroom = 12; pebble = 4; twig = 12; flint = 1; fern = 50; berry = 10; break;
        case SHRUBLAND: grass = 120; bush = 40; pebble = 7; twig = 4; flint = 3; break;
        case SAVANNA: grass = 420; bush = 8; twig = 2; flint = 2; break;   // the column loop lays dry tussocks (fine_gen.cpp)
        case JUNGLE: grass = 350; flower = 20; shroom = 14; twig = 10; fern = 110; break;
        case SWAMP: grass = 450; shroom = 18; twig = 6; fern = 40; break;
        case BIRCH_FOREST: grass = 210; flower = 22; shroom = 12; pebble = 2; twig = 12; flint = 1; fern = 25; berry = 8; break;
        case FLOWER_MEADOW: grass = 240; flower = 170; pebble = 2; twig = 1; flint = 2; break;
        case MANGROVE: grass = 300; shroom = 10; fern = 80; break;
        default: break;
    }
    if (bush > 0 && r >= 1000 - bush) return DEAD_BUSH;
    // the remains of an old kill (ADR 0027): rare, where predators and grazers meet
    if (!marsh && (col.biome == PLAINS || col.biome == FLOWER_MEADOW || col.biome == BIRCH_FOREST || col.biome == FOREST || col.biome == TAIGA || col.biome == SAVANNA || col.biome == SHRUBLAND)
            && (hash3(x, z, seed_ + 97) % 20000u) < 3u) return CARCASS;
    if (shroom > 0 && r >= 1000 - bush - shroom) {
        // red caps in the wet, dark biomes, brown elsewhere
        const bool red = (col.biome == SWAMP || col.biome == MANGROVE || col.biome == OLD_GROWTH_FOREST || col.biome == JUNGLE) && ((v >> 14) & 3) == 0;
        return red ? MUSHROOM_RED : MUSHROOM_BROWN;
    }
    if (pebble > 0 && r >= 1000 - bush - shroom - pebble) return PEBBLES;
    if (twig > 0 && r >= 1000 - bush - shroom - pebble - twig) return TWIGS;
    if (flint > 0 && r >= 1000 - bush - shroom - pebble - twig - flint) return FLINT_STONES;
    if (berry > 0 && r >= 1000 - bush - shroom - pebble - twig - flint - berry) return BLUEBERRY_BUSH;
    if (flower > 0) {
        const float patch = n_detail_.GetNoise((float)x * 2.0f + 5000.0f, (float)z * 2.0f - 5000.0f);
        flower = patch > 0.3f ? flower * 5 : (col.biome == FLOWER_MEADOW ? flower : flower / 4);   // a meadow flowers everywhere
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
        if (col.biome == FOREST || col.biome == OLD_GROWTH_FOREST || col.biome == BIRCH_FOREST) mix = WOODS;
        else if (col.biome == HIGHLANDS || col.biome == MOUNTAINS || col.biome == ALPINE) mix = UPLAND;
        else if (col.biome == JUNGLE || col.biome == SWAMP || col.biome == MANGROVE) mix = WET;
        // flowers grow in patches of one kind: the kind follows a coarse cell, not the column
        const uint32_t fk = hash3(x >> 3, z >> 3, seed_ + 223);
        return mix[fk & 3];
    }
    return AIR;
}

// ---------- debug ----------

Vector3i WorldGen::find_nearest(const String &kind, int x, int z, int max_radius) const {
    Vector3i best(0, -1, 0);
    long best_d2 = -1;
    int cave_kind = -1;   // a named cave biome ("sulfur_cave", "lush_cave", ...); "cave" and "deep_cave" are plain rooms
    for (int b = 0; b < technosim_biome::COUNT; b++)
        if (kind == technosim_biome::NAMES[b] && kind.ends_with("_cave") && kind != "deep_cave") cave_kind = b;
    if (kind == "ruin" || kind.begins_with("ruin_") || kind == "crypt_room") {
        // "ruin", "ruin_<kind>" (data/ruins.json), or "crypt_room": a free spot of a crypt's room floor
        int want = -1;
        if (kind == "crypt_room") want = -2;
        for (int k = 0; k < technosim_ruin::NKIND; k++)
            if (kind == String("ruin_") + technosim_ruin::KIND_NAMES[k]) want = k;
        if (want == -1 && kind != "ruin") return best;
        const int cx0 = (int)std::floor((float)x / RUIN_CELL), cz0 = (int)std::floor((float)z / RUIN_CELL);
        const int rings = max_radius / RUIN_CELL + 1;
        for (int ring = 0; ring <= rings; ring++) {
            for (int dz = -ring; dz <= ring; dz++)
                for (int dx = -ring; dx <= ring; dx++) {
                    if (std::max(std::abs(dx), std::abs(dz)) != ring) continue;
                    Ruin r;
                    if (!ruin_at_cell(cx0 + dx, cz0 + dz, r)) continue;
                    if (want >= 0 && r.kind != want) continue;
                    if (want == -2 && technosim_ruin::KIND_SHAPE[r.kind] != technosim_ruin::S_CRYPT) continue;
                    long d2 = (long)(r.x - x) * (r.x - x) + (long)(r.z - z) * (r.z - z);
                    if (best_d2 < 0 || d2 < best_d2) {
                        best_d2 = d2;
                        best = Vector3i(r.x, r.base, r.z);
                        if (want == -2) {   // beside the coffin, between the room's far pots
                            const int U = (r.rot & 1) ? r.hd : r.hw, V = (r.rot & 1) ? r.hw : r.hd;
                            const int u = U - 2, v = -(V - 2) + 2;
                            int ddx = 0, ddz = 0;
                            switch (r.rot & 3) { case 0: ddx = u; ddz = v; break; case 1: ddx = -v; ddz = u; break; case 2: ddx = -u; ddz = -v; break; default: ddx = v; ddz = -u; break; }
                            best = Vector3i(r.x + ddx, r.base - std::max(4, r.depth - 1) + 1, r.z + ddz);
                        }
                    }
                }
            if (best_d2 >= 0) break;
        }
        return best;
    }
    if (kind == "monolith" || kind == "sea_stack" || kind == "arch") {
        const bool mono = kind == "monolith";
        const int cell = mono ? MONO_CELL : STACK_CELL;
        const int cx0 = floor_div(x, cell), cz0 = floor_div(z, cell);
        const int rings = max_radius / cell + 1;
        for (int ring = 0; ring <= rings; ring++) {
            for (int dz = -ring; dz <= ring; dz++)
                for (int dx = -ring; dx <= ring; dx++) {
                    if (std::max(std::abs(dx), std::abs(dz)) != ring) continue;
                    int px, pz;
                    if (mono) {
                        // the highest of a few samples over the cell
                        float hb = 0.0f;
                        px = pz = 0;
                        for (int k = 0; k < 64; k++) {
                            const int sx = (cx0 + dx) * cell + (k % 8) * cell / 8 + cell / 16, sz = (cz0 + dz) * cell + (k / 8) * cell / 8 + cell / 16;
                            const float hm = monolith_height(sx, sz);
                            if (hm > hb) { hb = hm; px = sx; pz = sz; }
                        }
                        if (hb <= 0.0f) continue;
                    } else {
                        SeaStack st;
                        if (!stack_at_cell(cx0 + dx, cz0 + dz, st) || (kind == "arch" && !st.arch)) continue;
                        px = (int)st.x;
                        pz = (int)st.z;
                    }
                    const long d2 = (long)(px - x) * (px - x) + (long)(pz - z) * (pz - z);
                    if (best_d2 < 0 || d2 < best_d2) { best_d2 = d2; best = Vector3i(px, height_at(px, pz), pz); }
                }
            if (best_d2 >= 0) break;
        }
        return best;
    }
    if (kind == "sinkhole" || kind == "chimney") {
        // the nearest shaft of that kind under dry land: a sinkhole at its rim, a chimney 4 m into its top
        const int want = kind == "sinkhole" ? MOUTH_SINKHOLE : MOUTH_CHIMNEY;
        const int cx0 = floor_div(x, MOUTH_CELL), cz0 = floor_div(z, MOUTH_CELL);
        const int rings = max_radius / MOUTH_CELL + 1;
        for (int ring = 0; ring <= rings; ring++) {
            for (int dz = -ring; dz <= ring; dz++)
                for (int dx = -ring; dx <= ring; dx++) {
                    if (std::max(std::abs(dx), std::abs(dz)) != ring) continue;
                    CaveMouth m[9];
                    const int n = mouths_near((cx0 + dx) * MOUTH_CELL + MOUTH_CELL / 2, (cz0 + dz) * MOUTH_CELL + MOUTH_CELL / 2, m);
                    for (int i = 0; i < n; i++) {
                        if (m[i].kind != want || floor_div((int)m[i].x, MOUTH_CELL) != cx0 + dx || floor_div((int)m[i].z, MOUTH_CELL) != cz0 + dz) continue;
                        const Column c = column_base((int)m[i].x, (int)m[i].z, false);
                        const Hydro hy = hydro_at((int)m[i].x, (int)m[i].z, c.height);
                        if (c.height <= sea_ + 4 || hy.lake || hy.river > 0.0f || hy.marsh) continue;   // no caves under water
                        long d2 = (long)(m[i].x - x) * (long)(m[i].x - x) + (long)(m[i].z - z) * (long)(m[i].z - z);
                        if (best_d2 < 0 || d2 < best_d2) {
                            best_d2 = d2;
                            best = want == MOUTH_SINKHOLE ? Vector3i((int)(m[i].x + 2.0f * m[i].r + 2.0f), c.height, (int)m[i].z)
                                                          : Vector3i((int)m[i].x, c.height - (int)m[i].top - 8, (int)m[i].z);
                        }
                    }
                }
            if (best_d2 >= 0) break;
        }
        return best;
    }
    if (kind == "geode") {
        const int cx0 = (int)std::floor((float)x / GEODE_CELL), cz0 = (int)std::floor((float)z / GEODE_CELL);
        const int rings = max_radius / GEODE_CELL + 1;
        for (int ring = 0; ring <= rings; ring++) {
            for (int dz = -ring; dz <= ring; dz++)
                for (int dx = -ring; dx <= ring; dx++) {
                    if (std::max(std::abs(dx), std::abs(dz)) != ring) continue;
                    Geode g;
                    if (!geode_at_cell(cx0 + dx, cz0 + dz, g)) continue;
                    long d2 = (long)(g.x - x) * (long)(g.x - x) + (long)(g.z - z) * (long)(g.z - z);
                    if (best_d2 < 0 || d2 < best_d2) { best_d2 = d2; best = Vector3i((int)g.x, (int)(g.y - g.r * 0.4f), (int)g.z); }
                }
            if (best_d2 >= 0) break;
        }
        return best;
    }
    // Conduits (ADR 0110): "aquifer" / "magma_conduit" -> the top of the nearest chamber of that kind
    if (kind == "aquifer" || kind == "magma_conduit") {
        const int want = kind == "aquifer" ? CONDUIT_AQUIFER : CONDUIT_MAGMA;
        const int cx0 = floor_div(x, CONDUIT_CELL), cz0 = floor_div(z, CONDUIT_CELL);
        for (int ring = 0; ring <= max_radius / CONDUIT_CELL + 1; ring++) {
            for (int dz = -ring; dz <= ring; dz++)
                for (int dx = -ring; dx <= ring; dx++) {
                    if (std::max(std::abs(dx), std::abs(dz)) != ring) continue;
                    Conduit c;
                    if (!conduit_at_cell(cx0 + dx, cz0 + dz, c) || c.kind != want) continue;
                    const long d2 = (long)(c.x[0] - x) * (long)(c.x[0] - x) + (long)(c.z[0] - z) * (long)(c.z[0] - z);
                    if (best_d2 < 0 || d2 < best_d2) { best_d2 = d2; best = Vector3i((int)c.x[0], (int)(c.y[0] + c.src_r) - 3, (int)c.z[0]); }
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
                else if (kind == "ravine") {
                    hit = c.height > sea_ + 4 && !hy.lake && hy.river < 0.05f
                          && ravine_value(n_ravmask_.GetNoise((float)px, (float)pz), std::fabs(n_ravine_.GetNoise((float)px, (float)pz)), 30.0f) > 0.0f;
                }
                else if (kind == "cavern") {
                    // a hall at least 10 m tall: the floor of the first such run of cavern 30..150 voxels down
                    if (c.height > sea_ + 4 && !hy.lake && hy.river < 0.05f) {
                        int run = 0;
                        const float pillar = n_pillar_.GetNoise((float)px, (float)pz);   // not inside a hall pillar (ADR 0077)
                        for (int y = c.height - 30; y >= std::max(24, c.height - 150); y--) {
                            if (cavern_value(n_cavern_.GetNoise((float)px, (float)y * CAVERN_Y, (float)pz), (float)y, (float)(c.height - y), pillar) > 0.0f) { run++; continue; }
                            if (run >= 20) { hit = y > sea_ - 180; py = y + 1; break; }
                            run = 0;
                        }
                    }
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
                else if (kind == "delta") { hit = hy.delta > 0.6f && hy.river > 0.8f; py = std::max(hy.bed, hy.water_level); }
                else if (kind == "river") { hit = hy.river > 0.5f && !hy.spring; py = hy.bed; }
                else if (kind == "alpine") { hit = c.alpine && !c.snow; }
                else if (kind == "snow") { hit = c.snow; }
                else if (kind == "mountain") { hit = c.height > sea_ + 200; }
                else if (biome_kind >= 0) {
                    // a real patch of that biome, not one dithered cell: 4 samples 24 voxels out agree
                    // (8 for the shore biomes: a beach is a strip ~20 m wide)
                    const bool strip = biome_kind == technosim_biome::COAST_ID || biome_kind == technosim_biome::COAST_FROZEN_ID
                        || biome_kind == technosim_biome::COAST_STEEP_ID || biome_kind == technosim_biome::MANGROVE;
                    const int po = strip ? 8 : 24;
                    hit = c.biome == biome_kind && !hy.lake && hy.river < 0.05f;
                    for (int q = 0; q < 4 && hit; q++)
                        hit = column_base(px + (q & 1 ? po : -po), pz + (q & 2 ? po : -po), false).biome == biome_kind;
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
