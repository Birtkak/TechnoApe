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
//  - 3D cheese + spaghetti caves with lava pools, sea, beaches
//  - GregTech-style ore veins: one mixed-ore lens per 96 m cell at most, from
//    game/data/ore_veins.json through ore_veins.gen.h, with ore-bearing
//    stones on the ground above shallow veins
//  - waterfalls: where a stream drops >= FALL_MIN voxels between cells, the
//    ground along the segment is sampled and a static water sheet is hung
//    over the steepest part (a real cliff); marshes in shallow humid hollows,
//    ice on cold lakes, sandstone under deserts, clay in beds
//  - trees on a jittered grid, small ruins on flat ground, per-column
//    vegetation and detail at LOD 0 (tall grass, flowers, cacti, dead bushes,
//    boulders, pebbles, flint, twigs, mushrooms, reeds and lily pads in marshes) and cave
//    detail on floors and ceilings (stalagmites, stalactites, mushrooms)
// Deterministic from a seed. Heights in voxels (0.5 m each, ADR 0004).
// Output blocks are 16-bit material ids in godot_voxel VoxelBuffer order
// (y fastest, then x, then z).

#include <godot_cpp/classes/ref_counted.hpp>
#include <godot_cpp/variant/array.hpp>
#include <godot_cpp/variant/dictionary.hpp>
#include <godot_cpp/variant/packed_byte_array.hpp>
#include <godot_cpp/variant/vector3i.hpp>

#include "FastNoiseLite.h"
#include "biome_table.gen.h" // generated from game/data/biomes.json (36 biomes, classify(), classify_3d())
#include "ore_veins.gen.h"   // generated from game/data/ore_veins.json (vein types, grid, indicators)

#include <cstdint>
#include <memory>
#include <future>
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
        SAND_STATIC = 21, GRAVEL_STATIC = 22, WATER_STATIC = 23,
        SANDSTONE = 24, CLAY = 25, TALL_GRASS = 26, FLOWER = 27, CACTUS = 28, DEAD_BUSH = 29, ICE = 30, FLOWER_RED = 31,
        // 32..36 station blocks (ADR 0007, never generated), then micro-voxel detail (ADR 0006 grid)
        PEBBLES = 37, MUSHROOM_BROWN = 38, MUSHROOM_RED = 39, REED = 40, LILY_PAD = 41, STALACTITE = 42, STALAGMITE = 43,
        CHISELED = 44, // reserved, never generated
        GLOW_MUSHROOM = 45, MOSS = 46, // deep / lush cave detail
        // 47 blood (never generated); ground pickups (ADR 0019)
        COPPER_BEARING_STONES = 48, TIN_BEARING_STONES = 49, IRON_BEARING_STONES = 50, FLINT_STONES = 51, TWIGS = 52,
        TOXIC_SLUDGE = 53, TOXIC_SLUDGE_STATIC = 54, SULFUR = 55, OBSIDIAN = 56,
        // 57..72 wool, carpet, farmland, crops (never generated); rare wild crops (ADR 0022)
        WILD_WHEAT = 73, WILD_CARROT = 74,
        // 75 furniture (never generated); content pass: tree kinds, badlands strata, basalt, ice fields, ferns
        SPRUCE_LOG = 76, SPRUCE_LEAVES = 77, ACACIA_LOG = 78, ACACIA_LEAVES = 79, JUNGLE_LOG = 80, JUNGLE_LEAVES = 81,
        TERRACOTTA = 82, TERRACOTTA_RED = 83, TERRACOTTA_CREAM = 84, BASALT = 85, PACKED_ICE = 86, FERN = 87,
        // 88..89 work pieces (ADR 0023, never generated); content pass 2
        BIRCH_LOG = 90, BIRCH_LEAVES = 91, BLUEBERRY_BUSH = 92, PUMPKIN = 93, CORNFLOWER = 94, OXEYE_DAISY = 95, ALLIUM = 96,
        OAK_BARK = 97, SPRUCE_BARK = 98, ACACIA_BARK = 99, JUNGLE_BARK = 100, BIRCH_BARK = 101,
        CARCASS = 102   // wildlife (ADR 0027)
    };
    // The first 13 ids are stable (bound to GDScript); the rest come from
    // biomes.json through biome_table.gen.h (technosim_biome::Id, NAMES, COLORS,
    // SURFACE_* and TREE_DENSITY tables, classify() / classify_3d()).
    enum Biome : int {
        BIOME_DEEP_OCEAN = 0, BIOME_OCEAN = 1, BIOME_BEACH = 2, BIOME_PLAINS = 3, BIOME_FOREST = 4,
        BIOME_DESERT = 5, BIOME_TUNDRA = 6, BIOME_HIGHLANDS = 7, BIOME_MOUNTAINS = 8, BIOME_SNOWY_PEAKS = 9,
        BIOME_RIVER = 10, BIOME_LAKE = 11, BIOME_ALPINE = 12,
        BIOME_COUNT = 37
    };
    static_assert(BIOME_COUNT == technosim_biome::COUNT, "biome_table.gen.h changed: regenerate and update BIOME_COUNT");

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
    // Debug: nearest feature ("lake" | "ruin" | "spring" | "river" | "waterfall" | "marsh" |
    // "alpine" | "snow" | "mountain" | any biome name) within max_radius of (x, z).
    // Returns Vector3i(x, ground_y, z) or Vector3i(0, -1, 0) when none.
    PackedByteArray climate_map(int x0, int z0, int n, int step) const;
    PackedInt32Array water_floor_map(int x0, int z0, int n, int step) const;
    Vector3i find_nearest(const String &kind, int x, int z, int max_radius) const;
    // Waterfalls within radius voxels of (x, z): [{x, y, z, top, dir_x, dir_z}] with (x, y, z) the
    // first air voxel at the foot of the sheet and (dir_x, dir_z) the flow direction (unit).
    Array waterfalls_near(int x, int z, int radius) const;
    Dictionary hydrology_stats(int x, int z) const; // tile stats around a point (debug)
    // Debug: the ore vein of the cell holding (x, z): {name, type, x, y, z, size, thickness,
    // half_thickness (lens thickness / 2 at this column, 0 outside), indicator} or {} when none.
    Dictionary vein_at(int x, int z) const;

protected:
    static void _bind_methods();

private:
    // ---- terrain columns ----
    struct Column {
        int height;      // base terrain height (no rivers/lakes)
        int biome;
        float c, e, pv, t, h; // t is the final climate (noise + latitude + lapse), see biomes.md
        float t_noise;        // temperature noise + jitter before latitude and lapse (debug / probes)
        float slope;     // |dh/dx|, |dh/dz| max (voxels per voxel), only computed on mountains
        bool alpine;     // above the tree line
        bool snow;       // permanent snow
    };
    // ---- hydrology ----
    static constexpr int HCELL = 32;          // voxels per hydrology cell (16 m)
    static constexpr int HTILE = 128;         // cells per tile (4096 voxels = 2 km)
    static constexpr int HMARGIN = 96;        // cells of margin on each side (1.5 km)
    static constexpr int HWIN = HTILE + 2 * HMARGIN; // window cells per axis
    friend struct HydroCtx;
    static constexpr int HBLEND = 12;         // cells inside a tile's core border where accumulation blends with the neighbour tile
    static constexpr float STREAM_ACC = 18.0f; // cells of drainage to become a stream (18 cells ≈ 4600 m²)
    static constexpr float LAKE_MIN_DEPTH = 5.0f; // filled - h (voxels) for a depression to hold a lake
    static constexpr float MARSH_MIN_DEPTH = 1.5f; // shallower humid depressions become marsh
    static constexpr float FALL_MIN = 10.0f;  // drop (voxels) between stream cells that makes a waterfall
    struct HydroTile {
        int tx, tz;                       // tile coords
        std::vector<float> h;             // base height per window cell (voxels)
        std::vector<float> filled;        // depression-filled surface
        std::vector<float> acc;           // drainage accumulation (cells)
        std::vector<int8_t> dir;          // D8 direction index 0..7, -1 outlet
        std::vector<uint8_t> lake;        // 1 if filled - h > LAKE_MIN_DEPTH (and above sea)
        std::vector<float> jx, jz;        // jittered centre offsets (voxels)
        std::vector<float> fall_t0, fall_t1; // waterfall band along the cell's segment (segment parameter), -1 none
        std::vector<int16_t> fall_top;    // water sheet top (exclusive) at that band
    };
    struct Hydro {
        float river;      // 0..1 river mask (1 = centre line)
        int bed;          // river bed height where river > 0
        int water_level;  // water surface for rivers / lakes (>= sea or -1)
        bool lake;
        bool spring;
        bool marsh;       // shallow humid depression: flat wet floor at marsh_level with puddles
        bool puddle;      // this marsh column is a puddle (water_level = marsh_level)
        int marsh_level;  // marsh floor / water surface (exclusive), -1 none
        int fall_top;     // waterfall sheet: static water from water_level up to this (exclusive), -1 none
        bool shore;       // dry ground within ~1 m of a lake or river surface (reeds)
    };
    // Which tile owns a column, plus the neighbour tiles its accumulation blends with
    // near the core border (so river width does not jump at tile seams).
    struct HydroCtx {
        const HydroTile *t = nullptr;
        int ci = 0;                   // window cell index in t
        const HydroTile *bx = nullptr, *bz = nullptr;
        float wx = 0.0f, wz = 0.0f;   // blend weights (0 .. 0.5)
        float acc(int k) const;       // blended accumulation of owner cell k
    };
    // ---- decorations ----
    static constexpr int TREE_CELL = 8;
    static constexpr int TREE_MAX_RADIUS = 4;
    // kind = technosim_biome::TREE_KIND / TREE_MIX_KIND (0 oak, 1 spruce, 2 acacia, 3 jungle, 4 birch): shape and wood
    enum TreeKind : int { TREE_OAK = 0, TREE_SPRUCE = 1, TREE_ACACIA = 2, TREE_JUNGLE = 3, TREE_BIRCH = 4 };
    struct Tree { int x, z, base, trunk_h, radius, kind; };
    static int canopy_radius(const Tree &t, int wy);   // leaf radius of the crown layer at wy, -1 = none
    static int trunk_top(const Tree &t);               // highest log voxel
    static uint16_t tree_log(int kind);
    static uint16_t tree_leaves(int kind);
    static uint16_t tree_bark(int kind);
    // Ground decorations (biomes.json decor, DECOR_* tables): at most one per 8 m cell,
    // anchored 2..13 voxels into the cell so it never reaches past a neighbour cell.
    static constexpr int STAMP_CELL = 16;
    static constexpr int STAMP_REACH = 3;
    enum StampKind : int { STAMP_BUSH = 1, STAMP_BOULDER = 2, STAMP_DEAD_TREE = 3, STAMP_FALLEN_LOG = 4 };
    struct Stamp { int kind, x, z, size, tree_kind; bool mossy; uint32_t seed; };
    bool stamp_at_cell(int cx, int cz, Stamp &out) const;
    int stamps_near(int x0, int z0, int x1, int z1, Stamp *out, int max_out) const;
    static int stamp_height(const Stamp &s);                  // voxels above the ground it can fill
    static uint16_t stamp_voxel(const Stamp &s, int x, int z, int dy); // dy = 0: the first voxel above the ground
    static constexpr int RUIN_CELL = 160;
    static constexpr int RUIN_MAX_HALF = 10;
    struct Ruin { int x, z, hw, hd, base; uint32_t seed; };

    int seed_ = 1337;
    int sea_ = 256;
    int world_height_ = 768;
    FastNoiseLite n_cont_, n_eros_, n_pv_, n_detail_, n_temp_, n_hum_, n_cheese_, n_spag1_, n_spag2_, n_ore_, n_beach_, n_jitter_, n_cavebiome_, n_pool_;
    // Global tile cache: a tile is built outside the lock by the first thread
    // that asks for it, others wait on its future; least recently used tiles
    // are evicted past HYDRO_CACHE_MAX (ADR 0020: a build under the lock
    // serialised every worker during the first load, and the old wholesale
    // clear rebuilt a region's tiles again and again when flying far).
    static constexpr size_t HYDRO_CACHE_MAX = 96;   // ~2.8 MB each
    struct HydroEntry { std::shared_future<std::shared_ptr<HydroTile>> tile; uint64_t used; };
    mutable std::mutex hydro_mutex_;
    mutable std::unordered_map<uint64_t, HydroEntry> hydro_cache_;
    mutable uint64_t hydro_tick_ = 0;

    Column column(int x, int z) const;
    Column column_base(int x, int z, bool with_slope) const;
    static uint32_t hash3(int a, int b, int c);
    static float spline(float v, const float *xs, const float *ys, int n);

    std::shared_ptr<HydroTile> hydro_tile(int tx, int tz) const;
    void build_hydro_tile(HydroTile &t) const;
    Hydro hydro_at(int x, int z, int base_height) const;
    bool hydro_ctx(int x, int z, HydroCtx &ctx) const;
    const HydroTile *tile_cached(int tx, int tz) const; // thread-local small cache over hydro_tile
    uint16_t vegetation_at(const Column &col, int x, int z, uint16_t top, const Hydro &hy, int &height) const;
    uint16_t strata_at(int x, int y, int z) const;   // badlands terracotta bands by height
    int spire_at(int biome, int x, int z) const;      // spire height above ground (decor.spire), 0 = none
    static int marsh_floor(int h, const Hydro &hy);

    bool tree_at_cell(int cx, int cz, Tree &out) const;
    int trees_near(int x0, int z0, int x1, int z1, Tree *out, int max_out) const;
    bool ruin_at_cell(int cx, int cz, Ruin &out) const;
    int ruins_near(int x0, int z0, int x1, int z1, Ruin *out, int max_out) const;
    static bool in_ruin(const Ruin &r, int x, int z, int margin);
    inline bool is_cave(int x, int y, int z, int height) const;
    // ---- ore veins (technosim_ore tables) ----
    struct Vein { int type, x, y, z; };
    bool vein_at_cell(int cx, int cz, Vein &out) const;
    float vein_half_thickness(const Vein &v, int x, int z) const; // 0 outside the lens
    inline uint16_t vein_ore(const Vein &v, float half_t, int x, int y, int z) const;
    uint16_t indicator_at(const Vein &v, int x, int z, int ground, uint16_t top) const;
};

} // namespace godot

VARIANT_ENUM_CAST(godot::WorldGen::Biome);
