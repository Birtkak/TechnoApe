#pragma once
// The macro layer of the world (ADR 0033, 0035), in 0.5 m units: FineGen draws the 12.5 cm voxels
// from it and nothing else generates blocks.
//  - landforms: continents (~8 km), regions of plains, hill country and ridged mountain ranges,
//    erosion -> height; temperature + humidity (jittered) -> biomes with dithered borders
//  - altitude bands on mountains: alpine rock, scree, jittered tree line, permanent snow line
//  - hydrology on a coarse grid (32-voxel cells, 2 km tiles with 1 km margin):
//    priority-flood depression filling, D8 flow routing, flow accumulation ->
//    springs, streams that merge into collector rivers, basin lakes, outflow
//    to the ocean, waterfalls, marshes
//  - the cave noises FineGen interpolates, GregTech-style ore veins (ore_veins.gen.h) and
//    the ore-bearing stones above shallow veins
//  - where trees, ground stamps and ruins stand, and which plant a column grows
// Deterministic from a seed. Heights in macro voxels of 0.5 m (FineGen::FS fine voxels each).

#include <godot_cpp/classes/ref_counted.hpp>
#include <godot_cpp/variant/array.hpp>
#include <godot_cpp/variant/dictionary.hpp>
#include <godot_cpp/variant/packed_byte_array.hpp>
#include <godot_cpp/variant/vector3i.hpp>

#include "FastNoiseLite.h"
#include "biome_table.gen.h" // generated from game/data/biomes.json (36 biomes, classify(), classify_3d())
#include "ore_veins.gen.h"   // generated from game/data/ore_veins.json (vein types, grid, indicators)
#include "ruins.h"           // ruin kinds and shapes (ruin_table.gen.h from game/data/ruins.json, ADR 0062)

#include <cstdint>
#include <memory>
#include <algorithm>
#include <atomic>
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
        LAVA_STATIC = 121, // still lava: cave pools and the deep lava sea, finite moving lava once opened (ADR 0069 C)
        // 88..89 work pieces (ADR 0023, never generated); content pass 2
        BIRCH_LOG = 90, BIRCH_LEAVES = 91, BLUEBERRY_BUSH = 92, PUMPKIN = 93, CORNFLOWER = 94, OXEYE_DAISY = 95, ALLIUM = 96,
        OAK_BARK = 97, SPRUCE_BARK = 98, ACACIA_BARK = 99, JUNGLE_BARK = 100, BIRCH_BARK = 101,
        CARCASS = 102,  // wildlife (ADR 0027)
        // 103..107 Flint Age (never generated)
        LEAF_LITTER = 108,  // the shaded forest floor (2026-10-23)
        // 109..110 patterned building materials (never generated); clutter as voxel pieces (ADR 0047)
        PLANT_STEM = 111, KELP = 112, SEAGRASS = 113, SEASHELL = 114, MUSHROOM_STEM = 115,
        // 116..119 reserved (part A of the 2026-10-25 split); world generation 120..129
        CRYSTAL = 120,
        SPRING_STONE = 122, MAGMA_HEART = 123,   // the source of an aquifer / a magma conduit (ADR 0110)
        FIREDAMP = 133,   // ADR 0148: explosive mine gas, pockets in coal lumps
        GIANT_STEM = 124, GIANT_PETAL = 125, GIANT_HEART = 126,   // giant flowers in the flower meadow
        // cave dressing (FineGen::cave_dressing): game/scripts/materials.gd CAVE_IDS
        DRIPSTONE = 228, CAVE_VINE = 229, GLOW_BERRY_VINE = 230, FUNGUS_STEM = 231, FUNGUS_CAP = 232, MAGMA_ROCK = 233, RUSTED_RAIL = 234,
        DUNE_GRASS = 235, HEATHER = 236, TUNDRA_SEDGE = 237,
        PLANKS = 36, ROPE = 238, COBBLESTONE = 109,
        SAGEBRUSH = 239, DESERT_MARIGOLD = 240, MOSS_CAMPION = 241, FIRE_LILY = 242, LINGONBERRY = 243, BROMELIAD = 244, CATTAIL = 245, ALOE = 246, BLUEBELL = 247, FROST_LICHEN = 248, SEA_ASTER = 249,
        // the dry savanna floor (bug report 2026-10-25 20-37-22): straw grass, its tussocks, bare red earth
        SAVANNA_GRASS = 127, SAVANNA_TALL_GRASS = 128, RED_EARTH = 129
    };
    // The first 13 ids are stable (bound to GDScript); the rest come from
    // biomes.json through biome_table.gen.h (technosim_biome::Id, NAMES, COLORS,
    // SURFACE_* and TREE_DENSITY tables, classify() / classify_3d()).
    enum Biome : int {
        BIOME_DEEP_OCEAN = 0, BIOME_OCEAN = 1, BIOME_BEACH = 2, BIOME_PLAINS = 3, BIOME_FOREST = 4,
        BIOME_DESERT = 5, BIOME_TUNDRA = 6, BIOME_HIGHLANDS = 7, BIOME_MOUNTAINS = 8, BIOME_SNOWY_PEAKS = 9,
        BIOME_RIVER = 10, BIOME_LAKE = 11, BIOME_ALPINE = 12,
        BIOME_COUNT = 41
    };
    static_assert(BIOME_COUNT == technosim_biome::COUNT, "biome_table.gen.h changed: regenerate and update BIOME_COUNT");

    WorldGen();
    static uint32_t hash3(int a, int b, int c);

    void configure(int p_seed, int p_sea_level, int p_world_height);
    int get_seed() const { return seed_; }

    // Column queries (include hydrology: rivers and lakes)
    int height_at(int x, int z) const;
    int biome_at(int x, int z) const;
    Dictionary column_info(int x, int z) const;


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
    // The clay pots of the ruins within radius of (x, z) (ADR 0057, 0062): [{x, y, z, ruin, seed, kind}] in
    // macro voxels, y = the macro cell the pot stands in; one to four per ruin, the same spots the generator
    // puts its pot voxels in (technosim_ruin::pot_spots).
    Array ruin_pots_near(int x, int z, int radius) const;
    // The ruin whose region (footprint + 2 cells) holds macro (x, z): {kind, x, y (floor), z, half_x, half_z, rot}, {} when none.
    Dictionary ruin_at(int x, int z) const;
    // Debug: switch a generator variant for comparison sheets (clears the hydrology cache; not thread-safe
    // against running generation). Slot VAR_MOUNTAINS: 0 the ranges of ADR 0035, 1 massif (default, ADR
    // 0051), 2 horns; VAR_CAVES: 0 the caves of ADR 0035, 1 caverns, ravines and mouths (default, ADR 0052);
    // VAR_RELIEF: mountain relief in metres (0: 400 m).
    enum { VAR_MOUNTAINS = 0, VAR_CAVES = 1, VAR_RELIEF = 2, VAR_COUNT = 4 };
    void set_variant(int slot, int value);

protected:
    static void _bind_methods();

private:
    // ---- terrain columns ----
    friend class FineGen;   // the 12.5 cm voxel layer samples these fields (ADR 0033)
    struct Column {
        int height;      // base terrain height (no rivers/lakes)
        float hf;        // the same before rounding (FineGen interpolates it)
        int biome;
        float c, e, pv, t, h; // t is the final climate (noise + latitude + lapse), see biomes.md
        float t_noise;        // temperature noise + jitter before latitude and lapse (debug / probes)
        float slope;     // |dh/dx|, |dh/dz| max (voxels per voxel), only computed on mountains
        bool alpine;     // above the tree line
        bool snow;       // permanent snow
        uint16_t rock = 0;   // a landmark's own rock for its faces and top (monoliths), 0 = none
        float shore = 1e9f;  // estimated signed distance to the coastline (voxels): > 0 inland, < 0 out at sea
        float dn = 0.0f;     // the detail noise (~37 m patches): sea floor sand / mud / gravel
        float lava = -1.0f;  // a volcano's crater: still lava up to this height (macro, absolute), -1 none
    };
    // What the sea floor is made of here (fine and far meshes agree): sand in the shallows and on
    // the foreshore, sand with mud and gravel patches on the shelf, mud in the deep, rock on steep slopes.
    uint16_t seabed(const Column &col, float slope) const;
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
        float lake_near = -1.0f;   // the level of a lake within reach (debug: column_info), -1 none
        float delta = 0.0f;   // 0..1 how far this river column is into a delta (ADR 0051)
        int berm = -1;    // ground raised to at least this (a shallow lake's rim on flat land), -1 none
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
    struct Tree { int x, z, base, trunk_h, radius, kind, biome; bool stunted; };
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
    static constexpr int RUIN_CELL = 160;
    static constexpr int RUIN_MAX_HALF = 14;
    using Ruin = technosim_ruin::Ruin;

    int seed_ = 1337;
    int variant_[VAR_COUNT] = {1, 1, 0, 0};
    int sea_ = 256;
    int world_height_ = 1280;
    FastNoiseLite n_cont_, n_eros_, n_pv_, n_detail_, n_temp_, n_hum_, n_cheese_, n_spag1_, n_spag2_, n_ore_, n_jitter_, n_cavebiome_;
    FastNoiseLite n_region_, n_hillmask_, n_hill_;   // landform regions (plains, hills, mountain ranges) and hill shapes
    FastNoiseLite n_cavern_, n_ravine_, n_ravmask_;   // caverns and ravines (VAR_CAVES 1)
    FastNoiseLite n_pillar_;                          // rock columns inside the halls (ADR 0077)
    FastNoiseLite n_hall_, n_giant_;                  // hall footprints per storey, the mask of the giant chambers (ADR 0134)
    FastNoiseLite n_gallery_, n_galoff_;              // gallery routes per storey and the storeys' slow tilt (ADR 0080)
    FastNoiseLite n_cliff_;   // rocky coasts: where the land ends in a sea cliff (ADR 0051)
    FastNoiseLite n_meander_; // lowland river bends
    FastNoiseLite n_band_;    // mountain cliff bands: where the slopes break into tiers and how tall they are
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
    std::atomic<uint64_t> epoch_{0};   // new on every configure and set_variant (tile_cached slots)

    Column column(int x, int z) const;
    Column column_base(int x, int z, bool with_slope) const;
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
    // The cave field (macro units): positive inside a cave. FineGen interpolates it per voxel.
    // small_only: tunnels, rooms and mouths without the caverns and ravines (FineGen adds those from coarse
    // samples: cavern_value, ravine_value).
    float cave_value(int x, int y, int z, int height, bool small_only = false) const;
    // Gallery storeys (ADR 0080): spacing and half height (macro). Halls stand on them (ADR 0134).
    static constexpr float GAL_SP = 28.0f, GAL_HH = 3.2f;
    // Halls (ADR 0134): footprint threshold, roof height and talus height at the wall (macro); the giant chambers where
    // n_giant_ passes GIANT_MASK.
    // DOME: columns in from the wall over which the roof rises; SPAN: over which the talus falls to the flat floor.
    static constexpr float HALL_THR = 0.45f, HALL_H = 24.0f, HALL_TALUS = 6.0f, HALL_DOME = 10.0f, HALL_SPAN = 10.0f;
    static constexpr float GIANT_MASK = 0.45f, GIANT_THR = 0.3f, GIANT_H = 44.0f, GIANT_TALUS = 36.0f, GIANT_DOME = 20.0f, GIANT_SPAN = 50.0f;
    // memo: the storey footprints hall_value read for this column (ADR 0135: a column's rows mostly share a storey)
    struct HallCol { float x = 0, z = 0, galoff = 0, giant = 0, pillar = -1; mutable float memo_k[4] = {1e30f, 1e30f, 1e30f, 1e30f}, memo_v[4] = {}; mutable int memo_i = 0; };
    void hall_column(int x, int z, float galoff, HallCol &out) const;
    // Positive inside a hall. rim (when given): 0 away from halls, else 1 at a hall's wall falling to 0 on its flat
    // floor; talus: whether fy lies in the rubble slope at the foot of the wall. Giant chambers keep pillars (ADR 0077).
    // cut: how much a tunnel or room at this point must close so it meets a hall at its floor (0..1).
    float hall_value(const HallCol &c, float fy, float depth, float *rim = nullptr, bool *talus = nullptr, bool giant_only = false, float *cut = nullptr) const;
    // A ravine at one column (ADR 0133): the mask (faded out near streams and lakes), the signed line noise at
    // the warped spot (|line| near 0 is the centre, so the gorge winds) and the two wall wobbles. Every caller
    // reads a ravine through this, so the carve, the rims, the roof and the bridges agree.
    struct RavineCol { float mask = 0.0f, line = 1.0f, wa = 0.0f, wb = 0.0f; };
    RavineCol ravine_col(float x, float z) const;
    float ravine_line(float x, float z) const;   // the signed, warped line noise alone (bridges walk onto it)
    float stream_near(float x, float z) const;   // 0..1: a stream or a lake within ~16 m, fading out by ~40 m
    static float ravine_value(const RavineCol &r, float depth);
    // Whether a ravine or a sinkhole has cut the ground within r macro columns of (x, z): nothing stands there
    // (trees, stamps, sticks), or it would hang over the hole (bug report 2026-10-25 20-18-57).
    bool ravine_near(int x, int z, int r) const;
    // Whether a cavern or a ravine may reach into the box (macro units), for the deep-rock shortcut.
    bool big_cave_possible(int x0, int y0, int z0, int x1, int y1, int z1, int ground_min) const;
    // How far below the surface caves stop at this column (macro voxels): 5 normally, 0 where a cave mouth
    // or a ravine may open onto the surface.
    int cave_roof(int x, int z) const;
    // How far under the ground the open cave at this column reaches (macro voxels): the ravine's floor, the
    // sinkhole's bottom, the first stretch of a ramp; 0 where cave_roof is not 0. The streaming shell keeps a
    // column's data down to this instead of a flat 80 m under every opening (ADR 0124).
    int open_depth(int x, int z) const;
    // ---- landforms (ADR 0051) ----
    // Sea stacks off cliff coasts: one chance per 12 m cell; some are arches (a tunnel at the waterline).
    static constexpr int STACK_CELL = 24;
    struct SeaStack { float x, z, r, h; bool arch; float adx, adz; };
    bool stack_at_cell(int cx, int cz, SeaStack &out) const;
    float cliff_height(float cliff_mask) const { return 14.0f + 40.0f * std::clamp((cliff_mask - 0.15f) / 0.45f, 0.0f, 1.0f); }
    // Monoliths (Uluru-like) on open lowland: one chance per 1.5 km cell.
    static constexpr int MONO_CELL = 3000;
    float monolith_height(int x, int z) const;   // extra height at (x, z), 0 outside
    // Volcanoes: a basalt cone with a lava-filled crater, one chance per 2 km cell on open land.
    static constexpr int VOLCANO_CELL = 4000;
    bool volcano_at_cell(int cx, int cz, float &mx, float &mz, float &R, float &H) const;   // the cell's volcano (centre, radius, rim height), if any
    float volcano_height(int x, int z, float *lava_top) const;   // cone height over the sea at (x, z), 0 outside; lava_top: crater lava surface over the sea
    static constexpr int MOUTH_CELL = 128;   // macro voxels (64 m)
    // Cave entrances and connectors (ADR 0077), one chance per cell. kind 0: a winding ramp that leaves the ground
    // (len, slope along dx/dz). kind 1: a sinkhole, a funnel in the ground over a shaft len deep. kind 2: a chimney,
    // a shaft from `top` macro voxels under the ground down len more, joining the tunnels above to the halls and the
    // deep below; it never opens onto the surface.
    enum { MOUTH_RAMP = 0, MOUTH_SINKHOLE = 1, MOUTH_CHIMNEY = 2 };
    struct CaveMouth { float x, z, dx, dz, len, slope, r, top; int kind; };
    int mouths_near(int x, int z, CaveMouth *out) const;   // the mouths that may reach (x, z), at most 9
    // What cave_value reads at (x, z) whatever the height: a column of cave samples fills it once (ADR 0105).
    struct CaveColumn { int x = 0, z = 0; float galoff = 0.0f; int nmouths = 0; CaveMouth mouths[9]; };
    void cave_column(int x, int z, CaveColumn &out) const;
    // hc: the column's hall_column when the caller has it; hall: hall_value at (y, depth) out (epic caves only)
    float cave_value(const CaveColumn &col, int y, int height, bool small_only = false, const HallCol *hc = nullptr, float *hall = nullptr) const;
    // The ore of the small veins (stringers) in a 96 m ore cell: its lens's type, or a weighted pick where it has none.
    int stringer_type(int cx, int cz) const;
    // Crystal geodes (ADR 0052): at most one per 96 m cell, deep in the rock. Macro units.
    static constexpr int GEODE_CELL = 192;
    // Conduits, the veins of the world (ADR 0110): aquifers and magma conduits. One chance per CONDUIT_CELL
    // cell: a polyline of CONDUIT_SEGS segments from a source chamber deep in the rock, wandering and rising
    // toward a tail near the ground, as a tube of still liquid (finite once breached, ADR 0068 / 0069 C). The
    // chamber holds the liquid round a core of spring stone / magma heart. Macro units.
    static constexpr int CONDUIT_CELL = 512, CONDUIT_SEGS = 6;   // 256 m cells
    static constexpr int CONDUIT_RINGS = 2;   // a conduit wanders at most 2 cells from its own
    enum { CONDUIT_AQUIFER = 0, CONDUIT_MAGMA = 1 };
    struct Conduit { int kind; float r, src_r; float x[CONDUIT_SEGS + 1], y[CONDUIT_SEGS + 1], z[CONDUIT_SEGS + 1]; };
    bool conduit_at_cell(int cx, int cz, Conduit &out) const;
    int conduits_near(int x, int z, Conduit *out, int max) const;   // the conduits that may reach (x, z)
    // Distance (macro) from a point to the nearest conduit's tube or chamber surface (negative inside), with
    // that conduit's kind and the distance to its chamber; 1e9 when none is in reach.
    float conduit_dist(float x, float y, float z, int &kind, float &src_dist) const;
    static inline float seg_dist3(float px, float py, float pz, float ax, float ay, float az, float bx, float by, float bz) {
        const float dx = bx - ax, dy = by - ay, dz = bz - az, l2 = dx * dx + dy * dy + dz * dz;
        const float t = l2 > 0.0f ? std::clamp(((px - ax) * dx + (py - ay) * dy + (pz - az) * dz) / l2, 0.0f, 1.0f) : 0.0f;
        const float ex = px - (ax + dx * t), ey = py - (ay + dy * t), ez = pz - (az + dz * t);
        return std::sqrt(ex * ex + ey * ey + ez * ez);
    }
    struct Geode { float x, y, z, r; uint32_t seed; };
    bool geode_at_cell(int cx, int cz, Geode &out) const;
    // ---- ore veins (technosim_ore tables) ----
    struct Vein { int type, x, y, z; };
    bool vein_at_cell(int cx, int cz, Vein &out) const;
    float vein_half_thickness(const Vein &v, int x, int z) const; // 0 outside the lens
    uint16_t indicator_at(const Vein &v, int x, int z, int ground, uint16_t top) const;
};

} // namespace godot

VARIANT_ENUM_CAST(godot::WorldGen::Biome);
