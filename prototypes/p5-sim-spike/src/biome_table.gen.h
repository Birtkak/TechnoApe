// GENERATED from game/data/biomes.json by game/tools/biome_codegen.gd. Do not edit.
// Biome ids, names, surface materials and the climate tables the generator uses
// (see docs/design/biomes.md). Regenerate:
//   Godot_console.exe --headless --path game --script res://tools/biome_codegen.gd
#pragma once

namespace technosim_biome {

enum Id : int {
    DEEP_OCEAN = 0,
    OCEAN = 1,
    BEACH = 2,
    PLAINS = 3,
    FOREST = 4,
    DESERT = 5,
    TUNDRA = 6,
    HIGHLANDS = 7,
    MOUNTAINS = 8,
    SNOWY_PEAKS = 9,
    RIVER = 10,
    LAKE = 11,
    ALPINE = 12,
    FROZEN_OCEAN = 13,
    SNOWY_BEACH = 14,
    STONY_SHORE = 15,
    ICE_FIELDS = 16,
    SNOWY_TUNDRA = 17,
    SNOWY_TAIGA = 18,
    TAIGA = 19,
    SHRUBLAND = 20,
    SWAMP = 21,
    SAVANNA = 22,
    JUNGLE = 23,
    FROZEN_HIGHLANDS = 24,
    BADLANDS = 25,
    OLD_GROWTH_FOREST = 26,
    CAVE = 27,
    LUSH_CAVE = 28,
    DRIPSTONE_CAVE = 29,
    FUNGAL_CAVE = 30,
    FROZEN_CAVE = 31,
    DEEP_CAVE = 32,
    DEEP_DARK = 33,
    CRYSTAL_CAVE = 34,
    MAGMA_CAVE = 35,
    SULFUR_CAVE = 36,
    COUNT = 37
};

static const char *const NAMES[COUNT] = {"deep_ocean", "ocean", "beach", "plains", "forest", "desert", "tundra", "highlands", "mountains", "snowy_peaks", "river", "lake", "alpine", "frozen_ocean", "snowy_beach", "stony_shore", "ice_fields", "snowy_tundra", "snowy_taiga", "taiga", "shrubland", "swamp", "savanna", "jungle", "frozen_highlands", "badlands", "old_growth_forest", "cave", "lush_cave", "dripstone_cave", "fungal_cave", "frozen_cave", "deep_cave", "deep_dark", "crystal_cave", "magma_cave", "sulfur_cave"};
static const unsigned char COLORS[COUNT][3] = {{10, 30, 90}, {30, 70, 160}, {230, 220, 150}, {120, 190, 80}, {40, 120, 40}, {235, 210, 120}, {170, 190, 200}, {100, 140, 70}, {130, 120, 110}, {245, 245, 250}, {60, 120, 220}, {90, 160, 230}, {160, 150, 140}, {150, 190, 220}, {220, 225, 235}, {120, 125, 130}, {200, 230, 245}, {225, 235, 240}, {120, 160, 150}, {60, 110, 80}, {170, 180, 90}, {70, 100, 60}, {190, 180, 80}, {30, 140, 50}, {190, 205, 215}, {200, 120, 70}, {25, 90, 35}, {90, 90, 95}, {60, 150, 90}, {150, 120, 90}, {120, 80, 140}, {180, 210, 230}, {50, 50, 60}, {20, 30, 50}, {170, 120, 220}, {200, 70, 30}, {205, 190, 60}};
static const float TREE_DENSITY[COUNT] = {0.0f, 0.0f, 0.0f, 0.06f, 0.55f, 0.0f, 0.08f, 0.18f, 0.12f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.02f, 0.35f, 0.45f, 0.02f, 0.3f, 0.022f, 0.7f, 0.1f, 0.0f, 0.65f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f};
// Tree shape and wood: 0 oak, 1 spruce, 2 acacia, 3 jungle, 4 birch.
static const int TREE_KIND[COUNT] = {0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0, 2, 3, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0};
// A second tree kind mixed in (biomes.json tree_mix): which, and its share of the trees.
static const int TREE_MIX_KIND[COUNT] = {0, 0, 0, 4, 4, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0};
static const float TREE_MIX[COUNT] = {0.0f, 0.0f, 0.0f, 0.25f, 0.3f, 0.0f, 0.0f, 0.15f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.1f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f};
// Ground decorations (biomes.json decor): chance per 8 m cell (one of bush / boulder / dead tree / fallen log),
// spire chance per 12 m cell, its material (TERRACOTTA = strata) and height range in voxels.
// Wild food (biomes.json decor): blueberry share of bushes, mushrooms per floor clutter cell and per mossy log voxel.
static const float DECOR_BUSH[COUNT] = {0.0f, 0.0f, 0.0f, 0.08f, 0.35f, 0.0f, 0.0f, 0.05f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.15f, 0.25f, 0.2f, 0.06f, 0.8f, 0.0f, 0.0f, 0.4f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f};
static const float DECOR_BOULDER[COUNT] = {0.0f, 0.0f, 0.0f, 0.04f, 0.05f, 0.0f, 0.08f, 0.15f, 0.1f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.05f, 0.08f, 0.12f, 0.06f, 0.0f, 0.0f, 0.0f, 0.1f, 0.0f, 0.15f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f};
static const float DECOR_DEAD_TREE[COUNT] = {0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.02f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.04f, 0.1f, 0.05f, 0.0f, 0.0f, 0.03f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f};
static const float DECOR_FALLEN_LOG[COUNT] = {0.0f, 0.0f, 0.0f, 0.0f, 0.08f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.08f, 0.0f, 0.08f, 0.0f, 0.06f, 0.0f, 0.0f, 0.15f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f};
static const float DECOR_SPIRE[COUNT] = {0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.04f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.375f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.25f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f};
static const float DECOR_BERRIES[COUNT] = {0.0f, 0.0f, 0.0f, 0.056f, 0.056f, 0.0f, 0.056f, 0.056f, 0.056f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.056f, 0.056f, 0.056f, 0.056f, 0.056f, 0.0f, 0.056f, 0.056f, 0.0f, 0.056f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f};
static const float DECOR_MUSHROOMS[COUNT] = {0.0f, 0.0f, 0.0f, 0.0f, 0.031f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.031f, 0.031f, 0.0f, 0.047f, 0.0f, 0.031f, 0.0f, 0.0f, 0.047f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f};
static const float DECOR_LOG_MUSHROOMS[COUNT] = {0.0f, 0.0f, 0.0f, 0.0f, 0.012f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.012f, 0.012f, 0.0f, 0.017f, 0.0f, 0.012f, 0.0f, 0.0f, 0.017f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f};
static const int SPIRE_MATERIAL[COUNT] = {-1, -1, -1, -1, -1, 24, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 86, -1, -1, -1, -1, -1, -1, -1, -1, 82, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1};
static const int SPIRE_HEIGHT[COUNT][2] = {{0, 0}, {0, 0}, {0, 0}, {0, 0}, {0, 0}, {4, 12}, {0, 0}, {0, 0}, {0, 0}, {0, 0}, {0, 0}, {0, 0}, {0, 0}, {0, 0}, {0, 0}, {0, 0}, {6, 24}, {0, 0}, {0, 0}, {0, 0}, {0, 0}, {0, 0}, {0, 0}, {0, 0}, {0, 0}, {8, 26}, {0, 0}, {0, 0}, {0, 0}, {0, 0}, {0, 0}, {0, 0}, {0, 0}, {0, 0}, {0, 0}, {0, 0}, {0, 0}};
// Material ids (WorldGen::Material); -1 = none.
static const int SURFACE_TOP[COUNT] = {22, 21, 21, 3, 3, 21, 14, 3, 3, 14, 22, 22, 22, 22, 14, 1, 14, 14, 14, 3, 3, 3, 127, 3, 14, 22, 3, 1, 3, 1, 2, 14, 1, 1, 1, 85, 55};
static const int SURFACE_UNDER[COUNT] = {22, 22, 21, 2, 2, 21, 2, 2, 2, 1, 22, 22, 1, 22, 21, 1, 86, 2, 2, 2, 2, 2, 129, 2, 2, 82, 2, 1, 1, 1, 1, 1, 1, 1, 1, 85, 1};
static const int SURFACE_STEEP[COUNT] = {-1, -1, -1, -1, -1, -1, -1, -1, 1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 82, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1};
// Caves: the ceiling over a cave cell and static pools on cave floors (-1 = plain stone, no pools).
static const int SURFACE_CEILING[COUNT] = {-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 85, 55};
static const int SURFACE_POOL[COUNT] = {-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 23, -1, -1, -1, -1, -1, -1, 23, -1, -1, -1, -1, -1, -1, 121, 54};
// What the top voxel of open water freezes into (-1 = it stays water).
static const int SURFACE_ICE[COUNT] = {-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 30, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1};

// Climate: temperature = clamp(NOISE_WEIGHT * temp_noise - LATITUDE_WEIGHT * clamp(north / LATITUDE_FULL_VOXELS, -1, 1), -1, 1)
// with north = -z, then minus LAPSE_PER_VOXEL per voxel above LAPSE_START_ABOVE_SEA.
static const bool NORTH_IS_NEGATIVE_Z = true;
static const float LATITUDE_FULL_VOXELS = 32000.0f;
static const float LATITUDE_WEIGHT = 0.8f;
static const float NOISE_WEIGHT = 0.5f;
static const int LAPSE_START_ABOVE_SEA = 70;
static const float LAPSE_PER_VOXEL = 0.0015f;
static const float JITTER = 0.09f;
static const int BAND_COUNT = 5; // temperature: frozen, cold, temperate, warm, hot; humidity: arid, dry, neutral, humid, wet
static const float TEMP_BAND_MAX[BAND_COUNT] = {-0.6f, -0.2f, 0.2f, 0.55f, 1.01f}; // band i = first i with value < max
static const float HUM_BAND_MAX[BAND_COUNT] = {-0.5f, -0.15f, 0.15f, 0.5f, 1.01f};

// Altitude (voxels above sea level)
static const float DEEP_OCEAN_MAX_CONTINENTALNESS = -0.55f;
static const int COAST_MAX = 3;
static const int LOWLAND_MAX = 70;
static const int HIGHLAND_MAX = 170;
static const int TREE_LINE_BASE = 220;  // tree_line = base - cold_shift * (-temperature)
static const int TREE_LINE_COLD_SHIFT = 40;
static const int SNOW_LINE_BASE = 330;
static const int SNOW_LINE_COLD_SHIFT = 90;
static const float STONY_SHORE_MIN_SLOPE = 0.6f;

// Tables [temperature band][humidity band] -> Id
static const int LOWLAND[BAND_COUNT][BAND_COUNT] = {
    {16, 17, 17, 18, 18}, // frozen
    {6, 6, 19, 19, 19}, // cold
    {20, 3, 3, 4, 26}, // temperate
    {22, 22, 3, 4, 23}, // warm
    {5, 5, 22, 23, 23}, // hot
};
static const int HIGHLAND[BAND_COUNT][BAND_COUNT] = {
    {24, 24, 24, 24, 24}, // frozen
    {7, 7, 7, 7, 7}, // cold
    {7, 7, 7, 7, 7}, // temperate
    {7, 7, 7, 7, 7}, // warm
    {25, 25, 7, 7, 7}, // hot
};
// Lowland overrides, checked in order before LOWLAND: bit masks over band indices.
struct Override { int biome; int temp_mask; int hum_mask; int max_above_sea; float min_erosion; };
static const int LOWLAND_OVERRIDE_COUNT = 1;
static const Override LOWLAND_OVERRIDES[1] = {{21, 12, 16, 24, 0.05f}};

// Underground layer: depth = surface height - y. Below SOIL_DEPTH a cave biome
// comes from CAVE_TABLE[depth band][surface humidity band] after CAVE_OVERRIDES.
static const int SOIL_DEPTH = 6;
static const int DEPTH_BAND_COUNT = 3; // shallow, mid, deep
static const float DEPTH_BAND_MAX[DEPTH_BAND_COUNT] = {60.0f, 160.0f, 100000.0f};
static const int CAVE_NOISE_BAND_COUNT = 3; // low, mid, high
static const float CAVE_NOISE_BAND_MAX[CAVE_NOISE_BAND_COUNT] = {-0.35f, 0.45f, 1.01f};
static const float CAVE_NOISE_PERIOD_M = 120.0f;
static const int CAVE_TABLE[DEPTH_BAND_COUNT][BAND_COUNT] = {
    {29, 29, 27, 28, 28}, // shallow
    {29, 27, 27, 28, 30}, // mid
    {32, 32, 32, 32, 32}, // deep
};
struct CaveOverride { int biome; int depth_mask; int temp_mask; int hum_mask; int noise_mask; };
static const int CAVE_OVERRIDE_COUNT = 5;
static const CaveOverride CAVE_OVERRIDES[5] = {{31, 1, 1, 31, 7}, {35, 4, 16, 31, 7}, {33, 4, 31, 31, 1}, {34, 4, 31, 31, 4}, {36, 2, 31, 31, 4}};

static const int MOUNTAIN_ID = 8;
static const int ALPINE_ID = 12;
static const int SNOW_ID = 9;
static const int DEEP_OCEAN_ID = 0;
static const int OCEAN_ID = 1;
static const int OCEAN_FROZEN_ID = 13;
static const int COAST_ID = 2;
static const int COAST_FROZEN_ID = 14;
static const int COAST_STEEP_ID = 15;
static const int RIVER_ID = 10;
static const int LAKE_ID = 11;

// Reference classifier (mirrors game/scripts/world/biome_rules.gd::classify).
inline int band_of(const float *maxes, float v, int count = BAND_COUNT) {
    for (int i = 0; i < count; i++) if (v < maxes[i]) return i;
    return count - 1;
}
inline float temperature(float noise_t, float z, int above_sea) {
    float north = NORTH_IS_NEGATIVE_Z ? -z : z;
    float lat = north / LATITUDE_FULL_VOXELS;
    lat = lat < -1.0f ? -1.0f : (lat > 1.0f ? 1.0f : lat);
    float t = NOISE_WEIGHT * noise_t - LATITUDE_WEIGHT * lat;
    if (above_sea > LAPSE_START_ABOVE_SEA) t -= (float)(above_sea - LAPSE_START_ABOVE_SEA) * LAPSE_PER_VOXEL;
    return t < -1.0f ? -1.0f : (t > 1.0f ? 1.0f : t);
}
inline int tree_line(float t) { return TREE_LINE_BASE - (int)((float)TREE_LINE_COLD_SHIFT * (-t)); }
inline int snow_line(float t) { return SNOW_LINE_BASE - (int)((float)SNOW_LINE_COLD_SHIFT * (-t)); }
// t, h: final climate; above: height - sea; river mask 0..1; lake: under lake water.
inline int classify(float t, float h, int above, float continentalness, float erosion, float slope, float river, bool lake) {
    if (lake) return LAKE_ID;
    if (river > 0.5f) return RIVER_ID;
    const int ti = band_of(TEMP_BAND_MAX, t), hi = band_of(HUM_BAND_MAX, h);
    const bool frozen = ti == 0;
    if (continentalness < DEEP_OCEAN_MAX_CONTINENTALNESS) return DEEP_OCEAN_ID;
    if (above < -1) return frozen ? OCEAN_FROZEN_ID : OCEAN_ID;
    if (above <= COAST_MAX) return slope >= STONY_SHORE_MIN_SLOPE ? COAST_STEEP_ID : (frozen ? COAST_FROZEN_ID : COAST_ID);
    if (above > snow_line(t)) return SNOW_ID;
    if (above > tree_line(t)) return ALPINE_ID;
    if (above > HIGHLAND_MAX) return MOUNTAIN_ID;
    if (above > LOWLAND_MAX) return HIGHLAND[ti][hi];
    for (int i = 0; i < LOWLAND_OVERRIDE_COUNT; i++) {
        const Override &o = LOWLAND_OVERRIDES[i];
        if ((o.temp_mask >> ti) & 1 && (o.hum_mask >> hi) & 1 && above <= o.max_above_sea && erosion >= o.min_erosion) return o.biome;
    }
    return LOWLAND[ti][hi];
}
// Cave biome from band indices; cheap enough per voxel (bands change every few metres).
inline int cave_biome(int ti, int hi, int di, int ni) {
    for (int i = 0; i < CAVE_OVERRIDE_COUNT; i++) {
        const CaveOverride &o = CAVE_OVERRIDES[i];
        if ((o.depth_mask >> di) & 1 && (o.temp_mask >> ti) & 1 && (o.hum_mask >> hi) & 1 && (o.noise_mask >> ni) & 1) return o.biome;
    }
    return CAVE_TABLE[di][hi];  // CAVE_TABLE: the enum already has Id::CAVE
}
// surface_biome = classify(...) for the column; depth = surface height - y; cave_noise: slow 3D noise -1..1.
inline int classify_3d(int surface_biome, float t, float h, int depth, float cave_noise) {
    if (depth < SOIL_DEPTH) return surface_biome;
    return cave_biome(band_of(TEMP_BAND_MAX, t), band_of(HUM_BAND_MAX, h), band_of(DEPTH_BAND_MAX, (float)depth, DEPTH_BAND_COUNT), band_of(CAVE_NOISE_BAND_MAX, cave_noise, CAVE_NOISE_BAND_COUNT));
}

} // namespace technosim_biome
