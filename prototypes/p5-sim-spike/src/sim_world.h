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
    enum Flags : uint8_t { FLAG_BURNING = 1 };
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
    void ignite(Vector3i p);

    // ---- simulation ----
    Dictionary tick(int p_count);

    // ---- render feed ----
    PackedByteArray get_chunk_bytes(Vector3i p_chunk, int p_depth_bytes = 1) const;
    Array get_dirty_chunks() const;
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

    struct Marks { uint32_t wake = 0; uint32_t dirty = 0; };
    struct Ctx { int cx, cy, cz; Marks mk; uint32_t rng; uint64_t moved = 0; uint64_t reacted = 0; };

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
    std::vector<int> pass_lists_[8];
    std::atomic<uint64_t> moved_count_{0};
    std::atomic<uint64_t> react_count_{0};
    Pool pool_;

    inline int idx(int x, int y, int z) const { return x + W_ * (y + H_ * z); }
    inline int cidx(int cx, int cy, int cz) const { return cx + CX_ * (cy + CY_ * cz); }
    inline bool in_world(int x, int y, int z) const {
        return x >= 0 && y >= 0 && z >= 0 && x < W_ && y < H_ && z < D_;
    }
    static inline int mark_bit(int ox, int oy, int oz) { return (ox + 1) + 3 * ((oy + 1) + 3 * (oz + 1)); }
    inline uint32_t rnd(Ctx &c) { c.rng ^= c.rng << 13; c.rng ^= c.rng >> 17; c.rng ^= c.rng << 5; return c.rng; }

    inline void touch(int x, int y, int z);
    inline void mark_cell(int x, int y, int z, Ctx &c, bool changed);
    void flush_marks(const Ctx &c);
    inline void place(int i, uint8_t mat);                    // set material + init aux/flags (no marks)
    inline bool can_displace(uint8_t mover, uint8_t target) const;
    inline bool try_move(int sx, int sy, int sz, int tx, int ty, int tz, Ctx &c);
    void update_powder(int x, int y, int z, int i, Ctx &c);
    void update_liquid(int x, int y, int z, int i, Ctx &c);
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
