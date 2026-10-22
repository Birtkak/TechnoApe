#pragma once
// Native world generator (Minecraft 1.18 style): continentalness / erosion /
// peaks-and-valleys noises mapped through splines to terrain height,
// temperature + humidity for biomes, 3D cheese + spaghetti caves, ore noise,
// sea, beaches, snow caps. Deterministic from a seed. Heights in voxels
// (0.5 m each, ADR 0004).
//
// Output blocks are 16-bit material ids in godot_voxel VoxelBuffer order
// (y fastest, then x, then z), ready for VoxelBuffer.set_channel_from_byte_array.

#include <godot_cpp/classes/ref_counted.hpp>
#include <godot_cpp/variant/dictionary.hpp>
#include <godot_cpp/variant/packed_byte_array.hpp>
#include <godot_cpp/variant/vector3i.hpp>

#include "FastNoiseLite.h"

#include <cstdint>

namespace godot {

class WorldGen : public RefCounted {
    GDCLASS(WorldGen, RefCounted)

public:
    // Material ids must match game/scripts/materials.gd.
    enum Mat : uint16_t {
        AIR = 0, STONE = 1, DIRT = 2, GRASS = 3, SAND = 4, WATER = 5, WOOD = 6, LAVA = 7, ASH = 8,
        OIL = 9, STEAM = 10, SMOKE = 11, FIRE = 12, BEDROCK = 13, SNOW = 14, GRAVEL = 15,
        COAL_ORE = 16, IRON_ORE = 17, COPPER_ORE = 18, TIN_ORE = 19, LEAVES = 20,
        SAND_STATIC = 21, GRAVEL_STATIC = 22 // natural ground: static until disturbed
    };
    enum Biome : int {
        BIOME_DEEP_OCEAN = 0, BIOME_OCEAN = 1, BIOME_BEACH = 2, BIOME_PLAINS = 3, BIOME_FOREST = 4,
        BIOME_DESERT = 5, BIOME_TUNDRA = 6, BIOME_HIGHLANDS = 7, BIOME_MOUNTAINS = 8, BIOME_SNOWY_PEAKS = 9,
        BIOME_RIVER = 10,
        BIOME_COUNT = 11
    };

    WorldGen();

    void configure(int p_seed, int p_sea_level, int p_world_height);
    int get_seed() const { return seed_; }

    // Column queries
    int height_at(int x, int z) const;
    int biome_at(int x, int z) const;
    Dictionary column_info(int x, int z) const;

    // Block generation: returns empty array when the block is entirely air.
    // size may be non-cubic: godot_voxel requests padded mesh regions (e.g. 34x33x34).
    PackedByteArray generate_block(Vector3i origin, Vector3i size, int lod, int bytes_per_voxel = 2) const;

    // Debug: RGB8 image (w = h = 2*radius/step) of biomes shaded by height around a centre,
    // with lakes (light blue) and ruins (dark grey) overlaid.
    PackedByteArray map_image(int center_x, int center_z, int radius, int step) const;
    static String biome_name(int b);
    // Debug: nearest decoration of a kind ("lake" | "ruin") within max_radius voxels of (x, z).
    // Returns Vector3i(x, ground_y, z) or Vector3i(0, -1, 0) when none.
    Vector3i find_nearest(const String &kind, int x, int z, int max_radius) const;

protected:
    static void _bind_methods();

private:
    struct Column {
        int height;
        int biome;
        float c, e, pv, t, h;
        float river; // 0 = none, 1 = river centre
    };
    // Trees are placed on a jittered grid of TREE_CELL voxels; one hash per cell
    // decides existence, position, height and canopy, so any block can stamp them.
    static constexpr int TREE_CELL = 8;
    static constexpr int TREE_MAX_RADIUS = 3;
    struct Tree {
        int x, z, base, trunk_h, radius;
    };
    // Lakes above sea level: one candidate per LAKE_CELL cell; surface level is
    // sampled from the basin rim so no water floats at the edge.
    static constexpr int LAKE_CELL = 192;
    static constexpr int LAKE_MAX_RADIUS = 56;
    struct Lake {
        int x, z, radius, level, depth;
    };
    // Ruins: small broken stone enclosures, one candidate per RUIN_CELL cell.
    static constexpr int RUIN_CELL = 160;
    static constexpr int RUIN_MAX_HALF = 10;
    struct Ruin {
        int x, z, hw, hd, base;
        uint32_t seed;
    };
    int seed_ = 1337;
    int sea_ = 256;
    int world_height_ = 768;
    FastNoiseLite n_cont_, n_eros_, n_pv_, n_detail_, n_temp_, n_hum_, n_cheese_, n_spag1_, n_spag2_, n_ore_, n_beach_, n_river_, n_jitter_;

    Column column(int x, int z) const;
    static uint32_t hash3(int a, int b, int c);
    bool tree_at_cell(int cx, int cz, Tree &out) const;
    int trees_near(int x0, int z0, int x1, int z1, Tree *out, int max_out) const;
    bool lake_at_cell(int cx, int cz, Lake &out) const;
    int lakes_near(int x0, int z0, int x1, int z1, Lake *out, int max_out) const;
    // Applies lakes to a column: lowers height inside the basin, raises a small
    // berm at the rim, returns the water level (>= sea) and whether this is lake bed.
    static void apply_lakes(int x, int z, const Lake *lakes, int n, int &height, int &water_level, bool &lakebed);
    bool ruin_at_cell(int cx, int cz, Ruin &out) const;
    int ruins_near(int x0, int z0, int x1, int z1, Ruin *out, int max_out) const;
    static bool in_ruin(const Ruin &r, int x, int z, int margin);
    inline bool is_cave(int x, int y, int z, int height) const;
    inline uint16_t ore_at(int x, int y, int z) const;
    static float spline(float v, const float *xs, const float *ys, int n);
};

} // namespace godot

VARIANT_ENUM_CAST(godot::WorldGen::Biome);
