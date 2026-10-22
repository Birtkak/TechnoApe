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
        COAL_ORE = 16, IRON_ORE = 17, COPPER_ORE = 18, TIN_ORE = 19
    };
    enum Biome : int {
        BIOME_DEEP_OCEAN = 0, BIOME_OCEAN = 1, BIOME_BEACH = 2, BIOME_PLAINS = 3, BIOME_FOREST = 4,
        BIOME_DESERT = 5, BIOME_TUNDRA = 6, BIOME_HIGHLANDS = 7, BIOME_MOUNTAINS = 8, BIOME_SNOWY_PEAKS = 9,
        BIOME_COUNT = 10
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

    // Debug: RGB8 image (w = h = 2*radius/step) of biomes shaded by height around a centre.
    PackedByteArray map_image(int center_x, int center_z, int radius, int step) const;
    static String biome_name(int b);

protected:
    static void _bind_methods();

private:
    struct Column {
        int height;
        int biome;
        float c, e, pv, t, h;
    };
    int seed_ = 1337;
    int sea_ = 256;
    int world_height_ = 768;
    FastNoiseLite n_cont_, n_eros_, n_pv_, n_detail_, n_temp_, n_hum_, n_cheese_, n_spag1_, n_spag2_, n_ore_, n_beach_;

    Column column(int x, int z) const;
    inline bool is_cave(int x, int y, int z, int height) const;
    inline uint16_t ore_at(int x, int y, int z) const;
    static float spline(float v, const float *xs, const float *ys, int n);
};

} // namespace godot

VARIANT_ENUM_CAST(godot::WorldGen::Biome);
