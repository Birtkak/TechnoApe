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
    col.t = n_temp_.GetNoise(fx, fz);
    col.h = n_hum_.GetNoise(fx, fz);

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
    int h = (int)std::lround((float)sea_ + base + relief + detail);
    col.height = std::clamp(h, 8, world_height_ - 8);

    // Biome
    int above = col.height - sea_;
    if (col.c < -0.55f) col.biome = BIOME_DEEP_OCEAN;
    else if (col.height < sea_ - 1) col.biome = BIOME_OCEAN;
    else if (above <= 3) col.biome = BIOME_BEACH;
    else if (above > 300) col.biome = BIOME_SNOWY_PEAKS;
    else if (above > 170) col.biome = BIOME_MOUNTAINS;
    else if (above > 70) col.biome = BIOME_HIGHLANDS;
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
    return d;
}

String WorldGen::biome_name(int b) {
    static const char *names[] = {"deep_ocean", "ocean", "beach", "plains", "forest", "desert", "tundra", "highlands", "mountains", "snowy_peaks"};
    if (b < 0 || b >= BIOME_COUNT) return "unknown";
    return names[b];
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
    for (int z = 0; z < sz; z++) {
        const int wz = origin.z + z * step;
        for (int x = 0; x < sx; x++) {
            const int wx = origin.x + x * step;
            const Column col = column(wx, wz);
            const int h = col.height;
            const bool beach_sand = col.biome == BIOME_BEACH || (col.biome == BIOME_OCEAN && col.c > -0.4f);
            uint16_t *colp = w + sy * (x + sx * z); // y is fastest (VoxelBuffer order)
            for (int y = 0; y < sy; y++) {
                const int wy = origin.y + y * step;
                uint16_t m = AIR;
                if (wy < 3) {
                    m = BEDROCK;
                } else if (wy < h) {
                    const int depth = h - wy;
                    if (do_caves && is_cave(wx, wy, wz, h) && !(wy < sea_ && h < sea_ + 2)) {
                        m = (wy < sea_ - 180) ? LAVA : AIR; // deep caves hold lava pools
                    } else if (depth > 4 + (int)(step > 1)) {
                        m = do_ores ? ore_at(wx, wy, wz) : STONE;
                    } else {
                        // top layers by biome
                        const bool top = depth == 1;
                        switch (col.biome) {
                            case BIOME_DEEP_OCEAN: m = GRAVEL; break;
                            case BIOME_OCEAN: m = beach_sand ? SAND : GRAVEL; break;
                            case BIOME_BEACH: m = SAND; break;
                            case BIOME_DESERT: m = SAND; break;
                            case BIOME_TUNDRA: m = top ? SNOW : DIRT; break;
                            case BIOME_SNOWY_PEAKS: m = top ? SNOW : STONE; break;
                            case BIOME_MOUNTAINS: m = (top && col.pv > 0.2f) ? STONE : (top ? GRASS : DIRT); break;
                            default: m = top ? GRASS : DIRT; break;
                        }
                    }
                } else if (wy < sea_) {
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

PackedByteArray WorldGen::map_image(int center_x, int center_z, int radius, int step) const {
    const int n = std::max(1, 2 * radius / std::max(1, step));
    PackedByteArray img;
    img.resize(n * n * 3);
    uint8_t *p = img.ptrw();
    static const uint8_t colors[BIOME_COUNT][3] = {
        {20, 40, 120}, {40, 80, 200}, {230, 215, 150}, {110, 190, 80}, {40, 130, 50},
        {225, 200, 110}, {200, 210, 220}, {130, 150, 90}, {120, 120, 125}, {245, 245, 250},
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
        }
    return img;
}
