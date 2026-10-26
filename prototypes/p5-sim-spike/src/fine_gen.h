#pragma once
// The world at 12.5 cm voxels (ADR 0033). WorldGen stays the macro layer in
// its 0.5 m units: climate, biomes, hydrology, ore veins, ruins and where
// trees and decorations stand. FineGen samples it on a lattice of macro
// columns, interpolates the surface between them, adds a small ground noise
// and draws every voxel at FS x the resolution: soil layers, rock faces from
// the real slope, caves from a trilinear cave field, voxel trees (young to old,
// tapered trunks with roots, limbs ending in leaf clumps), a forest floor of
// leaf litter, ferns, mushrooms, logs, stumps and bushes, tufts and flower patches.
// Every public coordinate is a fine voxel; output blocks are 16-bit material
// ids in godot_voxel VoxelBuffer order (y fastest, then x, then z).

#include "world_gen.h"
#include "rock_types.h"

#include <vector>

namespace godot {

class FineGen : public RefCounted {
    GDCLASS(FineGen, RefCounted)

public:
    static constexpr int FS = 4;   // fine voxels per macro unit (0.5 m / 12.5 cm)

    FineGen();

    // sea_level and world_height in fine voxels (multiples of FS)
    void configure(int p_seed, int p_sea_level, int p_world_height);
    int get_seed() const { return g_->get_seed(); }

    int height_at(int x, int z) const;          // solid voxels in the column (the top one is height - 1)
    int biome_at(int x, int z) const;
    Dictionary column_info(int x, int z) const;
    PackedByteArray generate_block(Vector3i origin, Vector3i size, int lod, int bytes_per_voxel = 2) const;
    // Deep rock from LOD 1 on comes back as one voxel (the block is all that material) when on; off: always
    // the whole block (probes and tests decode it voxel by voxel).
    void set_uniform_blocks(bool on) { uniform_blocks_ = on; }
    bool get_uniform_blocks() const { return uniform_blocks_; }

    PackedByteArray map_image(int center_x, int center_z, int radius, int step) const;
    // Debug sheets: n x n columns from (x0, z0) every step voxels, three ints each: fine surface height,
    // water surface (fine, -1 none) and a cover code (0 grass, 1 rock, 2 snow, 3 sand, 4 forest).
    PackedInt32Array surface_map(int x0, int z0, int n, int step) const;
    // Debug sheets: n x n columns (fine coordinates), two bytes each: the share of the column under the
    // ground that is cave (0..255, macro samples) and 1 where a cave opens onto the surface.
    PackedByteArray cave_map(int x0, int z0, int n, int step) const;
    PackedByteArray climate_map(int x0, int z0, int n, int step) const;
    PackedInt32Array water_floor_map(int x0, int z0, int n, int step) const;
    Vector3i find_nearest(const String &kind, int x, int z, int max_radius) const;
    Array waterfalls_near(int x, int z, int radius) const;
    Dictionary hydrology_stats(int x, int z) const;
    Dictionary vein_at(int x, int z) const;
    Array ruin_pots_near(int x, int z, int radius) const;   // WorldGen's, in fine voxels (the spot's lowest air voxel)
    Ref<WorldGen> get_macro() const { return g_; }

protected:
    static void _bind_methods();

private:
    using Mat = WorldGen::Mat;
    // One macro column of the lattice, with everything the fine columns near it need.
    struct MacroPt {
        WorldGen::Column col;
        WorldGen::Hydro hy;
        float hc = 0.0f;        // carved surface (macro units, float): solid below it
        int water_level = 0;    // macro
        bool lakebed = false, riverbed = false, marshbed = false;
        int ruin = -1;          // index into the block's ruins
        int spike = 0;
        int roof = 5;           // caves stop this many macro voxels under the ground (0: a cave mouth may open)
        float gw = 1.0f;        // weight of the fine ground relief: 0 at water, rivers and ruins, interpolated
    };
    struct Blob { float x, y, z, rx, ry, rz; };
    struct Seg { float ax, ay, az, bx, by, bz, r0, r1; };   // a limb or root tapering from r0 to r1
    static constexpr int MAX_BLOBS = 48, MAX_SEGS = 32;
    struct FTree {
        float x, z;             // trunk centre (fine)
        int ground, base, top;  // ground at the trunk, lowest root voxel, trunk top (fine y)
        float r, r_top;         // trunk radius above the flare and at the top
        int kind;
        uint16_t log, bark, leaves;   // bark on the outside of trunks and limbs, wood inside
        Blob blobs[MAX_BLOBS];  // leaf clumps
        int nb = 0;
        Seg segs[MAX_SEGS];     // limbs, branches, roots
        int ns = 0;
        float cone_r = 0.0f;    // spruce: cone radius at its base
        int cone_lo = 0, cone_hi = 0;
        float reach = 0.0f;     // horizontal bound of everything (fine)
        int y_hi = 0;
        uint32_t seed = 0;
    };
    // The parts of one tree that can touch one column: bits into its blobs and segs.
    struct ColTree {
        const FTree *t;
        uint64_t bm;
        uint32_t sm;
        bool trunk, cone;
        int ylo, yhi;
    };
    // Ground features: WorldGen's stamps plus FineGen's forest floor (logs at any angle, stumps, bushes, rocks).
    enum { STUMP = 10, STICK = 11, FLOWER_S = 12, MUSHROOM_S = 13, SHELL_S = 14, CLUTTER_H = 12 };   // STICK: a fallen stick (twigs pickup), laid on the ground per column;
    // FLOWER_S, MUSHROOM_S, SHELL_S: clutter pieces of whole voxels on a CLUTTER_CELL grid (ADR 0047), laid per column
    struct FStamp {
        int kind;
        float x, z;
        int ground;
        float size;             // radius, half length (logs) or height (stumps)
        float r;                // log and stump radius
        float dx, dz;           // log direction
        uint16_t wood, bark, leaves;
        bool mossy;
        int biome = 0;          // where it stands (wild food chances, biomes.json decor)
        uint32_t seed;
    };

    Ref<WorldGen> g_;
    bool uniform_blocks_ = false;
    FastNoiseLite n_ground_, n_ground2_, n_hump_, n_leaf_, n_litter_, n_meadow_, n_pool_;
    technosim_rock::RockTypes rocks_;   // rock kinds, strata, new clutter (ADR 0057, rock_types.cpp)

    MacroPt macro_point(int mx, int mz, const WorldGen::Ruin *ruins, int nruins) const;
    float ground_noise(int x, int z) const;
    float surface_f(int x, int z) const;        // standalone fine surface (4 lattice corners)
    float tree_age(const WorldGen::Tree &t, uint32_t seed) const;
    void build_tree(int kind, float fx, float fz, int trunk_h, int radius, float age, bool old_growth, uint32_t seed, FTree &out) const;
    bool col_tree(const FTree &t, float x, float z, float grow, ColTree &out) const;
    uint16_t tree_voxel(const ColTree &c, float x, int y, float z, float grow, float &leaf_n) const;
    void young_trees(int x_lo, int z_lo, int x_hi, int z_hi, const WorldGen::Ruin *ruins, int nruins, std::vector<FTree> &out) const;
    void build_stamp(const WorldGen::Stamp &s, FStamp &out) const;
    void floor_features(int x_lo, int z_lo, int x_hi, int z_hi, const WorldGen::Ruin *ruins, int nruins, bool clutter, std::vector<FStamp> &out) const;
    // Clutter piece voxel at (dx, dy, dz) from its stamp's centre column and ground (dy = 0: the first voxel above it).
    static uint16_t clutter_voxel(const FStamp &s, int dx, int dy, int dz);
    // A voxel of an ore vein's lens: host stone or the ore of its lump (ADR 0064).
    static uint16_t vein_voxel(const WorldGen::Vein &v, float ht, int wx, int wy, int wz, float bias, int seed);
    uint16_t stamp_voxel(const FStamp &s, float x, int y, float z, float grow) const;
    static float stick_dist(const FStamp &s, float x, float z, float *fork_out = nullptr);   // ground-plane distance to a stick and its branch
};

} // namespace godot
