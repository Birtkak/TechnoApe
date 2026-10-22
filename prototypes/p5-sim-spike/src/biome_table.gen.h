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
    COUNT = 27
};

static const char *const NAMES[COUNT] = {"deep_ocean", "ocean", "beach", "plains", "forest", "desert", "tundra", "highlands", "mountains", "snowy_peaks", "river", "lake", "alpine", "frozen_ocean", "snowy_beach", "stony_shore", "ice_fields", "snowy_tundra", "snowy_taiga", "taiga", "shrubland", "swamp", "savanna", "jungle", "frozen_highlands", "badlands", "old_growth_forest"};
static const unsigned char COLORS[COUNT][3] = {{10, 30, 90}, {30, 70, 160}, {230, 220, 150}, {120, 190, 80}, {40, 120, 40}, {235, 210, 120}, {170, 190, 200}, {100, 140, 70}, {130, 120, 110}, {245, 245, 250}, {60, 120, 220}, {90, 160, 230}, {160, 150, 140}, {150, 190, 220}, {220, 225, 235}, {120, 125, 130}, {200, 230, 245}, {225, 235, 240}, {120, 160, 150}, {60, 110, 80}, {170, 180, 90}, {70, 100, 60}, {190, 180, 80}, {30, 140, 50}, {190, 205, 215}, {200, 120, 70}, {25, 90, 35}};
static const float TREE_DENSITY[COUNT] = {0.0f, 0.0f, 0.0f, 0.06f, 0.55f, 0.0f, 0.08f, 0.18f, 0.12f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.02f, 0.35f, 0.45f, 0.02f, 0.3f, 0.04f, 0.7f, 0.1f, 0.0f, 0.65f};
// Material ids (WorldGen::Material); -1 = none.
static const int SURFACE_TOP[COUNT] = {22, 21, 21, 3, 3, 21, 14, 3, 3, 14, 22, 22, 22, 22, 14, 1, 14, 14, 14, 3, 3, 3, 3, 3, 14, 22, 3};
static const int SURFACE_UNDER[COUNT] = {22, 22, 21, 2, 2, 21, 2, 2, 2, 1, 22, 22, 1, 22, 21, 1, 14, 2, 2, 2, 2, 2, 2, 2, 2, 21, 2};
static const int SURFACE_STEEP[COUNT] = {-1, -1, -1, -1, -1, -1, -1, -1, 1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1};

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
static const int TREE_LINE_COLD_SHIFT = 60;
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
inline int band_of(const float *maxes, float v) {
    for (int i = 0; i < BAND_COUNT; i++) if (v < maxes[i]) return i;
    return BAND_COUNT - 1;
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

} // namespace technosim_biome
