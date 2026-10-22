#pragma once
// Native world generator (Minecraft 1.18 style terrain + real hydrology):
//  - continentalness / erosion / ridged peaks noises through splines -> height
//  - temperature + humidity (jittered) -> biomes with dithered borders
//  - altitude bands on mountains: alpine rock, scree, jittered tree line,
//    permanent snow line (lower where cold); steep slopes expose rock
//  - hydrology on a coarse grid (32-voxel cells, 2 km tiles with 1 km margin):
//    priority-flood depression filling, D8 flow routing, flow accumulation ->
//    springs, streams that merge into collector rivers, basin lakes, outflow
//    to the ocean. River/lake water is generated static (liquid_static) so the
//    material simulation does not drain every slope.
//  - 3D cheese + spaghetti caves with lava pools, ore noise, sea, beaches
//  - trees on a jittered grid, small ruins on flat ground
// Deterministic from a seed. Heights in voxels (0.5 m each, ADR 0004).
// Output blocks are 16-bit material ids in godot_voxel VoxelBuffer order
// (y fastest, then x, then z).

#include <godot_cpp/classes/ref_counted.hpp>
#include <godot_cpp/variant/dictionary.hpp>
#include <godot_cpp/variant/packed_byte_array.hpp>
#include <godot_cpp/variant/vector3i.hpp>

#include "FastNoiseLite.h"

#include <cstdint>
#include <memory>
#include <mutex>
#include <unordered_map>
#include <vector>

namespace godot {

class WorldGen : public RefCounted {
    GDCLASS(WorldGen, RefCounted)

public:
    // Material ids must match game/scripts/materials.gd.
    enum Mat : uint16_t {
        AIR = 0, STONE = 1, DIRT = 2, GRASS = 3, SAND = 4, WATER = 5, WOOD = 6, LAVA = 7, ASH = 8,
        OIL = 9, STEAM = 10, SMOKE = 11, FIRE = 12, BEDROCK = 13, SNOW = 14, GRAVEL = 15,
        COAL_ORE = 16, IRON_ORE = 17, COPPER_ORE = 18, TIN_ORE = 19, LEAVES = 20,
        SAND_STATIC = 21, GRAVEL_STATIC = 22, WATER_STATIC = 23
    };
    enum Biome : int {
        BIOME_DEEP_OCEAN = 0, BIOME_OCEAN = 1, BIOME_BEACH = 2, BIOME_PLAINS = 3, BIOME_FOREST = 4,
        BIOME_DESERT = 5, BIOME_TUNDRA = 6, BIOME_HIGHLANDS = 7, BIOME_MOUNTAINS = 8, BIOME_SNOWY_PEAKS = 9,
        BIOME_RIVER = 10, BIOME_LAKE = 11, BIOME_ALPINE = 12,
        BIOME_COUNT = 13
    };

    WorldGen();

    void configure(int p_seed, int p_sea_level, int p_world_height);
    int get_seed() const { return seed_; }

    // Column queries (include hydrology: rivers and lakes)
    int height_at(int x, int z) const;
    int biome_at(int x, int z) const;
    Dictionary column_info(int x, int z) const;

    // Block generation: returns empty array when the block is entirely air.
    // size may be non-cubic: godot_voxel requests padded mesh regions (e.g. 34x33x34).
    PackedByteArray generate_block(Vector3i origin, Vector3i size, int lod, int bytes_per_voxel = 2) const;

    // Debug: RGB8 image (w = h = 2*radius/step) of biomes shaded by height, with
    // rivers, lakes and ruins overlaid.
    PackedByteArray map_image(int center_x, int center_z, int radius, int step) const;
    static String biome_name(int b);
    // Debug: nearest feature ("lake" | "ruin" | "spring" | "river") within max_radius of (x, z).
    // Returns Vector3i(x, ground_y, z) or Vector3i(0, -1, 0) when none.
    Vector3i find_nearest(const String &kind, int x, int z, int max_radius) const;
    Dictionary hydrology_stats(int x, int z) const; // tile stats around a point (debug)

protected:
    static void _bind_methods();

private:
    // ---- terrain columns ----
    struct Column {
        int height;      // base terrain height (no rivers/lakes)
        int biome;
        float c, e, pv, t, h;
        float slope;     // |dh/dx|, |dh/dz| max (voxels per voxel), only computed on mountains
        bool alpine;     // above the tree line
        bool snow;       // permanent snow
    };
    // ---- hydrology ----
    static constexpr int HCELL = 32;          // voxels per hydrology cell (16 m)
    static constexpr int HTILE = 128;         // cells per tile (4096 voxels = 2 km)
    static constexpr int HMARGIN = 64;        // cells of margin on each side (1 km)
    static constexpr int HWIN = HTILE + 2 * HMARGIN; // window cells per axis
    static constexpr float STREAM_ACC = 18.0f; // cells of drainage to become a stream (18 cells ≈ 4600 m²)
    struct HydroTile {
        int tx, tz;                       // tile coords
        std::vector<float> h;             // base height per window cell (voxels)
        std::vector<float> filled;        // depression-filled surface
        std::vector<float> acc;           // drainage accumulation (cells)
        std::vector<int8_t> dir;          // D8 direction index 0..7, -1 outlet
        std::vector<uint8_t> lake;        // 1 if filled - h > 0.5
        std::vector<float> jx, jz;        // jittered centre offsets (voxels)
    };
    struct Hydro {
        float river;      // 0..1 river mask (1 = centre line)
        int bed;          // river bed height where river > 0
        int water_level;  // water surface for rivers / lakes (>= sea or -1)
        bool lake;
        bool spring;
    };
    // ---- decorations ----
    static constexpr int TREE_CELL = 8;
    static constexpr int TREE_MAX_RADIUS = 3;
    struct Tree { int x, z, base, trunk_h, radius; };
    static constexpr int RUIN_CELL = 160;
    static constexpr int RUIN_MAX_HALF = 10;
    struct Ruin { int x, z, hw, hd, base; uint32_t seed; };

    int seed_ = 1337;
    int sea_ = 256;
    int world_height_ = 768;
    FastNoiseLite n_cont_, n_eros_, n_pv_, n_detail_, n_temp_, n_hum_, n_cheese_, n_spag1_, n_spag2_, n_ore_, n_beach_, n_jitter_;
    mutable std::mutex hydro_mutex_;
    mutable std::unordered_map<uint64_t, std::shared_ptr<HydroTile>> hydro_cache_;

    Column column(int x, int z) const;
    Column column_base(int x, int z, bool with_slope) const;
    static uint32_t hash3(int a, int b, int c);
    static float spline(float v, const float *xs, const float *ys, int n);

    std::shared_ptr<HydroTile> hydro_tile(int tx, int tz) const;
    void build_hydro_tile(HydroTile &t) const;
    Hydro hydro_at(int x, int z, int base_height) const;
    bool hydro_cell(int x, int z, const HydroTile *&tile, int &ci) const; // window cell for world coords

    bool tree_at_cell(int cx, int cz, Tree &out) const;
    int trees_near(int x0, int z0, int x1, int z1, Tree *out, int max_out) const;
    bool ruin_at_cell(int cx, int cz, Ruin &out) const;
    int ruins_near(int x0, int z0, int x1, int z1, Ruin *out, int max_out) const;
    static bool in_ruin(const Ruin &r, int x, int z, int margin);
    inline bool is_cave(int x, int y, int z, int height) const;
    inline uint16_t ore_at(int x, int y, int z) const;
};

} // namespace godot

VARIANT_ENUM_CAST(godot::WorldGen::Biome);
