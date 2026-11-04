#pragma once
// P5 v4: data-driven material simulation in the Falling Everything (Noita) model.
//
// Every voxel has a material id (u8 in this spike; u16 in the real store), a
// flags byte (burning, ...) and an aux byte (remaining lifetime or fire hp).
// Every material has a cell_type (solid / liquid / gas / powder / fire) and
// Noita-named attributes (density, lifetime, burnable, fire_hp, ...). A
// reaction table (input a + input b -> output a + output b, probability)
// drives chemistry. Chunks of 16^3; only active chunks update; 8 parity passes
// on a persistent thread pool; per-chunk flag batching.

#include <godot_cpp/classes/ref_counted.hpp>
#include <godot_cpp/variant/array.hpp>
#include <godot_cpp/variant/dictionary.hpp>
#include <godot_cpp/variant/packed_byte_array.hpp>
#include <godot_cpp/variant/packed_float32_array.hpp>
#include <godot_cpp/variant/vector3.hpp>
#include <godot_cpp/variant/packed_int32_array.hpp>
#include <godot_cpp/variant/vector2i.hpp>
#include <godot_cpp/variant/vector3i.hpp>

#include <array>
#include <atomic>
#include <condition_variable>
#include <cstdint>
#include <functional>
#include <memory>
#include <mutex>
#include <thread>
#include <unordered_map>
#include <vector>

namespace godot {

class SimWorld : public RefCounted {
    GDCLASS(SimWorld, RefCounted)

public:
    // Powders (sand, gravel, dirt, snow, ash) are their own type: Noita spells them liquid +
    // liquid_sand, which set_materials still reads.
    enum CellType : uint8_t { CELL_SOLID = 0, CELL_LIQUID = 1, CELL_GAS = 2, CELL_FIRE = 3, CELL_POWDER = 4 };
    // FLAG_LOOSE: a static-ground cell (liquid_static) that a nearby change has shaken loose; it now
    // moves like its loose kind (Noita's static -> dynamic) until something places it again.
    // FLAG_SHAKEN: static ground that saw a fresh opening; it keeps rolling its loosen chance while the
    // opening stays open (crumbles over time instead of only on the tick of the change).
    // FLAG_SHAKEN on generated (still) liquid: it touches something it reacts with and stays awake for it.
    enum Flags : uint8_t { FLAG_BURNING = 1, FLAG_LOOSE = 2, FLAG_SHAKEN = 4 };
    static constexpr int CS = 16;
    static constexpr int MAX_MATERIALS = 256;
    static constexpr int FALL_SLOTS = 4;   // fluid slots whose falls are counted (water, lava, oil, sludge)
    static constexpr int MAX_DISP = 7; // must stay < CS/2 for the parity scheme
    static constexpr int SURFACE_REACH = 7; // liquid surface levelling looks this far (< CS/2 like MAX_DISP)
    // Partly full liquid cells in the terrain (Minecraft-style fluid models, one per level):
    // voxel value FLUID_LEVEL_BASE + fluid_slot * 8 + (eighths - 1) for 1..7 eighths; a full
    // cell keeps its material id. get_chunk_bytes / set_chunk_bytes (2 bytes) encode and
    // decode it, so the terrain shows the fullness and a reload restores it.
    static constexpr int FLUID_LEVEL_BASE = 256;

    struct MaterialDef {
        uint8_t cell_type = CELL_GAS; // air default
        bool liquid_static = false;    // ground: never moves
        int8_t fluid_slot = -1;        // terrain fluid models (FLUID_LEVEL_BASE), -1 = drawn as a cube
        bool burnable = false;
        uint8_t density = 0;           // 0..255 (Noita 0..15 scaled by 16)
        uint8_t disp = 0;              // lateral dispersion cells for liquids (from liquid_flow_speed)
        uint8_t gravity_prob = 255;    // chance per tick to fall (from liquid_gravity)
        uint8_t speed = 1;             // liquid substeps per tick (liquid_speed, ADR 0069 B): spreading runs this often
        uint8_t gas_up = 100, gas_horizontal = 100, gas_down = 0; // 0..255 chances
        uint8_t lifetime = 0;          // ticks (0 = infinite), scaled: aux = lifetime
        uint8_t life_step = 1;         // aux counts down every life_step ticks (data lifetimes past 255 ticks)
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
        bool replaceable = false;      // a plant liquids and powders wash away
        uint16_t film_life = 0;        // ADR 0069 B3: mean ticks a lone one-eighth film on a foreign floor lasts (0 = forever)
        uint8_t film_to = 0;           // what the film becomes when it goes (0 = air: dries; lava: a basalt crust)
        // ADR 0119 C: heat and state. heat_c: the material pulls its coarse heat cell toward this (0 = none). A moving
        // liquid at or over boil_c + boil_rise_c * chunk pressure becomes boils_to, at or under freeze_c freezes_to; a
        // gas at or under condense_c becomes condenses_to (one eighth); a solid at or over melt_c melts_to.
        int16_t heat_c = 0, boil_c = 32767, boil_rise_c = 0, freeze_c = -32768, condense_c = -32768, melt_c = 32767;
        uint8_t boils_to = 0, freezes_to = 0, condenses_to = 0, melts_to = 0;
        // ADR 0141: react_awake: a resting cell stays awake while a reaction partner touches it (sludge, fouled water);
        // wind_drift: share of the wind (set_wind) a gas follows, 0..255
        bool react_awake = false;
        uint8_t wind_drift = 0;
    };

    struct Reaction {
        uint8_t in1, in2, out1, out2;
        uint16_t probability; // 0..65535 per tick per neighbour test
        int16_t next = -1;    // the next reaction of the same input pair (ADR 0081): each rolls in turn, the first hit fires
        bool passive = false; // ADR 0141: only in1 rolls it, in2 stays inert (the soil sludge erodes); a film is spent only once it fires
    };
    // sample_cells flag bits beyond Flags: the cell moved this or the last tick (SAMPLE_MOVED), or is a gas or fire
    // born within the last eight ticks (SAMPLE_FRESH); sound loops key off them (ADR 0081)
    enum SampleBits : uint8_t { SAMPLE_MOVED = 8, SAMPLE_FRESH = 16 };

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
    // ADR 0050 pinned windows: one chunk becomes a wall (not ticked, never read as open) or open again; the active
    // chunks (anything still moving) as window-local chunk coords.
    void set_chunk_loaded(Vector3i p_chunk, bool p_loaded);
    Array get_active_chunks() const;
    // v5 (P10): fall acceleration in cells/tick^2 for powders and liquids (0 = one cell per tick, P5 behaviour).
    // At 6.25 cm and 30 Hz, 9.8 m/s^2 is 0.174 cells/tick^2.
    void set_gravity(float p_cells_per_tick2);
    // v5: scan only each active chunk's dirty box (default on); off = whole chunks (P5 behaviour) for comparison.
    void set_dirty_boxes(bool p_on);
    // ADR 0141: the wind in cells a tick along x and z (|v| <= 1); gases with a wind_drift follow it. Zero = still air.
    void set_wind(Vector2 p_wind);
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
    void set_column_flow(bool p_on);
    // ADR 0119 C: the outside temperature the heat field relaxes toward, and what a cell reports to the HUD
    void set_ambient_c(float p_c, bool p_fill = false);   // fill: the whole field takes it now (tests, a new world)
    void set_states(bool p_on);
    Dictionary cell_info(Vector3i p) const;   // {material, cell_type, moving, eighths, temp_c, pressure}
    float chunk_pressure(Vector3i p) const;   // gas cells / open cells of the cell's chunk, 0..1
    // ADR 0029: a cell vacated by a move into air stays free this tick, so the cell above can follow at once and a
    // column falls together (one layer per tick otherwise: slow motion at 6.25 cm). Static ground still sees it as a
    // fresh opening (ADR 0013). It also lets a wall of pressed liquid next to air flow out every tick instead of one
    // tick in eight. Off by default (the 0.5 m game keeps its pacing).
    void set_cascade(bool p_on);
    int count_liquid_eighths(int p_material) const;   // total eighths of a liquid (mass check)
    // v6: one tick on a background thread. Every other call that reads or writes cells waits for it.
    bool start_tick();          // false if a tick is still running
    bool is_tick_done() const;
    Dictionary finish_tick();   // waits, returns that tick's stats

    // ---- simulation ----
    Dictionary tick(int p_count);

    // ---- render feed / world streaming ----
    PackedByteArray get_chunk_bytes(Vector3i p_chunk, int p_depth_bytes = 1) const;
    // ADR 0119 B: the cells lo..hi (inclusive, chunk-local) of a chunk, 2 bytes each in VoxelBuffer order
    PackedByteArray get_box_bytes(Vector3i p_chunk, Vector3i p_lo, Vector3i p_hi) const;
    void set_chunk_bytes(Vector3i p_chunk, const PackedByteArray &p_bytes, int p_depth_bytes = 1); // load from world; wakes, not dirty
    // A column of ncy chunks from (c.x, c.y, c.z) up, from one buffer of CS x (ncy * CS) x CS voxels in
    // godot_voxel order (y fastest, then x, then z): one terrain copy per column instead of one per chunk.
    void set_column_bytes(Vector3i c, int ncy, const PackedByteArray &bytes, int depth_bytes);
    Array shift(Vector3i p_delta_chunks); // slide the window; returns vacated local chunks that must be loaded
    Array get_dirty_chunks() const;
    // v6: the changed cells of every dirty chunk as a box, for uploads finer than whole chunks:
    // 9 ints per chunk: chunk x,y,z, then min x,y,z and max x,y,z in local cells (inclusive).
    PackedInt32Array get_dirty_boxes() const;
    void clear_dirty();

    // ---- wrapped window (ADR 0029, ADR 0033: the bubble) ----
    // Toroidal window: call after configure (chunk counts must be powers of two). shift() then moves an
    // offset instead of memory: vacated chunks keep stale cells, are marked unloaded (walls, not ticked)
    // and must be loaded again; per-chunk state is reset. Cell coordinates stay window-local.
    bool set_wrapped(bool p_on);
    bool is_wrapped() const { return wrapped_; }
    // ---- debug / introspection ----
    int count_material(int p_material) const;
    int count_in_box(Vector3i p_min, Vector3i p_max, int p_material) const;
    int count_unsupported(int p_material) const;
    int count_burning() const;
    // Minecraft's animateTick for ambient particles: p_count random cells within p_radius of p_center
    // (local cells, a cube); returns 7 ints per hit (x, y, z, material, material above, material below,
    // flags) for loaded cells whose material has a nonzero byte in p_mask (256 bytes) or that burn.
    // Never waits: returns nothing while a tick runs (the caller tries again next frame).
    PackedInt32Array get_falls(int p_slot) const;
    PackedFloat32Array get_fall_streaks(int p_slot, Vector3 p_offset, float p_width) const;
    PackedInt32Array sample_cells(Vector3i p_center, int p_radius, int p_count, int p_seed, const PackedByteArray &p_mask) const;
    // Minimap feed (part D): count.x * count.y columns from window-local (min_xz.x, min_xz.y), every step voxels,
    // index cx + count.x * cz. Each: the first cell from the window top down that is not air, gas or fire, as
    // material | liquid_depth << 8 | (y + y_base) << 16 (liquid_depth: liquid cells down from a liquid top, at most
    // 255, else 0); 0 = nothing there or not loaded (chunks still loading are passed over). Empty while a tick runs.
    PackedInt32Array surface_map(Vector2i p_min_xz, Vector2i p_count, int p_step, int p_y_base) const;
    PackedInt32Array layer_histogram(int p_material) const;
    PackedInt32Array material_histogram() const;
    Dictionary chunk_info(Vector3i p_chunk) const;
    Dictionary get_stats() const;
    int get_active_chunk_count() const;
    Vector3i get_size_voxels() const;

protected:
    static void _bind_methods();

private:
    friend class RigidPieces;   // P12: reads and writes cells between ticks (rigid_pieces.h)
    friend class ShapeTool;     // ADR 0038: fills and reads shapes (shape_tool.h)
    friend class ClimateProbe;  // ADR 0040: shelter and heat around the player (climate_probe.h)
    void touch_cell(int x, int y, int z);              // touch / place for callers outside this file
    void place_cell(int x, int y, int z, uint8_t m);   // place + touch
    std::function<void()> pre_tick_;                   // runs on the driver thread before each background tick
    struct Pool {
        std::vector<std::thread> threads;
        std::mutex m;
        std::condition_variable cv_work, cv_done;
        const std::vector<int> *items = nullptr;
        std::function<void(int)> job;
        std::atomic<int> next{0};
        int remaining = 0;
        int slots = 0;   // helpers still to join this generation (ADR 0137)
        uint64_t generation = 0;
        bool stop = false;
        void start(int n);
        void run(const std::vector<int> &p_items, const std::function<void(int)> &p_job);
        void shutdown();
    };

    struct Marks { uint32_t wake = 0; uint32_t dirty = 0; uint32_t box[27] = {}; uint32_t dbox[27] = {}; };
    static constexpr int STREAK_CTX = 128;   // fall paths one chunk update keeps before it hands them over
    struct Ctx { int cx, cy, cz; Marks mk; uint32_t rng; uint64_t moved = 0; uint64_t reacted = 0;
        int64_t falls[FALL_SLOTS][4] = {};
        int nst = 0; int32_t st[STREAK_CTX * 5]; };   // fall paths this update: slot, x, z, lowest and highest cell passed   // liquid cells that fell this tick per fluid slot: count, sum x y z
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
    // single-cell reads: wait for a running tick, but not for the pre-tick phase (P12 rigid pieces take and
    // put back wood and leaf cells there; a read sees a cell just before or just after, and the main thread
    // does not stall for a felled tree)
    void wait_readable() const;
    void stop_driver();
    void wake_full(int ci);
    void chunk_of(int ci, int &cx, int &cy, int &cz) const;   // physical chunk index -> window-local chunk

    Vector3i chunks_;
    int W_ = 0, H_ = 0, D_ = 0;
    int CX_ = 0, CY_ = 0, CZ_ = 0;
    int threads_ = 1;
    uint32_t seed_ = 1;
    uint32_t tick_ = 0;
    uint16_t stamp_ = 1;           // the tick's stamp: freshness (openings, edits, cascades) counts in ticks
    uint16_t mark_ = 1;            // "moved this pass" marker: the tick's stamp in the main pass, a value with the top bit set in a substep (ADR 0069 B)

    std::vector<MaterialDef> mats_;
    std::vector<Reaction> reactions_;
    std::vector<int16_t> react_lut_; // [a*256+b] -> reaction index or -1
    std::vector<uint8_t> reactive_;  // per material: 1 if it appears as an input of any reaction
    std::vector<uint8_t> switches_;  // per material: SW_COLD a cell of it changes state in the cold (ADR 0119 C: freeze,
                                     // condense), SW_WARM it melts (ADR 0137)
    std::vector<uint8_t> switch_;    // per chunk: may hold such a cell, a melter only where exposed (all bits until a
                                     // whole-chunk scan finds none, ADR 0137)
    static constexpr uint8_t SW_COLD = 1, SW_WARM = 2, SW_ALL = 3;
    int melt_min_c_ = 32767;         // the lowest melt_c of any material
    std::vector<uint8_t> thaw_;      // per chunk: its next scan may melt (its sleeping heat turn found it warm)
    void on_loaded(int ci);          // ADR 0137: a chunk took terrain data: awake once, heat at the ambient
    std::vector<uint8_t> inert_;     // per material: 1 if nothing ever needs to run on it (static solid, no lifetime, not reactive, not burnable)
    bool any_reactions_ = false;
    bool any_fire_ = false;

    struct Cell { uint8_t mat, flags, aux, pad; }; // 4 bytes, one cache line touch per voxel
    std::vector<Cell> cells_;
    int16_t slot_material_[32];         // fluid_slot -> the moving material that a partial level decodes to
    std::vector<uint8_t> displace_lut_; // [mover*256+target] -> 1 if mover may swap into target
    std::vector<uint16_t> moved_;
    std::unique_ptr<std::atomic<uint8_t>[]> active_now_;
    std::unique_ptr<std::atomic<uint8_t>[]> fast_;   // ADR 0137: moving liquid with substeps left at the chunk's last scan this tick
    bool needs_substep(int ci) const;
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
    std::atomic<bool> pre_phase_{false};   // the driver runs pre_tick_
    TickOut last_;
    bool dirty_boxes_ = true;
    bool fullness_ = false;
    bool column_flow_ = false;     // ADR 0119 A: moving liquids level as columns by head (set_column_flow)
    // ADR 0119 C: a coarse heat field (one int16 degrees C per 4^3 cells, aligned to chunks so a chunk's update owns
    // its own coarse cells), a pressure per chunk (gas / open cells, 0..255) and the outside temperature
    static constexpr int HC = 4;
    std::vector<int16_t> heat_;
    std::vector<uint8_t> pressure_;
    int16_t ambient_c_ = 18;
    bool states_ = true;
    inline int hidx(int x, int y, int z) const {
        return (((x + ox_) & mx_) >> 2) + (W_ >> 2) * ((((y + oy_) & my_) >> 2) + (H_ >> 2) * (((z + oz_) & mz_) >> 2));
    }
    void relax_heat(int ci, bool slow_turn);   // diffuse a chunk's coarse cells and pull them toward the ambient
    int max_speed_ = 1;            // the largest liquid_speed among the materials: extra passes per tick
    int substep_ = 0;              // 0 = the main pass; 1.. = a liquid substep (only liquids with speed > substep_ move)
    bool cascade_ = false;
    // toroidal window (set_wrapped): physical = (local + offset) & mask; non-wrapped: offset 0, mask -1
    bool wrapped_ = false;
    int ox_ = 0, oy_ = 0, oz_ = 0;        // cells
    int mx_ = -1, my_ = -1, mz_ = -1;
    int ocx_ = 0, ocy_ = 0, ocz_ = 0;     // chunks
    int cmx_ = -1, cmy_ = -1, cmz_ = -1;
    uint8_t washable_[MAX_MATERIALS] = {};      // replaceable plant (grass, flowers): liquids and powders move through it, destroying it
    inline bool open_air(uint8_t m) const { return m == 0 || washable_[m]; }
    // A floor a film can dry or crust on: rock, powder or a still liquid. A lava film over steam crusted to basalt
    // and hung in the air once the steam moved (playtest 2026-11-03).
    inline bool film_floor(uint8_t m) const {
        if (m == 0 || m >= mats_.size()) return false;
        const MaterialDef &d = mats_[m];
        return d.cell_type == CELL_SOLID || d.cell_type == CELL_POWDER || d.liquid_static;
    }
    std::vector<int> pass_lists_[8];
    std::atomic<uint64_t> moved_count_{0};
    std::atomic<uint64_t> react_count_{0};
    std::atomic<int64_t> falls_[FALL_SLOTS][4] = {};   // this tick's Ctx::falls, summed (the waterfall sound, ADR 0103)
    int64_t falls_last_[FALL_SLOTS][4] = {};           // the last finished tick's, what get_falls returns
    // ADR 0138: the cells each falling liquid cell passed this tick (the air it crossed), for the stream streaks
    static constexpr int STREAKS = 8192;
    std::atomic<int> streak_n_{0};
    int32_t streak_buf_[STREAKS * 5] = {};
    static constexpr int STREAK_TICKS = 3;   // a stream is the paths of the last few ticks: one column gets a cell every 2-3
    std::vector<std::array<int32_t, 5>> streak_hist_[STREAK_TICKS];
    std::vector<int32_t> streaks_last_;   // merged runs of the last STREAK_TICKS ticks: slot, x, z, y0, y1
    mutable std::mutex streak_m_;
    void build_streaks();
    Pool pool_;

    inline int idx(int x, int y, int z) const { return ((x + ox_) & mx_) + W_ * (((y + oy_) & my_) + H_ * ((z + oz_) & mz_)); }
    inline int cidx(int cx, int cy, int cz) const { return ((cx + ocx_) & cmx_) + CX_ * (((cy + ocy_) & cmy_) + CY_ * ((cz + ocz_) & cmz_)); }
    inline bool in_world(int x, int y, int z) const {
        return x >= 0 && y >= 0 && z >= 0 && x < W_ && y < H_ && z < D_;
    }
    // In world AND inside a chunk whose data has been loaded (streaming bubbles load
    // chunks over several frames; an unloaded neighbour must not look like air).
    // loaded_: 0 = waiting for data, 1 = loaded.
    inline bool is_open(int x, int y, int z) const {
        return in_world(x, y, z) && loaded_[cidx(x >> 4, y >> 4, z >> 4)] == 1;
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
    void reservoir(int x, int y, int z, int i, Ctx &c);
    bool still_liquid(int x, int y, int z, int slot) const;   // generated (static) liquid of that fluid slot
    std::atomic<int> still_woken_{0};   // still liquid cells that became moving liquid (ADR 0068)
    void update_powder(int x, int y, int z, int i, Ctx &c);
    static inline void count_fall(const MaterialDef &m, int x, int y, int z, Ctx &c) {
        if (m.fluid_slot < 0 || m.fluid_slot >= FALL_SLOTS) return;
        int64_t *f = c.falls[m.fluid_slot];
        f[0]++; f[1] += x; f[2] += y; f[3] += z;
    }
    void update_liquid(int x, int y, int z, int i, Ctx &c);
    void update_liquid_fullness(int x, int y, int z, int i, Ctx &c);
    void update_liquid_column(int x, int y, int z, int i, Ctx &c);   // ADR 0119 A
    // ADR 0138: past FLAT_REACH a resting flood keeps a tilt of an eighth per reach. Cells whose conveyor look failed at
    // an edge one eighth down seed a serial pass after the tick that levels the connected surface they are on
    static constexpr int LEVEL_SEEDS = 256;
    static constexpr int LEVEL_MAX_COLUMNS = 8192;
    std::atomic<int> level_n_{0};
    int32_t level_seeds_[LEVEL_SEEDS * 3] = {};
    void level_surfaces();
    static constexpr int FLAT_REACH = 14;     // the surface conveyor looks this far and writes one cell over (1 + 14 < CS: ADR 0138)
    static constexpr int COLUMN_REACH = 7;    // cells a column scan follows up or down (< CS / 2 like SURFACE_REACH: chunks two apart never touch one cell)
    void update_gas(int x, int y, int z, int i, Ctx &c);
    void update_fire(int x, int y, int z, int i, Ctx &c);
    void update_burning(int x, int y, int z, int i, Ctx &c);
    void try_react(int x, int y, int z, int i, Ctx &c);
    bool spend_partial(int x, int y, int z, int nx, int ny, int nz, int i, int ni, Ctx &c);
    void react_keep_awake(int x, int y, int z, int i, Ctx &c);   // ADR 0141
    bool wind_drift(int x, int y, int z, const MaterialDef &m, Ctx &c);   // ADR 0141
    static constexpr int ROOF_SCAN = 96;   // cells (12 m) up a drifting gas looks for a roof that shelters it from the wind
    int wind_x_ = 0, wind_z_ = 0;      // the wind's sign along x and z
    uint32_t wind_p_ = 0, wind_xshare_ = 0;   // chance per tick (0..65535) a drifting gas moves with it, and the x share of it
    void update_chunk(int ci);
    void rebuild_pass_lists();
    void rebuild_reaction_lut();
};

} // namespace godot

VARIANT_ENUM_CAST(godot::SimWorld::CellType);
