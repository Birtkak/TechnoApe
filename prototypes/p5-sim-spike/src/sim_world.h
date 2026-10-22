#pragma once
// P5 v4: data-driven material simulation in the Falling Everything (Noita) model.
//
// Every voxel has a material id (u8 in this spike; u16 in the real store), a
// flags byte (burning, ...) and an aux byte (remaining lifetime or fire hp).
// Every material has a cell_type (solid / liquid / gas / fire) and Noita-named
// attributes (liquid_sand, density, lifetime, burnable, fire_hp, ...). A
// reaction table (input a + input b -> output a + output b, probability)
// drives chemistry. Chunks of 16^3; only active chunks update; 8 parity passes
// on a persistent thread pool; per-chunk flag batching.

#include <godot_cpp/classes/ref_counted.hpp>
#include <godot_cpp/variant/array.hpp>
#include <godot_cpp/variant/dictionary.hpp>
#include <godot_cpp/variant/packed_byte_array.hpp>
#include <godot_cpp/variant/packed_int32_array.hpp>
#include <godot_cpp/variant/vector3i.hpp>

#include <atomic>
#include <condition_variable>
#include <cstdint>
#include <functional>
#include <memory>
#include <mutex>
#include <thread>
#include <vector>

namespace godot {

class SimWorld : public RefCounted {
    GDCLASS(SimWorld, RefCounted)

public:
    enum CellType : uint8_t { CELL_SOLID = 0, CELL_LIQUID = 1, CELL_GAS = 2, CELL_FIRE = 3 };
    // FLAG_LOOSE: a static-ground cell (liquid_static) that a nearby change has shaken loose; it now
    // moves like its loose kind (Noita's static -> dynamic) until something places it again.
    // FLAG_SHAKEN: static ground that saw a fresh opening; it keeps rolling its loosen chance while the
    // opening stays open (crumbles over time instead of only on the tick of the change).
    enum Flags : uint8_t { FLAG_BURNING = 1, FLAG_LOOSE = 2, FLAG_SHAKEN = 4 };
    static constexpr int CS = 16;
    static constexpr int MAX_MATERIALS = 256;
    static constexpr int MAX_DISP = 7; // must stay < CS/2 for the parity scheme

    struct MaterialDef {
        uint8_t cell_type = CELL_GAS; // air default
        bool liquid_sand = false;      // powder
        bool liquid_static = false;    // ground: never moves
        bool burnable = false;
        uint8_t density = 0;           // 0..255 (Noita 0..15 scaled by 16)
        uint8_t disp = 0;              // lateral dispersion cells for liquids (from liquid_flow_speed)
        uint8_t gravity_prob = 255;    // chance per tick to fall (from liquid_gravity)
        uint8_t gas_up = 100, gas_horizontal = 100, gas_down = 0; // 0..255 chances
        uint8_t lifetime = 0;          // ticks (0 = infinite), scaled: aux = lifetime
        uint8_t fire_hp = 0;           // ticks a burning cell lasts
        uint8_t temperature_of_fire = 10;
        uint8_t autoignition_temperature = 100;
        uint8_t on_fire_convert_to = 0; // material when burnt out (0 = air)
        uint8_t on_fire_flame_material = 0;
        uint8_t on_fire_smoke_material = 0;
        uint8_t generates_smoke = 0;   // 0..255 chance per burning tick
        uint8_t extinguish_mask = 0;   // unused for now
        uint16_t ignite_chance = 8192; // 0..65535 per attempt by a burning neighbour (see set_materials)
        uint16_t loosen = 0;   // static ground: chance per tick (0..65535) to turn loose when the cell below just opened
        uint16_t erode = 0;    // static powder: chance per tick to slide off an edge that just opened beside and below it
    };

    struct Reaction {
        uint8_t in1, in2, out1, out2;
        uint16_t probability; // 0..65535 per tick per neighbour test
    };

    SimWorld();
    ~SimWorld();

    // ---- setup ----
    void configure(Vector3i p_chunks, int p_threads, int p_seed);
    void set_materials(const Array &p_defs);      // Array of Dictionary (Noita attribute names)
    void set_reactions(const Array &p_reactions); // Array of Dictionary {input_cell1, input_cell2, output_cell1, output_cell2, probability}
    int get_material_count() const;
    void fill_box(Vector3i p_min, Vector3i p_max, int p_material);
    void scatter(Vector3i p_min, Vector3i p_max, int p_material, float p_density, int p_seed);
    void set_voxel(Vector3i p, int p_material);
    int get_voxel(Vector3i p) const;
    int get_flags(Vector3i p) const; // FLAG_BURNING etc. (0 outside)
    void ignite(Vector3i p);
    int fill_sphere(Vector3 p_center, float p_radius, int p_material, const PackedInt32Array &p_keep);
    int ignite_box(Vector3i p_min, Vector3i p_max);
    void mark_all_loaded(bool p_loaded);
    // v5 (P10): fall acceleration in cells/tick^2 for powders and liquids (0 = one cell per tick, P5 behaviour).
    // At 6.25 cm and 30 Hz, 9.8 m/s^2 is 0.174 cells/tick^2.
    void set_gravity(float p_cells_per_tick2);
    // v5: scan only each active chunk's dirty box (default on); off = whole chunks (P5 behaviour) for comparison.
    void set_dirty_boxes(bool p_on);
    // v6 (P10): level of detail and budget. Chunks further than lod_radius chunks from the focus tick
    // only every lod_divisor-th tick; when cell_budget > 0 a tick scans at most that many cells
    // (nearest chunks first, waiting chunks gain priority each tick). Deferred chunks keep their
    // activity and dirty box for the next tick, so nothing is lost, only delayed.
    void set_focus(Vector3i p_cell);
    void set_lod(int p_radius_chunks, int p_divisor);
    void set_cell_budget(int p_cells);
    // P10: liquid fullness (Grimorium): a liquid cell holds 1..8 eighths (Cell.pad, 0 = full), fills
    // the partly full cell below it, splits half into an empty neighbour (the front moves one cell per
    // tick) and evens out with neighbours that differ by two or more eighths; mass is conserved and a
    // one-eighth film stays put, so floods settle. Off = P5 liquids (whole cells, lateral slides).
    void set_liquid_fullness(bool p_on);
    int count_liquid_eighths(int p_material) const;   // total eighths of a liquid (mass check)
    // v6: one tick on a background thread. Every other call that reads or writes cells waits for it.
    bool start_tick();          // false if a tick is still running
    bool is_tick_done() const;
    Dictionary finish_tick();   // waits, returns that tick's stats

    // ---- simulation ----
    Dictionary tick(int p_count);

    // ---- render feed / world streaming ----
    PackedByteArray get_chunk_bytes(Vector3i p_chunk, int p_depth_bytes = 1) const;
    void set_chunk_bytes(Vector3i p_chunk, const PackedByteArray &p_bytes, int p_depth_bytes = 1); // load from world; wakes, not dirty
    Array shift(Vector3i p_delta_chunks); // slide the window; returns vacated local chunks that must be loaded
    Array get_dirty_chunks() const;
    // v6: the changed cells of every dirty chunk as a box, for uploads finer than whole chunks:
    // 9 ints per chunk: chunk x,y,z, then min x,y,z and max x,y,z in local cells (inclusive).
    PackedInt32Array get_dirty_boxes() const;
    // Bridge to the brick world (ADR 0012 step 3): writes the cells inside the dirty boxes into a
    // BrickStore. scale 8: one sim cell is one brick (the 0.5 m game); origin = store brick of sim
    // cell (0,0,0); a brick is rewritten only when its material (an expanded brick: its majority)
    // differs, so chisel detail survives. scale 1: one sim cell is one store cell (micro sim);
    // origin = store cell of sim cell (0,0,0). Returns bricks/cells written. Call before clear_dirty().
    int write_dirty_to_store(Object *p_store, Vector3i p_origin, int p_scale);
    void clear_dirty();

    // ---- debug / introspection ----
    int count_material(int p_material) const;
    int count_in_box(Vector3i p_min, Vector3i p_max, int p_material) const;
    int count_unsupported(int p_material) const;
    int count_burning() const;
    PackedInt32Array layer_histogram(int p_material) const;
    PackedInt32Array material_histogram() const;
    Dictionary chunk_info(Vector3i p_chunk) const;
    Dictionary get_stats() const;
    int get_active_chunk_count() const;
    Vector3i get_size_voxels() const;

protected:
    static void _bind_methods();

private:
    struct Pool {
        std::vector<std::thread> threads;
        std::mutex m;
        std::condition_variable cv_work, cv_done;
        const std::vector<int> *items = nullptr;
        std::function<void(int)> job;
        std::atomic<int> next{0};
        int remaining = 0;
        uint64_t generation = 0;
        bool stop = false;
        void start(int n);
        void run(const std::vector<int> &p_items, const std::function<void(int)> &p_job);
        void shutdown();
    };

    struct Marks { uint32_t wake = 0; uint32_t dirty = 0; uint32_t box[27] = {}; uint32_t dbox[27] = {}; };
    struct Ctx { int cx, cy, cz; Marks mk; uint32_t rng; uint64_t moved = 0; uint64_t reacted = 0; };
    // Dirty box of a chunk packed in 25 bits: valid | min xyz | max xyz (4 bits each, local 0..15).
    static constexpr uint32_t BOX_VALID = 1u << 24;
    static constexpr uint32_t BOX_FULL = BOX_VALID | (15u << 12) | (15u << 16) | (15u << 20);
    static inline uint32_t box_point(int x, int y, int z) {
        return BOX_VALID | (uint32_t)x | ((uint32_t)y << 4) | ((uint32_t)z << 8) | ((uint32_t)x << 12) | ((uint32_t)y << 16) | ((uint32_t)z << 20);
    }
    static inline uint32_t box_union(uint32_t a, uint32_t b) {
        if (!(a & BOX_VALID)) return b;
        if (!(b & BOX_VALID)) return a;
        uint32_t r = BOX_VALID;
        for (int k = 0; k < 3; k++) {
            uint32_t lo = std::min((a >> (4 * k)) & 15u, (b >> (4 * k)) & 15u);
            uint32_t hi = std::max((a >> (12 + 4 * k)) & 15u, (b >> (12 + 4 * k)) & 15u);
            r |= (lo << (4 * k)) | (hi << (12 + 4 * k));
        }
        return r;
    }
    void merge_box(int ci, uint32_t b);
    void merge_dbox(int ci, uint32_t b);
    struct TickOut { double ms = 0; uint64_t active = 0, moved = 0, react = 0, scanned = 0, deferred = 0; };
    void tick_once(TickOut &o);
    void defer_chunk(int ci);
    static inline int box_volume(uint32_t b) {
        if (!(b & BOX_VALID)) b = BOX_FULL;
        int v = 1;
        for (int k = 0; k < 3; k++) {
            int lo = std::max(0, (int)((b >> (4 * k)) & 15u) - 1), hi = std::min(CS - 1, (int)((b >> (12 + 4 * k)) & 15u) + 1);
            v *= hi - lo + 1;
        }
        return v;
    }
    void wait_idle() const;
    void stop_driver();
    void wake_full(int ci);

    Vector3i chunks_;
    int W_ = 0, H_ = 0, D_ = 0;
    int CX_ = 0, CY_ = 0, CZ_ = 0;
    int threads_ = 1;
    uint32_t seed_ = 1;
    uint32_t tick_ = 0;
    uint16_t stamp_ = 1;

    std::vector<MaterialDef> mats_;
    std::vector<Reaction> reactions_;
    std::vector<int16_t> react_lut_; // [a*256+b] -> reaction index or -1
    std::vector<uint8_t> reactive_;  // per material: 1 if it appears as an input of any reaction
    std::vector<uint8_t> inert_;     // per material: 1 if nothing ever needs to run on it (static solid, no lifetime, not reactive, not burnable)
    bool any_reactions_ = false;
    bool any_fire_ = false;

    struct Cell { uint8_t mat, flags, aux, pad; }; // 4 bytes, one cache line touch per voxel
    std::vector<Cell> cells_;
    std::vector<uint8_t> displace_lut_; // [mover*256+target] -> 1 if mover may swap into target
    std::vector<uint16_t> moved_;
    std::unique_ptr<std::atomic<uint8_t>[]> active_now_;
    std::unique_ptr<std::atomic<uint8_t>[]> active_next_;
    std::unique_ptr<std::atomic<uint8_t>[]> dirty_;
    std::vector<uint8_t> loaded_; // per chunk: 1 once world data was loaded; unloaded chunks act as solid walls
    std::unique_ptr<std::atomic<uint32_t>[]> box_now_;  // v5 dirty boxes (packed, see box_point)
    std::unique_ptr<std::atomic<uint32_t>[]> box_next_;
    std::unique_ptr<std::atomic<uint32_t>[]> dbox_;     // v6 changed-cells box per dirty chunk
    uint16_t gravity16_ = 0;   // v5 fall acceleration in 1/16 cell per tick^2
    Vector3i focus_chunk_;     // v6 level of detail
    int lod_radius_ = -1;      // chunks; -1 = off
    int lod_divisor_ = 1;
    int cell_budget_ = 0;      // 0 = unlimited
    std::vector<uint8_t> defer_age_;
    std::vector<std::pair<int64_t, int>> sched_;
    // v6 background tick
    std::thread driver_;
    mutable std::mutex dm_;
    mutable std::condition_variable dcv_;
    bool drv_stop_ = false, drv_req_ = false;
    std::atomic<bool> busy_{false};
    TickOut last_;
    bool dirty_boxes_ = true;
    bool fullness_ = false;
    std::vector<int> pass_lists_[8];
    std::atomic<uint64_t> moved_count_{0};
    std::atomic<uint64_t> react_count_{0};
    Pool pool_;

    inline int idx(int x, int y, int z) const { return x + W_ * (y + H_ * z); }
    inline int cidx(int cx, int cy, int cz) const { return cx + CX_ * (cy + CY_ * cz); }
    inline bool in_world(int x, int y, int z) const {
        return x >= 0 && y >= 0 && z >= 0 && x < W_ && y < H_ && z < D_;
    }
    // In world AND inside a chunk whose data has been loaded (streaming bubbles load
    // chunks over several frames; an unloaded neighbour must not look like air).
    inline bool is_open(int x, int y, int z) const {
        return in_world(x, y, z) && loaded_[cidx(x >> 4, y >> 4, z >> 4)];
    }
    static inline int mark_bit(int ox, int oy, int oz) { return (ox + 1) + 3 * ((oy + 1) + 3 * (oz + 1)); }
    inline uint32_t rnd(Ctx &c) { c.rng ^= c.rng << 13; c.rng ^= c.rng >> 17; c.rng ^= c.rng << 5; return c.rng; }

    inline void touch(int x, int y, int z);
    inline void mark_cell(int x, int y, int z, Ctx &c, bool changed);
    void flush_marks(const Ctx &c);
    inline void place(int i, uint8_t mat);                    // set material + init aux/flags (no marks)
    inline bool can_displace(uint8_t mover, uint8_t target) const;
    inline bool try_move(int sx, int sy, int sz, int tx, int ty, int tz, Ctx &c);
    bool fall(int x, int y, int z, int i, Ctx &c);
    void try_loosen(int x, int y, int z, int i, Ctx &c);
    void update_powder(int x, int y, int z, int i, Ctx &c);
    void update_liquid(int x, int y, int z, int i, Ctx &c);
    void update_liquid_fullness(int x, int y, int z, int i, Ctx &c);
    void update_gas(int x, int y, int z, int i, Ctx &c);
    void update_fire(int x, int y, int z, int i, Ctx &c);
    void update_burning(int x, int y, int z, int i, Ctx &c);
    void try_react(int x, int y, int z, int i, Ctx &c);
    void update_chunk(int ci);
    void rebuild_pass_lists();
    void rebuild_reaction_lut();
};

} // namespace godot

VARIANT_ENUM_CAST(godot::SimWorld::CellType);
