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

#include <deque>
#include <memory>
#include <mutex>
#include <unordered_map>
#include <vector>

namespace godot {

// A cache shared by the generation threads (ADR 0105): the first thread to need a key builds its value, the
// others asking at the same time wait for that build instead of repeating it; the oldest keys go first.
template <typename V>
class SharedCache {
public:
    explicit SharedCache(int capacity) : per_shard_(capacity / SHARDS) {}
    // The value for key (built by build() if absent); built = this call built it.
    template <typename F>
    std::shared_ptr<const V> get(uint64_t key, F &&build, bool *built = nullptr) {
        Shard &sh = shards_[(key ^ (key >> 29)) % SHARDS];
        std::shared_ptr<Slot> slot;
        {
            std::lock_guard<std::mutex> l(sh.lock);
            auto it = sh.map.find(key);
            if (it != sh.map.end()) {
                slot = it->second;
            } else {
                while ((int)sh.map.size() >= per_shard_ && !sh.order.empty()) {
                    sh.map.erase(sh.order.front());
                    sh.order.pop_front();
                }
                slot = std::make_shared<Slot>();
                sh.map.emplace(key, slot);
                sh.order.push_back(key);
            }
        }
        bool b = false;
        std::call_once(slot->once, [&]() { slot->v = build(); b = true; });
        if (built) *built = b;
        return slot->v;
    }
    void clear() {
        for (int i = 0; i < SHARDS; i++) {
            std::lock_guard<std::mutex> l(shards_[i].lock);
            shards_[i].map.clear();
            shards_[i].order.clear();
        }
    }

private:
    static constexpr int SHARDS = 64;
    struct Slot {
        std::once_flag once;
        std::shared_ptr<const V> v;
    };
    struct Shard {
        std::mutex lock;
        std::unordered_map<uint64_t, std::shared_ptr<Slot>> map;
        std::deque<uint64_t> order;
    };
    int per_shard_;
    std::unique_ptr<Shard[]> shards_ = std::make_unique<Shard[]>(SHARDS);
};

class FineGen : public RefCounted {
    GDCLASS(FineGen, RefCounted)

public:
    static constexpr int FS = 4;   // fine voxels per macro unit (0.5 m / 12.5 cm)
    static constexpr int RUIN_RAMP = 4;   // macro cells round a ruin over which the ground ramps to its floor (ADR 0062)

    FineGen();

    // sea_level and world_height in fine voxels (multiples of FS)
    void configure(int p_seed, int p_sea_level, int p_world_height);
    int get_seed() const { return g_->get_seed(); }

    int height_at(int x, int z) const;          // solid voxels in the column (the top one is height - 1)
    int biome_at(int x, int z) const;
    Dictionary column_info(int x, int z) const;
    PackedByteArray generate_block(Vector3i origin, Vector3i size, int lod, int bytes_per_voxel = 2) const;
    // Blocks generated since the last call and the worker time they took, by result: air, uniform, full (usec).
    static Dictionary gen_stats();
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
    PackedInt32Array pickup_heights(int kind, int seed) const;   // a stone pickup's stamp: 16 column heights, x fastest (showcase)
    Array ruin_pots_near(int x, int z, int radius) const;   // WorldGen's, in fine voxels (the spot's lowest air voxel)
    Dictionary ruin_at(int x, int z) const;                 // WorldGen's, in fine voxels ({} outside every ruin)
    Ref<WorldGen> get_macro() const { return g_; }

protected:
    static void _bind_methods();

private:
    using Mat = WorldGen::Mat;
    PackedByteArray generate_block_impl(Vector3i origin, Vector3i size, int lod, int bytes_per_voxel) const;
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

    // What every block of one footprint (a vertical stack of blocks) shares: the macro lattice, the ruins
    // near it and the highest tree or stamp voxel over it (ADR 0105). A stack of ~100 blocks built it ~100
    // times; the cache builds it once.
    struct ColumnCtx {
        int x = 0, z = 0, sx = 0, sz = 0, lod = 0;   // the footprint it was built for
        std::vector<MacroPt> lat;
        WorldGen::Ruin ruins[4];
        technosim_ruin::Prepared rprep[4];
        int nruins = 0;
        int hmax_m = 0, wl_max = 0, fall_max = -1;
        float hmin_m = 1e9f;
        int feat_top = 0;   // highest tree or stamp voxel over the footprint at any height (0: none)
        // the trees (from the tree caches) and stamps reaching the footprint at any height, in drawing order
        std::vector<std::shared_ptr<const FTree>> trees;
        size_t n_mature = 0;
        std::vector<FStamp> stamps;
    };
    mutable SharedCache<ColumnCtx> ctx_cache_{16384};   // 16k footprints (5-15 KB with their stamps): a 100 m box holds ~10k
    // A young tree's cell (24 voxels): the tree standing in it, or none, before the ruins near a block decide.
    struct YoungCell {
        bool any = false;
        int mx = 0, mz = 0;
        FTree t;
    };
    mutable SharedCache<FTree> tree_cache_{16384};       // built mature trees (~2.3 KB) by trunk position
    mutable SharedCache<YoungCell> young_cache_{16384};
    std::shared_ptr<const ColumnCtx> column_ctx(Vector3i origin, Vector3i size, int lod) const;
    std::shared_ptr<const ColumnCtx> build_column_ctx(Vector3i origin, Vector3i size, int lod) const;
    // Trees and stamps reaching the footprint at any height, in the order the block pass draws them; n_mature
    // = how many of the trees come from WorldGen's tree list (young trees follow).
    void gather_features(int x_lo, int z_lo, int x_hi, int z_hi, const WorldGen::Ruin *ruins, int nruins, int lod,
                         std::vector<std::shared_ptr<const FTree>> &trees, size_t &n_mature, std::vector<FStamp> &stamps) const;

    Ref<WorldGen> g_;
    bool uniform_blocks_ = false;
    FastNoiseLite n_ground_, n_ground2_, n_hump_, n_leaf_, n_litter_, n_meadow_, n_pool_, n_terrace_;
    technosim_rock::RockTypes rocks_;   // rock kinds, strata, new clutter (ADR 0057, rock_types.cpp)

    MacroPt macro_point(int mx, int mz, const WorldGen::Ruin *ruins, int nruins) const;
    float surface_f(int x, int z) const;        // standalone fine surface (4 lattice corners)
    float ground_f(float base, int x, int z, float w) const;   // swells, ledges and hummocks over a macro height (weight w)
    float tree_age(const WorldGen::Tree &t, uint32_t seed) const;
    void build_tree(int kind, float fx, float fz, int trunk_h, int radius, float age, bool old_growth, uint32_t seed, FTree &out) const;
    bool col_tree(const FTree &t, float x, float z, float grow, ColTree &out) const;
    uint16_t tree_voxel(const ColTree &c, float x, int y, float z, float grow, float &leaf_n) const;
    void young_trees(int x_lo, int z_lo, int x_hi, int z_hi, const WorldGen::Ruin *ruins, int nruins, std::vector<std::shared_ptr<const FTree>> &out) const;
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
