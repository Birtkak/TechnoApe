#include "sim_world.h"

#include <godot_cpp/core/class_db.hpp>
#include <godot_cpp/variant/utility_functions.hpp>

#include <algorithm>
#include <chrono>
#include <cstring>

using namespace godot;

static const int DX4[4] = {1, -1, 0, 0};
static const int DZ4[4] = {0, 0, 1, -1};
static const int NX6[6] = {1, -1, 0, 0, 0, 0};
static const int NY6[6] = {0, 0, 1, -1, 0, 0};
static const int NZ6[6] = {0, 0, 0, 0, 1, -1};

// ---------- thread pool ----------

void SimWorld::Pool::start(int n) {
    shutdown();
    stop = false;
    for (int i = 0; i < n; i++) {
        threads.emplace_back([this]() {
            uint64_t seen = 0;
            for (;;) {
                {
                    std::unique_lock<std::mutex> lk(m);
                    cv_work.wait(lk, [&] { return stop || generation != seen; });
                    if (stop) return;
                    seen = generation;
                }
                for (;;) {
                    int i = next.fetch_add(1);
                    if (i >= (int)items->size()) break;
                    job((*items)[i]);
                }
                {
                    std::lock_guard<std::mutex> lk(m);
                    if (--remaining == 0) cv_done.notify_one();
                }
            }
        });
    }
}

void SimWorld::Pool::run(const std::vector<int> &p_items, const std::function<void(int)> &p_job) {
    if (p_items.empty()) return;
    if (threads.empty()) {
        for (int i : p_items) p_job(i);
        return;
    }
    {
        std::lock_guard<std::mutex> lk(m);
        items = &p_items;
        job = p_job;
        next.store(0);
        remaining = (int)threads.size();
        generation++;
    }
    cv_work.notify_all();
    std::unique_lock<std::mutex> lk(m);
    cv_done.wait(lk, [&] { return remaining == 0; });
}

void SimWorld::Pool::shutdown() {
    {
        std::lock_guard<std::mutex> lk(m);
        stop = true;
    }
    cv_work.notify_all();
    for (auto &t : threads) t.join();
    threads.clear();
}

// ---------- bindings ----------

SimWorld::SimWorld() {
    mats_.resize(1); // material 0 = air: gas, density 0
    rebuild_reaction_lut();
}
SimWorld::~SimWorld() { stop_driver(); pool_.shutdown(); }

void SimWorld::_bind_methods() {
    ClassDB::bind_method(D_METHOD("configure", "chunks", "threads", "seed"), &SimWorld::configure);
    ClassDB::bind_method(D_METHOD("set_materials", "defs"), &SimWorld::set_materials);
    ClassDB::bind_method(D_METHOD("set_reactions", "reactions"), &SimWorld::set_reactions);
    ClassDB::bind_method(D_METHOD("get_material_count"), &SimWorld::get_material_count);
    ClassDB::bind_method(D_METHOD("fill_box", "min", "max", "material"), &SimWorld::fill_box);
    ClassDB::bind_method(D_METHOD("scatter", "min", "max", "material", "density", "seed"), &SimWorld::scatter);
    ClassDB::bind_method(D_METHOD("set_voxel", "pos", "material"), &SimWorld::set_voxel);
    ClassDB::bind_method(D_METHOD("get_voxel", "pos"), &SimWorld::get_voxel);
    ClassDB::bind_method(D_METHOD("get_flags", "pos"), &SimWorld::get_flags);
    ClassDB::bind_method(D_METHOD("ignite", "pos"), &SimWorld::ignite);
    ClassDB::bind_method(D_METHOD("fill_sphere", "center", "radius", "material", "keep"), &SimWorld::fill_sphere, DEFVAL(PackedInt32Array()));
    ClassDB::bind_method(D_METHOD("ignite_box", "min", "max"), &SimWorld::ignite_box);
    ClassDB::bind_method(D_METHOD("mark_all_loaded", "loaded"), &SimWorld::mark_all_loaded);
    ClassDB::bind_method(D_METHOD("set_chunk_loaded", "chunk", "loaded"), &SimWorld::set_chunk_loaded);
    ClassDB::bind_method(D_METHOD("get_active_chunks"), &SimWorld::get_active_chunks);
    ClassDB::bind_method(D_METHOD("tick", "count"), &SimWorld::tick);
    ClassDB::bind_method(D_METHOD("get_chunk_bytes", "chunk", "depth_bytes"), &SimWorld::get_chunk_bytes, DEFVAL(1));
    ClassDB::bind_method(D_METHOD("set_chunk_bytes", "chunk", "bytes", "depth_bytes"), &SimWorld::set_chunk_bytes, DEFVAL(1));
    ClassDB::bind_method(D_METHOD("set_column_bytes", "chunk", "count", "bytes", "depth_bytes"), &SimWorld::set_column_bytes, DEFVAL(2));
    ClassDB::bind_method(D_METHOD("shift", "delta_chunks"), &SimWorld::shift);
    ClassDB::bind_method(D_METHOD("get_dirty_chunks"), &SimWorld::get_dirty_chunks);
    ClassDB::bind_method(D_METHOD("get_dirty_boxes"), &SimWorld::get_dirty_boxes);
    ClassDB::bind_method(D_METHOD("clear_dirty"), &SimWorld::clear_dirty);
    ClassDB::bind_method(D_METHOD("count_material", "material"), &SimWorld::count_material);
    ClassDB::bind_method(D_METHOD("count_in_box", "min", "max", "material"), &SimWorld::count_in_box);
    ClassDB::bind_method(D_METHOD("count_unsupported", "material"), &SimWorld::count_unsupported);
    ClassDB::bind_method(D_METHOD("count_burning"), &SimWorld::count_burning);
    ClassDB::bind_method(D_METHOD("sample_cells", "center", "radius", "count", "seed", "mask"), &SimWorld::sample_cells);
    ClassDB::bind_method(D_METHOD("layer_histogram", "material"), &SimWorld::layer_histogram);
    ClassDB::bind_method(D_METHOD("surface_map", "min_xz", "count", "step", "y_base"), &SimWorld::surface_map);
    ClassDB::bind_method(D_METHOD("material_histogram"), &SimWorld::material_histogram);
    ClassDB::bind_method(D_METHOD("chunk_info", "chunk"), &SimWorld::chunk_info);
    ClassDB::bind_method(D_METHOD("get_stats"), &SimWorld::get_stats);
    ClassDB::bind_method(D_METHOD("get_active_chunk_count"), &SimWorld::get_active_chunk_count);
    ClassDB::bind_method(D_METHOD("get_size_voxels"), &SimWorld::get_size_voxels);
    ClassDB::bind_method(D_METHOD("set_gravity", "cells_per_tick2"), &SimWorld::set_gravity);
    ClassDB::bind_method(D_METHOD("set_dirty_boxes", "on"), &SimWorld::set_dirty_boxes);
    ClassDB::bind_method(D_METHOD("set_focus", "cell"), &SimWorld::set_focus);
    ClassDB::bind_method(D_METHOD("set_lod", "radius_chunks", "divisor"), &SimWorld::set_lod);
    ClassDB::bind_method(D_METHOD("set_cell_budget", "cells"), &SimWorld::set_cell_budget);
    ClassDB::bind_method(D_METHOD("set_liquid_fullness", "on"), &SimWorld::set_liquid_fullness);
    ClassDB::bind_method(D_METHOD("count_liquid_eighths", "material"), &SimWorld::count_liquid_eighths);
    ClassDB::bind_method(D_METHOD("start_tick"), &SimWorld::start_tick);
    ClassDB::bind_method(D_METHOD("is_tick_done"), &SimWorld::is_tick_done);
    ClassDB::bind_method(D_METHOD("finish_tick"), &SimWorld::finish_tick);
    ClassDB::bind_method(D_METHOD("set_wrapped", "on"), &SimWorld::set_wrapped);
    ClassDB::bind_method(D_METHOD("is_wrapped"), &SimWorld::is_wrapped);
    ClassDB::bind_method(D_METHOD("set_cascade", "on"), &SimWorld::set_cascade);
    BIND_ENUM_CONSTANT(CELL_SOLID);
    BIND_ENUM_CONSTANT(CELL_LIQUID);
    BIND_ENUM_CONSTANT(CELL_GAS);
    BIND_ENUM_CONSTANT(CELL_FIRE);
    BIND_ENUM_CONSTANT(CELL_POWDER);
}

// ---------- setup ----------

void SimWorld::configure(Vector3i p_chunks, int p_threads, int p_seed) {
    stop_driver();
    pool_.shutdown();
    chunks_ = p_chunks;
    CX_ = p_chunks.x; CY_ = p_chunks.y; CZ_ = p_chunks.z;
    W_ = CX_ * CS; H_ = CY_ * CS; D_ = CZ_ * CS;
    threads_ = std::max(1, p_threads);
    seed_ = (uint32_t)p_seed | 1u;
    tick_ = 0;
    stamp_ = 1;
    size_t n = (size_t)W_ * H_ * D_;
    size_t nc = (size_t)CX_ * CY_ * CZ_;
    cells_.assign(n, Cell{0, 0, 0, 0});
    moved_.assign(n, 0);
    active_now_.reset(new std::atomic<uint8_t>[nc]);
    active_next_.reset(new std::atomic<uint8_t>[nc]);
    dirty_.reset(new std::atomic<uint8_t>[nc]);
    box_now_.reset(new std::atomic<uint32_t>[nc]);
    box_next_.reset(new std::atomic<uint32_t>[nc]);
    dbox_.reset(new std::atomic<uint32_t>[nc]);
    for (size_t i = 0; i < nc; i++) { active_now_[i].store(0); active_next_[i].store(0); dirty_[i].store(0); box_now_[i].store(0); box_next_[i].store(0); dbox_[i].store(0); }
    defer_age_.assign(nc, 0);
    loaded_.assign(nc, 1); // default: everything counts as loaded (fill_box/scatter worlds); streaming callers clear it
    wrapped_ = false;
    ox_ = oy_ = oz_ = 0; ocx_ = ocy_ = ocz_ = 0;
    mx_ = my_ = mz_ = -1; cmx_ = cmy_ = cmz_ = -1;
    rebuild_pass_lists();
    if (threads_ > 1) pool_.start(threads_);
}

static int dget_int(const Dictionary &d, const char *k, int def) {
    return d.has(k) ? (int)d[k] : def;
}
static double dget_f(const Dictionary &d, const char *k, double def) {
    return d.has(k) ? (double)d[k] : def;
}
static bool dget_b(const Dictionary &d, const char *k, bool def) {
    return d.has(k) ? (bool)d[k] : def;
}
static uint8_t clamp8(double v) { return (uint8_t)std::clamp(v, 0.0, 255.0); }

void SimWorld::set_materials(const Array &p_defs) {
    mats_.clear();
    mats_.resize(1); // air
    max_speed_ = 1;
    for (int16_t &s : slot_material_) s = 0;
    any_fire_ = false;
    for (int i = 0; i < p_defs.size() && mats_.size() < MAX_MATERIALS; i++) {
        Dictionary d = p_defs[i];
        MaterialDef m;
        String ct = d.has("cell_type") ? (String)d["cell_type"] : String("liquid");
        if (ct == "solid") m.cell_type = CELL_SOLID;
        else if (ct == "gas") m.cell_type = CELL_GAS;
        else if (ct == "fire") m.cell_type = CELL_FIRE;
        else if (ct == "powder") m.cell_type = CELL_POWDER;
        else m.cell_type = CELL_LIQUID;
        if (m.cell_type == CELL_LIQUID && dget_b(d, "liquid_sand", false)) m.cell_type = CELL_POWDER;   // Noita spelling
        m.liquid_static = dget_b(d, "liquid_static", false);
        m.fluid_slot = (int8_t)std::clamp(dget_int(d, "fluid_slot", -1), -1, 31);
        m.burnable = dget_b(d, "burnable", false);
        m.replaceable = dget_b(d, "replaceable", false);
        washable_[mats_.size()] = m.replaceable ? 1 : 0;
        m.density = clamp8(dget_f(d, "density", 1.0) * 16.0);
        m.disp = (uint8_t)std::clamp((int)std::lround(dget_f(d, "liquid_flow_speed", 0.9) * MAX_DISP), 0, MAX_DISP);
        m.gravity_prob = clamp8(dget_f(d, "liquid_gravity", 0.5) * 2.0 * 255.0); // 0.5 -> always falls
        m.speed = (uint8_t)std::clamp(dget_int(d, "liquid_speed", 1), 1, 4);   // ADR 0069 B: water 3, oil 2, lava 1
        m.gas_up = clamp8(dget_f(d, "gas_upwards_speed", 100));
        m.gas_horizontal = clamp8(dget_f(d, "gas_horizontal_speed", 100));
        m.gas_down = clamp8(dget_f(d, "gas_downwards_speed", 0));
        m.lifetime = clamp8(dget_int(d, "lifetime", 0));
        m.film_life = (uint16_t)std::clamp(dget_int(d, "film_lifetime_ticks", 0), 0, 65535);
        m.film_to = clamp8(dget_int(d, "film_convert_to_material", 0));
        m.fire_hp = clamp8(dget_int(d, "fire_hp", 0));
        m.temperature_of_fire = clamp8(dget_int(d, "temperature_of_fire", 10));
        m.autoignition_temperature = clamp8(dget_int(d, "autoignition_temperature", 100));
        m.on_fire_convert_to = clamp8(dget_int(d, "on_fire_convert_to_material", 0));
        m.on_fire_flame_material = clamp8(dget_int(d, "on_fire_flame_material", 0));
        m.on_fire_smoke_material = clamp8(dget_int(d, "on_fire_smoke_material", 0));
        m.generates_smoke = clamp8(dget_f(d, "generates_smoke", 0) * 12.75);
        // Chance that a burning neighbour's attempt ignites this material. Thin, short-burning
        // fuels catch easily (grass, leaves), thick ones slowly (logs): 10 / fire_hp, clamped to
        // [1/8, 1]. With a fixed 1/8 a grass voxel (fire_hp 10) died before passing the fire on
        // and grass fires never spread (fire_bench.gd). Data may override with "ignite_chance" 0..1.
        {
            double ic = d.has("ignite_chance") ? dget_f(d, "ignite_chance", 0.125)
                                               : std::clamp(10.0 / std::max(1, (int)m.fire_hp), 0.125, 1.0);
            m.ignite_chance = (uint16_t)std::clamp((int)std::lround(ic * 65535.0), 0, 65535);
        }
        // Static ground shaken loose by nearby changes (Noita): unsupported cells fall, static water
        // flows once it has an opening, natural sand/gravel (they have a "loose" kind) erode into
        // slopes. Only next to cells that changed this or the previous tick, so a freshly loaded world
        // (caves, overhangs) never collapses by itself. Data: "loosen_chance", "erode_chance" (0..1).
        if (m.liquid_static) {
            double lc = dget_f(d, "loosen_chance", 0.35);
            double ec = dget_f(d, "erode_chance", (m.cell_type == CELL_POWDER && d.has("loose")) ? 0.04 : 0.0);
            m.loosen = (uint16_t)std::clamp((int)std::lround(lc * 65535.0), 0, 65535);
            m.erode = (uint16_t)std::clamp((int)std::lround(ec * 65535.0), 0, 65535);
        }
        if (m.cell_type == CELL_FIRE || m.burnable) any_fire_ = true;
        if (m.fluid_slot >= 0 && !m.liquid_static && slot_material_[m.fluid_slot] == 0) slot_material_[m.fluid_slot] = (int16_t)mats_.size();
        if (m.cell_type == CELL_LIQUID && !m.liquid_static) max_speed_ = std::max(max_speed_, (int)m.speed);
        mats_.push_back(m);
    }
    rebuild_reaction_lut();
}

void SimWorld::set_reactions(const Array &p_reactions) {
    reactions_.clear();
    for (int i = 0; i < p_reactions.size(); i++) {
        Dictionary d = p_reactions[i];
        Reaction r;
        r.in1 = clamp8(dget_int(d, "input_cell1", 0));
        r.in2 = clamp8(dget_int(d, "input_cell2", 0));
        r.out1 = clamp8(dget_int(d, "output_cell1", 0));
        r.out2 = clamp8(dget_int(d, "output_cell2", 0));
        r.probability = (uint16_t)std::clamp(dget_f(d, "probability", 0.0) * 65535.0, 0.0, 65535.0);
        reactions_.push_back(r);
    }
    rebuild_reaction_lut();
}

void SimWorld::rebuild_reaction_lut() {
    react_lut_.assign(MAX_MATERIALS * MAX_MATERIALS, -1);
    reactive_.assign(MAX_MATERIALS, 0);
    inert_.assign(MAX_MATERIALS, 0);
    any_reactions_ = !reactions_.empty();
    for (size_t i = 0; i < reactions_.size(); i++) {
        react_lut_[reactions_[i].in1 * MAX_MATERIALS + reactions_[i].in2] = (int16_t)i;
        reactive_[reactions_[i].in1] = 1;
        reactive_[reactions_[i].in2] = 1;
    }
    displace_lut_.assign(MAX_MATERIALS * MAX_MATERIALS, 0);
    for (size_t a = 0; a < mats_.size(); a++)
        for (size_t b = 0; b < mats_.size(); b++) {
            const MaterialDef &m = mats_[a];
            const MaterialDef &t = mats_[b];
            bool ok = !(t.cell_type == CELL_SOLID || t.liquid_static) && (t.cell_type == CELL_FIRE || t.density < m.density);
            displace_lut_[a * MAX_MATERIALS + b] = ok ? 1 : 0;
        }
    for (size_t m = 1; m < mats_.size(); m++) {
        const MaterialDef &d = mats_[m];
        bool static_cell = d.cell_type == CELL_SOLID || ((d.cell_type == CELL_LIQUID || d.cell_type == CELL_POWDER) && d.liquid_static);
        inert_[m] = static_cell && d.lifetime == 0 && !d.burnable && !reactive_[m] && d.loosen == 0 && d.erode == 0;
    }
}

int SimWorld::get_material_count() const { return (int)mats_.size(); }

void SimWorld::rebuild_pass_lists() {
    for (auto &l : pass_lists_) l.clear();
    for (int cz = 0; cz < CZ_; cz++)
        for (int cy = 0; cy < CY_; cy++)
            for (int cx = 0; cx < CX_; cx++)
                pass_lists_[(cx & 1) | ((cy & 1) << 1) | ((cz & 1) << 2)].push_back(cidx(cx, cy, cz));
}

inline void SimWorld::place(int i, uint8_t mat) {
    cells_[i].mat = mat;
    cells_[i].flags = 0;
    cells_[i].aux = mat < mats_.size() ? mats_[mat].lifetime : 0;
    cells_[i].pad = 0;   // liquid fullness: 0 = full
}

inline void SimWorld::touch(int x, int y, int z) {
    int cx = x / CS, cy = y / CS, cz = z / CS;
    int ci = cidx(cx, cy, cz);
    dirty_[ci].store(1, std::memory_order_relaxed);
    merge_dbox(ci, box_point(x & (CS - 1), y & (CS - 1), z & (CS - 1)));
    moved_[idx(x, y, z)] = stamp_;   // an edit is a change: static ground next to it may loosen
    wake_full(ci);
    int lx = x & (CS - 1), ly = y & (CS - 1), lz = z & (CS - 1);
    if (lx == 0 && cx > 0) wake_full(cidx(cx - 1, cy, cz));
    if (lx == CS - 1 && cx < CX_ - 1) wake_full(cidx(cx + 1, cy, cz));
    if (ly == 0 && cy > 0) wake_full(cidx(cx, cy - 1, cz));
    if (ly == CS - 1 && cy < CY_ - 1) wake_full(cidx(cx, cy + 1, cz));
    if (lz == 0 && cz > 0) wake_full(cidx(cx, cy, cz - 1));
    if (lz == CS - 1 && cz < CZ_ - 1) wake_full(cidx(cx, cy, cz + 1));
}

void SimWorld::touch_cell(int x, int y, int z) { touch(x, y, z); }

void SimWorld::place_cell(int x, int y, int z, uint8_t m) {
    place(idx(x, y, z), m);
    touch(x, y, z);
}

void SimWorld::fill_box(Vector3i p_min, Vector3i p_max, int p_material) {
    wait_idle();
    uint8_t m = (uint8_t)std::clamp(p_material, 0, (int)mats_.size() - 1);
    for (int z = std::max(0, p_min.z); z < std::min(D_, p_max.z); z++)
        for (int y = std::max(0, p_min.y); y < std::min(H_, p_max.y); y++)
            for (int x = std::max(0, p_min.x); x < std::min(W_, p_max.x); x++) {
                place(idx(x, y, z), m);
                touch(x, y, z);
            }
}

void SimWorld::scatter(Vector3i p_min, Vector3i p_max, int p_material, float p_density, int p_seed) {
    wait_idle();
    uint8_t m = (uint8_t)std::clamp(p_material, 0, (int)mats_.size() - 1);
    uint32_t s = (uint32_t)p_seed | 1u;
    for (int z = std::max(0, p_min.z); z < std::min(D_, p_max.z); z++)
        for (int y = std::max(0, p_min.y); y < std::min(H_, p_max.y); y++)
            for (int x = std::max(0, p_min.x); x < std::min(W_, p_max.x); x++) {
                s ^= s << 13; s ^= s >> 17; s ^= s << 5;
                if ((s & 0xFFFF) < (uint32_t)(p_density * 65535.0f)) {
                    place(idx(x, y, z), m);
                    touch(x, y, z);
                }
            }
}

void SimWorld::set_voxel(Vector3i p, int p_material) {
    wait_idle();
    if (!in_world(p.x, p.y, p.z)) return;
    place(idx(p.x, p.y, p.z), (uint8_t)std::clamp(p_material, 0, (int)mats_.size() - 1));
    touch(p.x, p.y, p.z);
}

int SimWorld::get_voxel(Vector3i p) const {
    wait_readable();
    if (!in_world(p.x, p.y, p.z)) return -1;
    return cells_[idx(p.x, p.y, p.z)].mat;
}

int SimWorld::get_flags(Vector3i p) const {
    wait_readable();
    if (!in_world(p.x, p.y, p.z)) return 0;
    return cells_[idx(p.x, p.y, p.z)].flags;
}

void SimWorld::mark_all_loaded(bool p_loaded) {
    wait_idle();
    std::fill(loaded_.begin(), loaded_.end(), p_loaded ? 1 : 0);
}

void SimWorld::set_chunk_loaded(Vector3i c, bool p_loaded) {
    wait_idle();
    if (c.x < 0 || c.y < 0 || c.z < 0 || c.x >= CX_ || c.y >= CY_ || c.z >= CZ_) return;
    const int ci = cidx(c.x, c.y, c.z);
    loaded_[ci] = p_loaded ? 1 : 0;
    if (p_loaded) wake_full(ci);
    else { active_next_[ci].store(0); box_next_[ci].store(0); }
}

Array SimWorld::get_active_chunks() const {
    wait_idle();
    Array a;
    for (int cz = 0; cz < CZ_; cz++)
        for (int cy = 0; cy < CY_; cy++)
            for (int cx = 0; cx < CX_; cx++) {
                const int ci = cidx(cx, cy, cz);
                if (loaded_[ci] == 1 && active_next_[ci].load(std::memory_order_relaxed)) a.push_back(Vector3i(cx, cy, cz));
            }
    return a;
}

void SimWorld::ignite(Vector3i p) {
    wait_idle();
    if (!in_world(p.x, p.y, p.z)) return;
    int i = idx(p.x, p.y, p.z);
    const MaterialDef &m = mats_[cells_[i].mat];
    if (!m.burnable) return;
    cells_[i].flags |= FLAG_BURNING;
    cells_[i].aux = m.fire_hp;
    touch(p.x, p.y, p.z);
}

// Mass edit (ADR 0012: explosions, black holes, big digs never iterate voxels in
// GDScript). Every cell whose centre lies within p_radius of p_center becomes
// p_material, except cells of a material listed in p_keep (block entities, bedrock)
// and cells of chunks that are not loaded yet (their data would be overwritten on
// load). Dirty boxes and wakes are merged once per chunk, not per cell, so a
// 32-voxel sphere costs about a millisecond. The changed chunks then reach the
// terrain through the bubble's budgeted push queue like any other change.
// Returns the number of cells changed.
int SimWorld::fill_sphere(Vector3 p_center, float p_radius, int p_material, const PackedInt32Array &p_keep) {
    wait_idle();
    if (p_radius <= 0.0f || mats_.empty()) return 0;
    const uint8_t m = (uint8_t)std::clamp(p_material, 0, (int)mats_.size() - 1);
    bool keep[256] = {};
    for (int k : p_keep)
        if (k >= 0 && k < 256) keep[k] = true;
    const float r2 = p_radius * p_radius;
    const int x0 = std::max(0, (int)std::floor(p_center.x - p_radius)), x1 = std::min(W_ - 1, (int)std::ceil(p_center.x + p_radius));
    const int y0 = std::max(0, (int)std::floor(p_center.y - p_radius)), y1 = std::min(H_ - 1, (int)std::ceil(p_center.y + p_radius));
    const int z0 = std::max(0, (int)std::floor(p_center.z - p_radius)), z1 = std::min(D_ - 1, (int)std::ceil(p_center.z + p_radius));
    if (x0 > x1 || y0 > y1 || z0 > z1) return 0;
    // one job per chunk on the pool (ADR 0029: an 8 m crater is 8.8 M cells at 6.25 cm); chunks write only their own
    // cells, marks are atomic
    std::vector<int> jobs;
    for (int cz = z0 / CS; cz <= z1 / CS; cz++)
        for (int cy = y0 / CS; cy <= y1 / CS; cy++)
            for (int cx = x0 / CS; cx <= x1 / CS; cx++) jobs.push_back(cx + CX_ * (cy + CY_ * cz));
    std::atomic<int> changed{0};
    pool_.run(jobs, [&](int job) {
        const int cx = job % CX_, cy = (job / CX_) % CY_, cz = job / (CX_ * CY_);
        const int ci = cidx(cx, cy, cz);
        if (loaded_[ci] != 1) return;   // still loading: a wall
        int lo[3] = {CS, CS, CS}, hi[3] = {-1, -1, -1};
        int n = 0;
        const int zs = std::max(z0, cz * CS), ze = std::min(z1, cz * CS + CS - 1);
        const int ys = std::max(y0, cy * CS), ye = std::min(y1, cy * CS + CS - 1);
        const int xs = std::max(x0, cx * CS), xe = std::min(x1, cx * CS + CS - 1);
        for (int z = zs; z <= ze; z++) {
            const float dz = z + 0.5f - p_center.z;
            for (int y = ys; y <= ye; y++) {
                const float dy = y + 0.5f - p_center.y;
                const float rem = r2 - dz * dz - dy * dy;
                if (rem < 0.0f) continue;
                for (int x = xs; x <= xe; x++) {
                    const float dx = x + 0.5f - p_center.x;
                    if (dx * dx > rem) continue;
                    const int i = idx(x, y, z);
                    const uint8_t cur = cells_[i].mat;
                    if (cur == m || keep[cur]) continue;
                    place(i, m);
                    moved_[i] = mark_;   // fresh change: static ground next to the crater may loosen (ADR 0013)
                    n++;
                    const int l[3] = {x & (CS - 1), y & (CS - 1), z & (CS - 1)};
                    for (int a = 0; a < 3; a++) { lo[a] = std::min(lo[a], l[a]); hi[a] = std::max(hi[a], l[a]); }
                }
            }
        }
        if (hi[0] < 0) return;
        changed.fetch_add(n, std::memory_order_relaxed);
        dirty_[ci].store(1, std::memory_order_relaxed);
        merge_dbox(ci, box_union(box_point(lo[0], lo[1], lo[2]), box_point(hi[0], hi[1], hi[2])));
        for (int oz = -1; oz <= 1; oz++)   // the chunk and its neighbours settle around the hole
            for (int oy = -1; oy <= 1; oy++)
                for (int ox = -1; ox <= 1; ox++) {
                    const int nx = cx + ox, ny = cy + oy, nz = cz + oz;
                    if (nx >= 0 && ny >= 0 && nz >= 0 && nx < CX_ && ny < CY_ && nz < CZ_) wake_full(cidx(nx, ny, nz));
                }
    });
    return changed.load();
}

// Ignites every burnable cell in [p_min, p_max) (fire tests, fire spells). Returns the count.
// One job per chunk on the pool, marks merged once per chunk (a 15 m fire test is 12 M cells at 6.25 cm).
int SimWorld::ignite_box(Vector3i p_min, Vector3i p_max) {
    wait_idle();
    const int x0 = std::max(0, p_min.x), x1 = std::min(W_, p_max.x) - 1;
    const int y0 = std::max(0, p_min.y), y1 = std::min(H_, p_max.y) - 1;
    const int z0 = std::max(0, p_min.z), z1 = std::min(D_, p_max.z) - 1;
    if (x0 > x1 || y0 > y1 || z0 > z1) return 0;
    std::vector<int> jobs;
    for (int cz = z0 / CS; cz <= z1 / CS; cz++)
        for (int cy = y0 / CS; cy <= y1 / CS; cy++)
            for (int cx = x0 / CS; cx <= x1 / CS; cx++) jobs.push_back(cx + CX_ * (cy + CY_ * cz));
    std::atomic<int> lit{0};
    pool_.run(jobs, [&](int job) {
        const int cx = job % CX_, cy = (job / CX_) % CY_, cz = job / (CX_ * CY_);
        const int ci = cidx(cx, cy, cz);
        if (loaded_[ci] != 1) return;   // still loading
        int lo[3] = {CS, CS, CS}, hi[3] = {-1, -1, -1};
        int n = 0;
        for (int z = std::max(z0, cz * CS); z <= std::min(z1, cz * CS + CS - 1); z++)
            for (int y = std::max(y0, cy * CS); y <= std::min(y1, cy * CS + CS - 1); y++)
                for (int x = std::max(x0, cx * CS); x <= std::min(x1, cx * CS + CS - 1); x++) {
                    const int i = idx(x, y, z);
                    const MaterialDef &md = mats_[cells_[i].mat];
                    if (!md.burnable || (cells_[i].flags & FLAG_BURNING)) continue;
                    cells_[i].flags |= FLAG_BURNING;
                    cells_[i].aux = md.fire_hp;
                    n++;
                    const int l[3] = {x & (CS - 1), y & (CS - 1), z & (CS - 1)};
                    for (int a = 0; a < 3; a++) { lo[a] = std::min(lo[a], l[a]); hi[a] = std::max(hi[a], l[a]); }
                }
        if (hi[0] < 0) return;
        lit.fetch_add(n, std::memory_order_relaxed);
        dirty_[ci].store(1, std::memory_order_relaxed);
        merge_dbox(ci, box_union(box_point(lo[0], lo[1], lo[2]), box_point(hi[0], hi[1], hi[2])));
        for (int oz = -1; oz <= 1; oz++)
            for (int oy = -1; oy <= 1; oy++)
                for (int ox = -1; ox <= 1; ox++) {
                    const int nx = cx + ox, ny = cy + oy, nz = cz + oz;
                    if (nx >= 0 && ny >= 0 && nz >= 0 && nx < CX_ && ny < CY_ && nz < CZ_) wake_full(cidx(nx, ny, nz));
                }
    });
    return lit.load();
}

// ---------- marks ----------

inline void SimWorld::mark_cell(int x, int y, int z, Ctx &c, bool changed) {
    int ox = x / CS - c.cx, oy = y / CS - c.cy, oz = z / CS - c.cz;
    int sb = mark_bit(ox, oy, oz);
    uint32_t self_bit = 1u << sb;
    c.mk.wake |= self_bit;
    if (changed) c.mk.dirty |= self_bit;
    int lx = x & (CS - 1), ly = y & (CS - 1), lz = z & (CS - 1);
    c.mk.box[sb] = box_union(c.mk.box[sb], box_point(lx, ly, lz));
    if (changed) c.mk.dbox[sb] = box_union(c.mk.dbox[sb], box_point(lx, ly, lz));
    if (lx == 0 && ox > -1) { int b = mark_bit(ox - 1, oy, oz); c.mk.wake |= 1u << b; c.mk.box[b] = box_union(c.mk.box[b], box_point(CS - 1, ly, lz)); }
    if (lx == CS - 1 && ox < 1) { int b = mark_bit(ox + 1, oy, oz); c.mk.wake |= 1u << b; c.mk.box[b] = box_union(c.mk.box[b], box_point(0, ly, lz)); }
    if (ly == 0 && oy > -1) { int b = mark_bit(ox, oy - 1, oz); c.mk.wake |= 1u << b; c.mk.box[b] = box_union(c.mk.box[b], box_point(lx, CS - 1, lz)); }
    if (ly == CS - 1 && oy < 1) { int b = mark_bit(ox, oy + 1, oz); c.mk.wake |= 1u << b; c.mk.box[b] = box_union(c.mk.box[b], box_point(lx, 0, lz)); }
    if (lz == 0 && oz > -1) { int b = mark_bit(ox, oy, oz - 1); c.mk.wake |= 1u << b; c.mk.box[b] = box_union(c.mk.box[b], box_point(lx, ly, CS - 1)); }
    if (lz == CS - 1 && oz < 1) { int b = mark_bit(ox, oy, oz + 1); c.mk.wake |= 1u << b; c.mk.box[b] = box_union(c.mk.box[b], box_point(lx, ly, 0)); }
}

void SimWorld::flush_marks(const Ctx &c) {
    if (!c.mk.wake) return;
    for (int oz = -1; oz <= 1; oz++)
        for (int oy = -1; oy <= 1; oy++)
            for (int ox = -1; ox <= 1; ox++) {
                uint32_t bit = 1u << mark_bit(ox, oy, oz);
                if (!(c.mk.wake & bit)) continue;
                int nx = c.cx + ox, ny = c.cy + oy, nz = c.cz + oz;
                if (nx < 0 || ny < 0 || nz < 0 || nx >= CX_ || ny >= CY_ || nz >= CZ_) continue;
                int ci = cidx(nx, ny, nz);
                active_next_[ci].store(1, std::memory_order_relaxed);
                merge_box(ci, c.mk.box[mark_bit(ox, oy, oz)] ? c.mk.box[mark_bit(ox, oy, oz)] : BOX_FULL);
                if (c.mk.dirty & bit) {
                    dirty_[ci].store(1, std::memory_order_relaxed);
                    uint32_t db = c.mk.dbox[mark_bit(ox, oy, oz)];
                    merge_dbox(ci, db ? db : BOX_FULL);
                }
            }
}

void SimWorld::merge_box(int ci, uint32_t b) {
    std::atomic<uint32_t> &a = box_next_[ci];
    uint32_t cur = a.load(std::memory_order_relaxed);
    for (;;) {
        uint32_t u = box_union(cur, b);
        if (u == cur || a.compare_exchange_weak(cur, u, std::memory_order_relaxed)) return;
    }
}

void SimWorld::merge_dbox(int ci, uint32_t b) {
    std::atomic<uint32_t> &a = dbox_[ci];
    uint32_t cur = a.load(std::memory_order_relaxed);
    for (;;) {
        uint32_t u = box_union(cur, b);
        if (u == cur || a.compare_exchange_weak(cur, u, std::memory_order_relaxed)) return;
    }
}

void SimWorld::wake_full(int ci) {
    active_next_[ci].store(1, std::memory_order_relaxed);
    box_next_[ci].store(BOX_FULL, std::memory_order_relaxed);
}

void SimWorld::set_gravity(float p_cells_per_tick2) {
    gravity16_ = (uint16_t)std::clamp((int)std::lround(p_cells_per_tick2 * 16.0), 0, 255);
}

void SimWorld::set_dirty_boxes(bool p_on) { dirty_boxes_ = p_on; }

// ---------- movement ----------

// Noita: a liquid/powder can displace a cell that is a liquid or gas of lower density.
inline bool SimWorld::can_displace(uint8_t mover, uint8_t target) const {
    return displace_lut_[mover * MAX_MATERIALS + target] != 0;
}

inline bool SimWorld::try_move(int sx, int sy, int sz, int tx, int ty, int tz, Ctx &c) {
    if (!is_open(tx, ty, tz)) return false;
    int ti = idx(tx, ty, tz);
    int si = idx(sx, sy, sz);
    if (moved_[ti] == mark_) return false;
    if (!can_displace(cells_[si].mat, cells_[ti].mat)) {
        const MaterialDef &sm = mats_[cells_[si].mat];
        const MaterialDef &tm = mats_[cells_[ti].mat];
        const uint8_t ct = sm.cell_type;
        if ((ct == CELL_LIQUID || ct == CELL_POWDER) && tm.liquid_static && tm.cell_type == CELL_LIQUID && tm.density < sm.density) {
            // ADR 0049: sand (or a heavier liquid) sinks through still liquid. Inside the body (still liquid beside
            // it) the still cell trades places, so the body stays whole; from above it, the body keeps its level
            // and the cell is gone.
            bool inside = false;
            for (int k = 0; k < 4 && !inside; k++)
                inside = in_world(sx + DX4[k], sy, sz + DZ4[k]) && cells_[idx(sx + DX4[k], sy, sz + DZ4[k])].mat == cells_[ti].mat;
            if (!inside) place(ti, 0);
        } else {
            // a plant cell in the way of a liquid or powder is washed away / buried (micro simulation only)
            if (!washable_[cells_[ti].mat] || (ct != CELL_LIQUID && ct != CELL_POWDER)) return false;
            place(ti, 0);
        }
    }
    std::swap(cells_[ti], cells_[si]);
    moved_[ti] = mark_;
    // cascade: the vacated air cell may be entered again this tick ("previous tick" still reads as fresh for loosening)
    moved_[si] = (cascade_ && cells_[si].mat == 0) ? (uint16_t)(stamp_ - 1) : mark_;
    mark_cell(sx, sy, sz, c, true);
    mark_cell(tx, ty, tz, c, true);
    return true;
}

// Static ground next to a fresh change gets a chance to turn loose (see set_materials).
void SimWorld::try_loosen(int x, int y, int z, int i, Ctx &c) {
    const MaterialDef &md = mats_[cells_[i].mat];
    if (md.loosen == 0 && md.erode == 0) return;
    const uint8_t self = cells_[i].mat;
    const uint16_t prev = (uint16_t)(stamp_ - 1);
    const bool shaken = (cells_[i].flags & FLAG_SHAKEN) != 0;
    // this cell was just placed or changed (an edit stamps it): a still powder set down over nothing
    // falls like Minecraft's sand (ADR 0073), while one set down on ground keeps its shape
    const bool fresh_self = moved_[i] == stamp_ || moved_[i] == prev;
    auto open = [&](int nx, int ny, int nz) -> bool {
        return is_open(nx, ny, nz) && can_displace(self, cells_[idx(nx, ny, nz)].mat);
    };
    // an opening counts if it is fresh (changed this or last tick) or this cell was shaken before
    auto trigger = [&](int nx, int ny, int nz) -> bool {
        if (!open(nx, ny, nz)) return false;
        if (shaken) return true;
        uint16_t s = moved_[idx(nx, ny, nz)];
        return s == stamp_ || s == prev;
    };
    uint32_t r = rnd(c);
    bool hit = false, go = false;
    if (md.loosen) {
        bool t = trigger(x, y - 1, z) || (fresh_self && md.cell_type == CELL_POWDER && open(x, y - 1, z));
        if (!t && md.cell_type == CELL_LIQUID)   // static water: any opening beside it
            for (int k = 0; k < 4 && !t; k++) t = trigger(x + DX4[k], y, z + DZ4[k]);
        if (t) { hit = true; go = (r & 0xFFFF) < md.loosen; }
    }
    if (!go && md.erode) {
        int k = (r >> 8) & 3;
        int sx = x + DX4[k], sz = z + DZ4[k];
        if (open(sx, y - 1, sz) && (trigger(sx, y, sz))) { hit = true; go = ((r >> 16) & 0xFFFF) < md.erode; }
    }
    if (!hit) return;
    if (!go) {
        // stay awake and keep rolling while the opening is there
        cells_[i].flags |= FLAG_SHAKEN;
        mark_cell(x, y, z, c, false);
        return;
    }
    cells_[i].flags = (uint8_t)((cells_[i].flags & ~FLAG_SHAKEN) | FLAG_LOOSE);
    moved_[i] = mark_;
    mark_cell(x, y, z, c, false);
}

bool SimWorld::still_liquid(int x, int y, int z, int slot) const {
    if (!is_open(x, y, z)) return false;
    const MaterialDef &t = mats_[cells_[idx(x, y, z)].mat];
    return t.liquid_static && t.cell_type == CELL_LIQUID && t.fluid_slot == slot;
}

// Still liquid (generated rivers, lakes, the sea, pools, waterfall sheets: liquid_static) sleeps until something
// opens beside or under it: air that changed this or the last two ticks (stamp 0 means never, so neither a reload
// nor the air the generator left wakes it). Then the cell becomes its moving kind, fullness kept, and is plain
// mass-conserving Noita liquid from then on (ADR 0068: all water is finite). The cell it leaves is the next fresh
// opening, so a hole beside a river drains the river into it cell by cell, and never more than the river holds.
// Air beside it over the body's own still cells (each 12.5 cm step of a river surface) is no opening, or every
// river would run down its own steps.
void SimWorld::reservoir(int x, int y, int z, int i, Ctx &c) {
    const uint8_t self = cells_[i].mat;
    const MaterialDef &md = mats_[self];
    if (md.fluid_slot < 0) return;
    const uint8_t flow = (uint8_t)slot_material_[md.fluid_slot];
    const uint16_t prev = (uint16_t)(stamp_ - 1), prev2 = (uint16_t)(stamp_ - 2);
    auto fresh_air = [&](int nx, int ny, int nz, bool side) -> bool {
        if (!is_open(nx, ny, nz)) return false;
        const int ni = idx(nx, ny, nz);
        if (!open_air(cells_[ni].mat)) return false;
        if (side && ny > 0 && still_liquid(nx, ny - 1, nz, md.fluid_slot)) return false;   // a river's own step
        const uint16_t m = moved_[ni];
        return m != 0 && (m == stamp_ || m == prev || m == prev2);
    };
    bool open = y > 0 && fresh_air(x, y - 1, z, false);
    for (int d = 0; d < 4 && !open; d++) open = fresh_air(x + DX4[d], y, z + DZ4[d], true);
    if (!open || flow == 0) {
        cells_[i].flags &= ~FLAG_SHAKEN;
        // asleep, unless something it reacts with touches it (water on a sludge pool)
        if (any_reactions_ && reactive_[self])
            for (int d = 0; d < 6; d++) {
                const int nx = x + NX6[d], ny = y + NY6[d], nz = z + NZ6[d];
                if (!is_open(nx, ny, nz)) continue;
                const uint8_t b = cells_[idx(nx, ny, nz)].mat;
                if (react_lut_[self * MAX_MATERIALS + b] >= 0 || react_lut_[b * MAX_MATERIALS + self] >= 0) {
                    cells_[i].flags |= FLAG_SHAKEN;
                    mark_cell(x, y, z, c, false);
                    break;
                }
            }
        return;
    }
    cells_[i].mat = flow;
    cells_[i].flags = 0;
    cells_[i].aux = 0;
    cells_[i].pad &= 15u;   // fullness kept, momentum cleared
    moved_[i] = mark_;
    still_woken_.fetch_add(1, std::memory_order_relaxed);
    mark_cell(x, y, z, c, true);
    c.moved++;
}

// v5: falling powders and liquids keep a velocity in aux (1/16 cell per tick) and move up to
// MAX_DISP cells down per tick (the parity scheme allows < CS/2); landing clears it.
// Returns true when the cell fell at least one cell.
bool SimWorld::fall(int x, int y, int z, int i, Ctx &c) {
    const bool fast = gravity16_ > 0 && !(cells_[i].flags & FLAG_BURNING) && mats_[cells_[i].mat].lifetime == 0;
    // ADR 0049: a cell following one that fell this tick (cascade: the air it left reads stamp - 1 and holds its
    // speed) takes that speed, so a stream stays one stream instead of thinning into scattered cells
    int lead = 0;
    if (fast && cascade_ && is_open(x, y - 1, z)) {
        const int bi = idx(x, y - 1, z);
        if (cells_[bi].mat == 0 && moved_[bi] == (uint16_t)(stamp_ - 1)) lead = cells_[bi].aux;
    }
    if (!try_move(x, y, z, x, y - 1, z, c)) {
        if (fast) cells_[i].aux = 0;
        return false;
    }
    int ny = y - 1;
    int ni = idx(x, ny, z);
    if (fast) {
        int v = std::max(lead, std::min(255, (int)cells_[ni].aux + gravity16_));
        int steps = std::min(MAX_DISP, std::max(1, v >> 4));
        for (int s = 1; s < steps; s++) {
            if (!try_move(x, ny, z, x, ny - 1, z, c)) { v = 0; break; }
            ny--;
            ni = idx(x, ny, z);
        }
        cells_[ni].aux = (uint8_t)v;
        if (cells_[i].mat == 0) cells_[i].aux = (uint8_t)v;   // the air it left: the speed for the cell above
    }
    if (fullness_) {   // falling liquid carries momentum into its landing (high nibble of pad)
        const MaterialDef &fm = mats_[cells_[ni].mat];
        if (fm.cell_type == CELL_LIQUID) cells_[ni].pad = (uint8_t)((cells_[ni].pad & 15u) | (15u << 4));
    }
    return true;
}

void SimWorld::update_powder(int x, int y, int z, int i, Ctx &c) {
    const MaterialDef &m = mats_[cells_[i].mat];
    uint32_t r = rnd(c);
    if ((r & 0xFF) > m.gravity_prob) return;
    if (fall(x, y, z, i, c)) { c.moved++; return; }
    int d = (r >> 8) & 3;
    for (int k = 0; k < 4; k++) {
        int dd = (d + k) & 3;
        if (try_move(x, y, z, x + DX4[dd], y - 1, z + DZ4[dd], c)) { c.moved++; return; }
    }
}

void SimWorld::update_liquid(int x, int y, int z, int i, Ctx &c) {
    if (fullness_) { update_liquid_fullness(x, y, z, i, c); return; }
    const uint8_t self = cells_[i].mat;
    const MaterialDef &m = mats_[self];
    uint32_t r = rnd(c);
    if (fall(x, y, z, i, c)) { c.moved++; return; }
    if ((r & 0xFF) > m.gravity_prob) return;
    int d = (r >> 8) & 3;
    for (int k = 0; k < 4; k++) {
        int dd = (d + k) & 3;
        if (try_move(x, y, z, x + DX4[dd], y - 1, z + DZ4[dd], c)) { c.moved++; return; }
    }
    // Lateral dispersion: slide up to disp cells through displaceable cells, stop at a ledge.
    const int DISP = m.disp;
    if (DISP == 0) return;
    for (int k = 0; k < 4; k++) {
        int dd = (d + k) & 3;
        int best_x = x, best_z = z, coh_x = x, coh_z = z;
        bool ledge = false;
        for (int s = 1; s <= DISP; s++) {
            int px = x + DX4[dd] * s, pz = z + DZ4[dd] * s;
            if (!is_open(px, y, pz)) break;
            int pi = idx(px, y, pz);
            if (!can_displace(self, cells_[pi].mat) || moved_[pi] == mark_) break;
            best_x = px; best_z = pz;
            uint8_t below = y > 0 ? cells_[idx(px, y - 1, pz)].mat : self;
            if (y > 0 && can_displace(self, below)) { ledge = true; break; }
            bool touching = below == self;
            for (int n = 0; n < 4 && !touching; n++) {
                int qx = px + DX4[n], qz = pz + DZ4[n];
                touching = in_world(qx, y, qz) && cells_[idx(qx, y, qz)].mat == self;
            }
            if (touching) { coh_x = px; coh_z = pz; }
        }
        if (!ledge && (coh_x != x || coh_z != z)) { best_x = coh_x; best_z = coh_z; }
        if (best_x == x && best_z == z) continue;
        uint8_t under = y > 0 ? cells_[idx(x, y - 1, z)].mat : self;
        if (!ledge && under == self && ((r >> 12) & 7) != 0) continue; // calm pool
        if (try_move(x, y, z, best_x, y, best_z, c)) { c.moved++; return; }
    }
}

void SimWorld::set_liquid_fullness(bool p_on) { fullness_ = p_on; }
void SimWorld::set_cascade(bool p_on) { cascade_ = p_on; }

int SimWorld::count_liquid_eighths(int p_material) const {
    wait_idle();
    int n = 0;
    for (const Cell &c : cells_)
        if (c.mat == (uint8_t)p_material) n += (c.pad & 15) ? (c.pad & 15) : 8;
    return n;
}

void SimWorld::update_liquid_fullness(int x, int y, int z, int i, Ctx &c) {
    const uint8_t self = cells_[i].mat;
    const MaterialDef &m = mats_[self];
    uint32_t r = rnd(c);
    // pad: low nibble = fullness in eighths (0 = full), high nibble = momentum (ticks of fast sliding left)
    auto full = [&](int k) -> int { int f = cells_[k].pad & 15; return f ? f : 8; };
    auto set_full = [&](int k, int f) { cells_[k].pad = (uint8_t)((cells_[k].pad & 0xF0u) | (uint32_t)(f >= 8 ? 0 : f)); };
    auto momentum = [&](int k) -> int { return cells_[k].pad >> 4; };
    auto set_momentum = [&](int k, int mo) { cells_[k].pad = (uint8_t)((cells_[k].pad & 15u) | ((uint32_t)std::clamp(mo, 0, 15) << 4)); };
    // air over this liquid's still body is no place to flow sideways from anywhere else (water in a trench beside a
    // river does not climb out onto it; ADR 0049), unless this cell already rests on that body: what is poured on a
    // lake spreads flat over it instead of standing as a heap (ADR 0068)
    const bool on_still = y > 0 && m.fluid_slot >= 0 && still_liquid(x, y - 1, z, m.fluid_slot);
    auto over_still = [&](int px, int pz) -> bool { return !on_still && y > 0 && m.fluid_slot >= 0 && still_liquid(px, y - 1, pz, m.fluid_slot); };
    // 1. fall into anything it displaces, as a whole cell (keeps velocity). Gravity does not wait for the
    // viscosity roll: a thick liquid (lava) pours as a stream, it only spreads slowly (ADR 0049)
    if (substep_ > 0) {
        // a substep spreads and pours; falling stays with the main pass (gravity once a tick, like everything else).
        // A cell that would fall, or that stands on a cell that moved this tick (a falling column, a stream), waits:
        // spreading it sideways now would scatter what is on its way down
        if (y > 0 && is_open(x, y - 1, z)) {
            const int bi = idx(x, y - 1, z);
            if (can_displace(self, cells_[bi].mat)) return;
            const uint16_t mb = moved_[bi];
            if (mb == stamp_ || ((mb & 0x8000u) && ((mb >> 2) & 0x1FFFu) == stamp_)) return;
        }
    } else if (fall(x, y, z, i, c)) { c.moved++; return; }
    int f = full(i);
    // ADR 0069 B3: a one-eighth film on a floor that is not its liquid, with nothing deeper of its kind beside it
    // (a lone speck or a spill thinned to a sheet; a pool's rim next to deeper water stays), dries up (or crusts) after about
    // film_life ticks: a spill leaves no permanent wet speckle. Mass is lost on purpose. Before the viscosity roll,
    // so a slow liquid's film keeps its clock; the film stays awake while it waits
    if (f == 1 && substep_ == 0 && m.film_life && y > 0 && is_open(x, y - 1, z) && cells_[idx(x, y - 1, z)].mat != self) {
        bool alone = true;
        for (int k = 0; k < 4 && alone; k++) {
            const int nx = x + DX4[k], nz = z + DZ4[k];
            if (is_open(nx, y, nz) && cells_[idx(nx, y, nz)].mat == self && full(idx(nx, y, nz)) > 1) alone = false;
        }
        if (alone) {
            if (rnd(c) % m.film_life == 0) {
                place(i, m.film_to);
                mark_cell(x, y, z, c, true);
                c.moved++;
                return;
            }
            mark_cell(x, y, z, c, false);
        }
    }
    if ((r & 0xFF) > m.gravity_prob) return;
    // 2. pour into a partly full cell of the same liquid below
    if (is_open(x, y - 1, z)) {
        int bi = idx(x, y - 1, z);
        if (cells_[bi].mat == self && moved_[bi] != mark_) {
            int fb = full(bi);
            if (fb < 8) {
                int t = std::min(8 - fb, f);
                set_full(bi, fb + t);
                moved_[bi] = mark_;
                moved_[i] = mark_;
                if (t >= f) { place(i, 0); mark_cell(x, y, z, c, true); }
                else { set_full(i, f - t); mark_cell(x, y, z, c, false); }
                mark_cell(x, y - 1, z, c, false);
                c.moved++;
                return;
            }
        }
    }
    // 3. run down a slope as a whole cell (the main pass only: a substep spreads, it does not descend, or a column
    // standing on its own liquid would scatter sideways while it falls)
    int d = (r >> 8) & 3;
    if (substep_ == 0) {
        for (int k = 0; k < 4; k++) {
            int dd = (d + k) & 3;
            if (try_move(x, y, z, x + DX4[dd], y - 1, z + DZ4[dd], c)) { c.moved++; return; }
        }
    }
    if (f < 2) {
        // a one-eighth droplet on top of full cells wanders until it can pour into a lower cell;
        // on the floor (nothing of its own kind below) it stays: floods settle as a thin film
        if (!is_open(x, y - 1, z) || cells_[idx(x, y - 1, z)].mat != self) return;
        int dd = (r >> 16) & 3;
        if (over_still(x + DX4[dd], z + DZ4[dd])) return;
        if (try_move(x, y, z, x + DX4[dd], y, z + DZ4[dd], c)) c.moved++;
        return;
    }
    // 4a. a full cell under more of its liquid (bulk, pressure) flows like a P5 liquid: slide up to
    // disp cells sideways, so floods move fast; the surface layer splits and evens out instead (4b)
    const bool pressed = is_open(x, y + 1, z) && cells_[idx(x, y + 1, z)].mat == self;
    const int mo = momentum(i);
    if (f == 8 && (pressed || mo > 0) && m.disp > 0) {
        for (int k = 0; k < 4; k++) {
            int dd = (d + k) & 3;
            int best_x = x, best_z = z;
            for (int s = 1; s <= m.disp; s++) {
                int px = x + DX4[dd] * s, pz = z + DZ4[dd] * s;
                if (!is_open(px, y, pz)) break;
                int pi = idx(px, y, pz);
                if (!open_air(cells_[pi].mat) || moved_[pi] == mark_ || over_still(px, pz)) break;
                best_x = px; best_z = pz;
                if (y > 0 && is_open(px, y - 1, pz) && open_air(cells_[idx(px, y - 1, pz)].mat)) break;   // ledge: drop there
            }
            if (best_x == x && best_z == z) continue;
            uint8_t under = y > 0 ? cells_[idx(x, y - 1, z)].mat : self;
            // calm pool: mostly stay. With cascade (micro) a pressed cell next to air is a water wall and goes.
            if (!cascade_ && under == self && ((r >> 12) & 7) != 0) continue;
            if (try_move(x, y, z, best_x, y, best_z, c)) {
                // pushed out of a column: full momentum; otherwise it spends one tick of it
                set_momentum(idx(best_x, y, best_z), pressed ? 15 : mo - 1);
                c.moved++;
                return;
            }
        }
        if (mo > 0) { set_momentum(i, mo - 1); mark_cell(x, y, z, c, false); }
    }
    // 4b. spread sideways: split half into an empty neighbour, or even out with a lower one
    for (int k = 0; k < 4; k++) {
        int dd = (d + k) & 3;
        int nx = x + DX4[dd], nz = z + DZ4[dd];
        if (!is_open(nx, y, nz)) continue;
        int ni = idx(nx, y, nz);
        if (moved_[ni] == mark_) continue;
        const uint8_t t = cells_[ni].mat;
        if (open_air(t)) {   // empty, or a plant the water washes away
            if (over_still(nx, nz)) continue;
            int g = f / 2;
            place(ni, self);
            set_full(ni, g);
            set_full(i, f - g);
            moved_[ni] = mark_;
            moved_[i] = mark_;
            mark_cell(nx, y, nz, c, true);
            mark_cell(x, y, z, c, false);
            c.moved++;
            return;
        }
        if (t == self) {
            int fn = full(ni);
            if (fn <= f - 2) {   // even out: half the difference (ADR 0069 B), so a flood ends flat in a few ticks
                const int t = (f - fn) / 2;
                set_full(ni, fn + t);
                set_full(i, f - t);
                moved_[ni] = mark_;
                moved_[i] = mark_;
                mark_cell(nx, y, nz, c, false);
                mark_cell(x, y, z, c, false);
                c.moved++;
                return;
            }
        }
    }
}

void SimWorld::update_gas(int x, int y, int z, int i, Ctx &c) {
    const MaterialDef &m = mats_[cells_[i].mat];
    uint32_t r = rnd(c);
    if ((r & 0xFF) < m.gas_up) {
        if (try_move(x, y, z, x, y + 1, z, c)) { c.moved++; return; }
    }
    if (((r >> 8) & 0xFF) < m.gas_horizontal) {
        int dd = (r >> 16) & 3;
        if (try_move(x, y, z, x + DX4[dd], y, z + DZ4[dd], c)) { c.moved++; return; }
        // blocked above and sideways: creep along the ceiling diagonally
        if (try_move(x, y, z, x + DX4[dd], y + 1, z + DZ4[dd], c)) { c.moved++; return; }
    }
    if (((r >> 24) & 0xFF) < m.gas_down) {
        if (try_move(x, y, z, x, y - 1, z, c)) { c.moved++; return; }
    }
}

// A fire cell: behaves like a short-lived gas and tries to ignite one random neighbour.
void SimWorld::update_fire(int x, int y, int z, int i, Ctx &c) {
    const MaterialDef &m = mats_[cells_[i].mat];
    uint32_t r = rnd(c);
    int n = (r >> 4) % 6;
    int nx = x + NX6[n], ny = y + NY6[n], nz = z + NZ6[n];
    if (is_open(nx, ny, nz)) {
        int ni = idx(nx, ny, nz);
        const MaterialDef &t = mats_[cells_[ni].mat];
        if (t.burnable && !(cells_[ni].flags & FLAG_BURNING) && m.temperature_of_fire >= t.autoignition_temperature) {
            cells_[ni].flags |= FLAG_BURNING;
            cells_[ni].aux = t.fire_hp;
            mark_cell(nx, ny, nz, c, true);
            c.reacted++;
        }
    }
    update_gas(x, y, z, i, c);
}

// A burning cell of any type: loses fire_hp, spawns flames/smoke into air, dies into convert_to.
void SimWorld::update_burning(int x, int y, int z, int i, Ctx &c) {
    const uint8_t self = cells_[i].mat;
    const MaterialDef &m = mats_[self];
    uint32_t r = rnd(c);
    // Extinguish if touching a liquid that is not burnable (water).
    for (int n = 0; n < 6; n++) {
        int nx = x + NX6[n], ny = y + NY6[n], nz = z + NZ6[n];
        if (!in_world(nx, ny, nz)) continue;
        const MaterialDef &t = mats_[cells_[idx(nx, ny, nz)].mat];
        if (t.cell_type == CELL_LIQUID && !t.burnable) {
            cells_[i].flags &= ~FLAG_BURNING;
            mark_cell(x, y, z, c, true);
            return;
        }
    }
    // Spread to a random burnable neighbour (Noita: burning pixels ignite neighbours directly).
    int n = (r >> 4) % 6;
    int nx = x + NX6[n], ny = y + NY6[n], nz = z + NZ6[n];
    if (is_open(nx, ny, nz)) {
        int ni = idx(nx, ny, nz);
        const MaterialDef &t = mats_[cells_[ni].mat];
        if (t.burnable && !(cells_[ni].flags & FLAG_BURNING) && m.temperature_of_fire >= t.autoignition_temperature
                && (rnd(c) & 0xFFFF) < t.ignite_chance) {
            cells_[ni].flags |= FLAG_BURNING;
            cells_[ni].aux = t.fire_hp;
            mark_cell(nx, ny, nz, c, true);
            c.reacted++;
        }
        // Spawn flame or smoke into an air neighbour.
        if (cells_[ni].mat == 0 && moved_[ni] != mark_) {
            uint8_t spawn = 0;
            if (((r >> 8) & 0xFF) < m.generates_smoke && m.on_fire_smoke_material) spawn = m.on_fire_smoke_material;
            else if (((r >> 16) & 0xFF) < 160 && m.on_fire_flame_material) spawn = m.on_fire_flame_material;
            if (spawn) {
                place(ni, spawn);
                moved_[ni] = mark_;
                mark_cell(nx, ny, nz, c, true);
            }
        }
    }
    // Burn down.
    if (cells_[i].aux > 0) cells_[i].aux--;
    mark_cell(x, y, z, c, false);
    if (cells_[i].aux == 0) {
        place(i, m.on_fire_convert_to);
        mark_cell(x, y, z, c, true);
        c.reacted++;
    }
}

void SimWorld::try_react(int x, int y, int z, int i, Ctx &c) {
    const uint8_t a = cells_[i].mat;
    uint32_t r = rnd(c);
    int n = (r >> 4) % 6;
    int nx = x + NX6[n], ny = y + NY6[n], nz = z + NZ6[n];
    if (!is_open(nx, ny, nz)) return;
    int ni = idx(nx, ny, nz);
    const uint8_t b = cells_[ni].mat;
    int16_t ri = react_lut_[a * MAX_MATERIALS + b];
    bool swapped = false;
    if (ri < 0) { ri = react_lut_[b * MAX_MATERIALS + a]; swapped = true; }
    if (ri < 0) return;
    const Reaction &rx = reactions_[ri];
    if ((r >> 8 & 0xFFFF) >= rx.probability) return;
    if (moved_[ni] == mark_) return;
    uint8_t oa = swapped ? rx.out2 : rx.out1;
    uint8_t ob = swapped ? rx.out1 : rx.out2;
    place(i, oa);
    place(ni, ob);
    moved_[i] = mark_;
    moved_[ni] = mark_;
    mark_cell(x, y, z, c, true);
    mark_cell(nx, ny, nz, c, true);
    c.reacted++;
}

void SimWorld::chunk_of(int ci, int &cx, int &cy, int &cz) const {
    cx = ((ci % CX_) - ocx_) & cmx_;
    cy = (((ci / CX_) % CY_) - ocy_) & cmy_;
    cz = ((ci / (CX_ * CY_)) - ocz_) & cmz_;
}

void SimWorld::update_chunk(int ci) {
    if (loaded_[ci] != 1) return;   // waiting for data
    Ctx c;
    chunk_of(ci, c.cx, c.cy, c.cz);
    c.rng = (seed_ ^ ((uint32_t)ci * 2654435761u) ^ (tick_ * 40503u)) | 1u;
    int x0 = c.cx * CS, y0 = c.cy * CS, z0 = c.cz * CS;
    const bool alt = (tick_ & 1) != 0; // alternate x/z scan direction per tick to avoid bias
    // v5: only the cells woken last tick (their dirty box grown by one cell) can change now
    uint32_t bx = dirty_boxes_ ? box_now_[ci].load(std::memory_order_relaxed) : BOX_FULL;
    if (!(bx & BOX_VALID)) bx = BOX_FULL;
    const int bx0 = std::max(0, (int)(bx & 15u) - 1), by0 = std::max(0, (int)((bx >> 4) & 15u) - 1), bz0 = std::max(0, (int)((bx >> 8) & 15u) - 1);
    const int bx1 = std::min(CS - 1, (int)((bx >> 12) & 15u) + 1), by1 = std::min(CS - 1, (int)((bx >> 16) & 15u) + 1), bz1 = std::min(CS - 1, (int)((bx >> 20) & 15u) + 1);
    const int nzs = bz1 - bz0 + 1, nxs = bx1 - bx0 + 1;
    for (int y = y0 + by0; y <= y0 + by1; y++)
        for (int zz = 0; zz < nzs; zz++) {
            int z = z0 + (alt ? bz1 - zz : bz0 + zz);
            for (int xx = 0; xx < nxs; xx++) {
                int x = x0 + (alt ? bx1 - xx : bx0 + xx);
                int i = idx(x, y, z);
                uint8_t m = cells_[i].mat;
                if (m == 0 || inert_[m]) continue;
                if (moved_[i] == mark_) continue;
                const MaterialDef &md = mats_[m];
                // ADR 0069 B: a liquid substep moves only the liquids that still have substeps left this tick
                if (substep_ > 0 && (md.cell_type != CELL_LIQUID || md.liquid_static || md.speed <= substep_)) continue;
                // Lifetime (gases, short-lived liquids, fire): count down, vanish.
                if (substep_ == 0 && md.lifetime && !(cells_[i].flags & FLAG_BURNING)) {
                    if (cells_[i].aux > 0) { cells_[i].aux--; mark_cell(x, y, z, c, false); }
                    if (cells_[i].aux == 0) { place(i, 0); mark_cell(x, y, z, c, true); continue; }
                }
                if (substep_ == 0 && (cells_[i].flags & FLAG_BURNING)) update_burning(x, y, z, i, c);
                if (cells_[i].mat != m) continue; // burnt away
                switch (md.cell_type) {
                    case CELL_SOLID: break;
                    case CELL_LIQUID:
                    case CELL_POWDER:
                        if (md.liquid_static && md.cell_type == CELL_LIQUID) { reservoir(x, y, z, i, c); break; }
                        if (md.liquid_static && !(cells_[i].flags & FLAG_LOOSE)) { try_loosen(x, y, z, i, c); break; }
                        if (md.cell_type == CELL_POWDER) update_powder(x, y, z, i, c);
                        else update_liquid(x, y, z, i, c);
                        break;
                    case CELL_GAS: update_gas(x, y, z, i, c); break;
                    case CELL_FIRE: update_fire(x, y, z, i, c); break;
                }
                if (substep_ == 0 && any_reactions_ && reactive_[cells_[i].mat] && moved_[i] != mark_) try_react(x, y, z, i, c);
            }
        }
    flush_marks(c);
    if (c.moved) moved_count_.fetch_add(c.moved, std::memory_order_relaxed);
    if (c.reacted) react_count_.fetch_add(c.reacted, std::memory_order_relaxed);
}

void SimWorld::defer_chunk(int ci) {
    active_now_[ci].store(0, std::memory_order_relaxed);
    active_next_[ci].store(1, std::memory_order_relaxed);
    uint32_t b = box_now_[ci].load(std::memory_order_relaxed);
    merge_box(ci, (b & BOX_VALID) ? b : BOX_FULL);
    if (defer_age_[ci] < 255) defer_age_[ci]++;
}

// One tick. v6: chunks beyond the level-of-detail radius run every lod_divisor-th
// tick, and a cell budget keeps the nearest (and longest-waiting) chunks.
void SimWorld::tick_once(TickOut &o) {
    using clock = std::chrono::steady_clock;
    auto t0 = clock::now();
    size_t nc = (size_t)CX_ * CY_ * CZ_;
    tick_++;
    stamp_++;
    if (stamp_ >= 0x2000) { std::fill(moved_.begin(), moved_.end(), 0); stamp_ = 1; }   // keep clear of the substep marks
    mark_ = stamp_;
    for (size_t i = 0; i < nc; i++) {
        active_now_[i].store(active_next_[i].load(std::memory_order_relaxed), std::memory_order_relaxed);
        active_next_[i].store(0, std::memory_order_relaxed);
        box_now_[i].store(box_next_[i].load(std::memory_order_relaxed), std::memory_order_relaxed);
        box_next_[i].store(0, std::memory_order_relaxed);
    }
    uint64_t scanned = 0, deferred = 0;
    const bool lod = lod_radius_ >= 0 && lod_divisor_ > 1;
    if (lod || cell_budget_ > 0) {
        sched_.clear();
        const int64_t r2 = (int64_t)lod_radius_ * lod_radius_;
        for (size_t i = 0; i < nc; i++) {
            if (!active_now_[i].load(std::memory_order_relaxed)) continue;
            int ci = (int)i;
            int cx, cy, cz;
            chunk_of(ci, cx, cy, cz);
            int64_t dx = cx - focus_chunk_.x, dy = cy - focus_chunk_.y, dz = cz - focus_chunk_.z;
            int64_t d2 = dx * dx + dy * dy + dz * dz;
            if (lod && d2 > r2 && ((tick_ + (uint32_t)ci) % (uint32_t)lod_divisor_) != 0) {
                defer_chunk(ci);
                deferred++;
                continue;
            }
            sched_.push_back({d2 >> std::min<int>(defer_age_[ci], 30), ci});
        }
        if (cell_budget_ > 0) {
            std::sort(sched_.begin(), sched_.end());
            uint64_t used = 0;
            for (size_t k = 0; k < sched_.size(); k++) {
                int ci = sched_[k].second;
                int v = dirty_boxes_ ? box_volume(box_now_[ci].load(std::memory_order_relaxed)) : CS * CS * CS;
                if (k > 0 && used + (uint64_t)v > (uint64_t)cell_budget_) { defer_chunk(ci); deferred++; continue; }
                used += v;
                defer_age_[ci] = 0;
            }
            scanned = used;
        } else {
            for (auto &p : sched_) {
                defer_age_[p.second] = 0;
                scanned += dirty_boxes_ ? box_volume(box_now_[p.second].load(std::memory_order_relaxed)) : CS * CS * CS;
            }
        }
    } else {
        for (size_t i = 0; i < nc; i++)
            if (active_now_[i].load(std::memory_order_relaxed))
                scanned += dirty_boxes_ ? box_volume(box_now_[i].load(std::memory_order_relaxed)) : CS * CS * CS;
    }
    moved_count_.store(0);
    react_count_.store(0);
    std::vector<int> work;
    work.reserve(nc / 8 + 1);
    uint64_t active = 0;
    // ADR 0069 B: after the main pass, liquids with liquid_speed > 1 get extra passes over the same chunks (a new
    // move stamp each), so water spreads two or three times per tick while powders, lava, fire and gas keep the tick
    // The tick's stamp stays one per tick (freshness of openings and edits counts in ticks); a substep marks the
    // cells it moves with a value of its own (top bit set, unique per tick and substep) so a cell may move once per
    // pass. moved_ is cleared when the stamp reaches the marks' range (every 8192 ticks, as the wrap did).
    for (int sub = 0; sub < max_speed_; sub++) {
        substep_ = sub;
        mark_ = sub == 0 ? stamp_ : (uint16_t)(0x8000u | ((uint32_t)stamp_ << 2) | (uint32_t)sub);
        for (int pass = 0; pass < 8; pass++) {
            work.clear();
            for (int ci : pass_lists_[pass])
                if (active_now_[ci].load(std::memory_order_relaxed)) work.push_back(ci);
            if (sub == 0) active += work.size();
            pool_.run(work, [this](int ci) { update_chunk(ci); });
        }
    }
    substep_ = 0;
    mark_ = stamp_;
    o.ms = std::chrono::duration<double, std::milli>(clock::now() - t0).count();
    o.active = active;
    o.moved = moved_count_.load();
    o.react = react_count_.load();
    o.scanned = scanned;
    o.deferred = deferred;
}

static Dictionary tick_dict(int p_count, int p_threads, double total_ms, double max_ms, uint64_t active, uint64_t moved,
        uint64_t react, uint64_t scanned, uint64_t deferred, int cs3) {
    Dictionary d;
    int n = std::max(1, p_count);
    d["ticks"] = p_count;
    d["threads"] = p_threads;
    d["total_ms"] = total_ms;
    d["ms_per_tick"] = total_ms / n;
    d["max_tick_ms"] = max_ms;
    d["active_chunks_per_tick"] = (double)active / n;
    d["active_cells_per_tick"] = (double)active * cs3 / n;
    d["scanned_cells_per_tick"] = (double)scanned / n;
    d["deferred_chunks_per_tick"] = (double)deferred / n;
    d["moved_cells_per_tick"] = (double)moved / n;
    d["reactions_per_tick"] = (double)react / n;
    d["active_cells_per_ms"] = total_ms > 0 ? (double)active * cs3 / total_ms : 0.0;
    d["moved_cells_total"] = (int64_t)moved;
    return d;
}

Dictionary SimWorld::tick(int p_count) {
    wait_idle();
    double total = 0, mx = 0;
    uint64_t active = 0, moved = 0, react = 0, scanned = 0, deferred = 0;
    for (int n = 0; n < p_count; n++) {
        TickOut o;
        tick_once(o);
        total += o.ms;
        mx = std::max(mx, o.ms);
        active += o.active; moved += o.moved; react += o.react; scanned += o.scanned; deferred += o.deferred;
    }
    return tick_dict(p_count, threads_, total, mx, active, moved, react, scanned, deferred, CS * CS * CS);
}

// ---------- v6: level of detail, budget, background tick ----------

void SimWorld::set_focus(Vector3i p_cell) { focus_chunk_ = Vector3i(p_cell.x / CS, p_cell.y / CS, p_cell.z / CS); }
void SimWorld::set_lod(int p_radius_chunks, int p_divisor) { lod_radius_ = p_radius_chunks; lod_divisor_ = std::max(1, p_divisor); }
void SimWorld::set_cell_budget(int p_cells) { cell_budget_ = std::max(0, p_cells); }

bool SimWorld::start_tick() {
    if (busy_.load()) return false;
    std::unique_lock<std::mutex> lk(dm_);
    if (!driver_.joinable()) {
        drv_stop_ = false;
        driver_ = std::thread([this]() {
            for (;;) {
                {
                    std::unique_lock<std::mutex> l(dm_);
                    dcv_.wait(l, [&] { return drv_stop_ || drv_req_; });
                    if (drv_stop_) return;
                }
                if (pre_tick_) {   // P12 rigid pieces: they read and write cells between ticks
                    pre_phase_.store(true);
                    pre_tick_();
                    pre_phase_.store(false);
                }
                TickOut o;
                tick_once(o);
                {
                    std::lock_guard<std::mutex> l(dm_);
                    last_ = o;
                    drv_req_ = false;
                    busy_.store(false);
                }
                dcv_.notify_all();
            }
        });
    }
    busy_.store(true);
    drv_req_ = true;
    lk.unlock();
    dcv_.notify_all();
    return true;
}

bool SimWorld::is_tick_done() const { return !busy_.load(); }

void SimWorld::wait_readable() const {
    if (!busy_.load() || pre_phase_.load()) return;
    wait_idle();
}

void SimWorld::wait_idle() const {
    if (!busy_.load()) return;
    std::unique_lock<std::mutex> lk(dm_);
    dcv_.wait(lk, [&] { return !busy_.load(); });
}

Dictionary SimWorld::finish_tick() {
    wait_idle();
    TickOut o;
    {
        std::lock_guard<std::mutex> lk(dm_);
        o = last_;
    }
    return tick_dict(1, threads_, o.ms, o.ms, o.active, o.moved, o.react, o.scanned, o.deferred, CS * CS * CS);
}

void SimWorld::stop_driver() {
    wait_idle();
    {
        std::lock_guard<std::mutex> lk(dm_);
        drv_stop_ = true;
    }
    dcv_.notify_all();
    if (driver_.joinable()) driver_.join();
    drv_stop_ = false;
}

// ---------- render feed ----------

PackedByteArray SimWorld::get_chunk_bytes(Vector3i c, int p_depth_bytes) const {
    wait_idle();
    PackedByteArray out;
    const int n = CS * CS * CS;
    const int bpv = p_depth_bytes == 2 ? 2 : 1;
    out.resize(n * bpv);
    uint8_t *w = out.ptrw();
    if (bpv == 2) std::memset(w, 0, n * 2);
    int x0 = c.x * CS, y0 = c.y * CS, z0 = c.z * CS;
    for (int z = 0; z < CS; z++)
        for (int x = 0; x < CS; x++)
            for (int y = 0; y < CS; y++) {
                const Cell &cell = cells_[idx(x0 + x, y0 + y, z0 + z)];
                const int o = (y + CS * (x + CS * z)) * bpv;
                int v = cell.mat;
                const int e = cell.pad & 15;   // liquid fullness in eighths, 0 = full
                if (bpv == 2 && e && mats_[cell.mat].fluid_slot >= 0 && mats_[cell.mat].cell_type == CELL_LIQUID)
                    v = FLUID_LEVEL_BASE + mats_[cell.mat].fluid_slot * 8 + (e - 1);
                w[o] = (uint8_t)(v & 0xFF);
                if (bpv == 2) w[o + 1] = (uint8_t)(v >> 8);
            }
    return out;
}

void SimWorld::set_chunk_bytes(Vector3i c, const PackedByteArray &p_bytes, int p_depth_bytes) {
    wait_idle();
    if (c.x < 0 || c.y < 0 || c.z < 0 || c.x >= CX_ || c.y >= CY_ || c.z >= CZ_) return;
    const int n = CS * CS * CS;
    const int bpv = p_depth_bytes == 2 ? 2 : 1;
    if (p_bytes.size() < n * bpv) return;
    const uint8_t *r = p_bytes.ptr();
    int x0 = c.x * CS, y0 = c.y * CS, z0 = c.z * CS;
    for (int z = 0; z < CS; z++)
        for (int x = 0; x < CS; x++)
            for (int y = 0; y < CS; y++) {
                const int o = (y + CS * (x + CS * z)) * bpv;
                int v = bpv == 2 ? (r[o] | (r[o + 1] << 8)) : r[o];
                int e = 0;
                if (v >= FLUID_LEVEL_BASE) {   // a partly full liquid cell from the terrain
                    const int k = v - FLUID_LEVEL_BASE;
                    const int slot = k / 8;
                    e = k % 8 + 1;
                    v = slot < 32 ? slot_material_[slot] : 0;
                    if (e >= 8) e = 0;
                }
                if (v >= (int)mats_.size()) v = 0;
                const int ci = idx(x0 + x, y0 + y, z0 + z);
                place(ci, (uint8_t)v);
                moved_[ci] = 0;   // loaded, not changed: no stale stamp may read as a fresh opening
                if (e) cells_[ci].pad = (uint8_t)e;
            }
    int ci = cidx(c.x, c.y, c.z);
    wake_full(ci); // let it settle once
    loaded_[ci] = 1;
}

void SimWorld::set_column_bytes(Vector3i c, int ncy, const PackedByteArray &p_bytes, int p_depth_bytes) {
    wait_idle();
    if (c.x < 0 || c.z < 0 || c.x >= CX_ || c.z >= CZ_ || ncy <= 0) return;
    const int bpv = p_depth_bytes == 2 ? 2 : 1;
    const int H = ncy * CS;
    if (p_bytes.size() < CS * H * CS * bpv) return;
    const uint8_t *r = p_bytes.ptr();
    for (int k = 0; k < ncy; k++) {
        const int cy = c.y + k;
        if (cy < 0 || cy >= CY_) continue;
        const int x0 = c.x * CS, y0 = cy * CS, z0 = c.z * CS;
        for (int z = 0; z < CS; z++)
            for (int x = 0; x < CS; x++)
                for (int y = 0; y < CS; y++) {
                    const int o = ((k * CS + y) + H * (x + CS * z)) * bpv;
                    int v = bpv == 2 ? (r[o] | (r[o + 1] << 8)) : r[o];
                    int e = 0;
                    if (v >= FLUID_LEVEL_BASE) {
                        const int q = v - FLUID_LEVEL_BASE;
                        const int slot = q / 8;
                        e = q % 8 + 1;
                        v = slot < 32 ? slot_material_[slot] : 0;
                        if (e >= 8) e = 0;
                    }
                    if (v >= (int)mats_.size()) v = 0;
                    const int ci = idx(x0 + x, y0 + y, z0 + z);
                    place(ci, (uint8_t)v);
                    moved_[ci] = 0;
                    if (e) cells_[ci].pad = (uint8_t)e;
                }
        const int ci = cidx(c.x, cy, c.z);
        wake_full(ci);
        loaded_[ci] = 1;
    }
}

// Slide the simulation window by whole chunks. Cells that stay inside move in memory;
// vacated chunks are cleared to air and returned so the caller loads them from the world.
//
// In place: cells are one linear array (idx = x + W*(y + H*z)), so new[i] = old[i + off]
// for a constant offset is a single overlapping memmove. Cells whose source lies outside
// the window (the vacated chunks, plus x rows that wrapped into the neighbouring row)
// are then zeroed. moved_ needs no shift: it holds per-tick stamps and the next tick
// takes a fresh stamp before reading it. Measured: 2.0-2.4 ms with the old copy into
// freshly allocated arrays (8.4 MB per shift) for a 7^3-chunk window.
Array SimWorld::shift(Vector3i d) {
    wait_idle();
    Array vacated;
    if (d == Vector3i()) return vacated;
    if (wrapped_) {
        // Toroidal: new local L = old local L + d, so the offset moves by d; nothing is copied. Chunks whose
        // source lies outside the old window keep stale cells: they become unloaded (walls, not ticked) with
        // their state reset, and the caller loads them (load_from_coarse rewrites every cell and stamp).
        ocx_ += d.x; ocy_ += d.y; ocz_ += d.z;
        ox_ = ocx_ * CS; oy_ = ocy_ * CS; oz_ = ocz_ * CS;
        for (int cz = 0; cz < CZ_; cz++)
            for (int cy = 0; cy < CY_; cy++)
                for (int cx = 0; cx < CX_; cx++) {
                    const int sx = cx + d.x, sy = cy + d.y, sz = cz + d.z;
                    if (sx >= 0 && sy >= 0 && sz >= 0 && sx < CX_ && sy < CY_ && sz < CZ_) continue;
                    const int ci = cidx(cx, cy, cz);
                    active_now_[ci].store(0); active_next_[ci].store(0);
                    box_now_[ci].store(0); box_next_[ci].store(0);
                    dirty_[ci].store(0); dbox_[ci].store(0);
                    loaded_[ci] = 0;
                    defer_age_[ci] = 0;
                    vacated.push_back(Vector3i(cx, cy, cz));
                }
        return vacated;
    }
    const int dx = d.x * CS, dy = d.y * CS, dz = d.z * CS;
    const int64_t n = (int64_t)cells_.size();
    const int64_t off = (int64_t)dx + (int64_t)W_ * ((int64_t)dy + (int64_t)H_ * dz);
    if (off >= n || -off >= n) {
        std::fill(cells_.begin(), cells_.end(), Cell{0, 0, 0, 0});
    } else if (off > 0) {
        std::memmove(cells_.data(), cells_.data() + off, (size_t)(n - off) * sizeof(Cell));
    } else if (off < 0) {
        std::memmove(cells_.data() - off, cells_.data(), (size_t)(n + off) * sizeof(Cell));
    }
    // zero every cell whose source was outside the old window
    const int xs = std::max(0, -dx), xe = std::min(W_, W_ - dx);
    for (int z = 0; z < D_; z++) {
        const bool z_out = z + dz < 0 || z + dz >= D_;
        for (int y = 0; y < H_; y++) {
            Cell *row = &cells_[idx(0, y, z)];
            if (z_out || y + dy < 0 || y + dy >= H_ || xe <= xs) {
                std::memset(row, 0, (size_t)W_ * sizeof(Cell));
                continue;
            }
            if (xs > 0) std::memset(row, 0, (size_t)xs * sizeof(Cell));
            if (xe < W_) std::memset(row + xe, 0, (size_t)(W_ - xe) * sizeof(Cell));
        }
    }
    size_t ncn = (size_t)CX_ * CY_ * CZ_;
    std::vector<uint8_t> na(ncn, 0), nd(ncn, 0), nl(ncn, 0);
    std::vector<uint32_t> ndb(ncn, 0);
    for (int cz = 0; cz < CZ_; cz++)
        for (int cy = 0; cy < CY_; cy++)
            for (int cx = 0; cx < CX_; cx++) {
                int ox = cx + d.x, oy = cy + d.y, oz = cz + d.z;
                int ci = cidx(cx, cy, cz);
                if (ox < 0 || oy < 0 || oz < 0 || ox >= CX_ || oy >= CY_ || oz >= CZ_) {
                    vacated.push_back(Vector3i(cx, cy, cz));
                    continue;
                }
                int oi = cidx(ox, oy, oz);
                na[ci] = active_next_[oi].load() | active_now_[oi].load();
                nd[ci] = dirty_[oi].load();
                ndb[ci] = dbox_[oi].load();
                nl[ci] = loaded_[oi];
            }
    for (size_t i = 0; i < ncn; i++) {
        active_next_[i].store(na[i]);
        box_next_[i].store(na[i] ? BOX_FULL : 0u);
        box_now_[i].store(0);
        active_now_[i].store(0);
        dirty_[i].store(nd[i]);
        dbox_[i].store(ndb[i]);
    }
    loaded_.swap(nl); // vacated chunks are unloaded (walls) until the caller loads them
    return vacated;
}

Array SimWorld::get_dirty_chunks() const {
    wait_idle();
    Array a;
    for (int cz = 0; cz < CZ_; cz++)
        for (int cy = 0; cy < CY_; cy++)
            for (int cx = 0; cx < CX_; cx++)
                if (dirty_[cidx(cx, cy, cz)].load(std::memory_order_relaxed)) a.push_back(Vector3i(cx, cy, cz));
    return a;
}

PackedInt32Array SimWorld::get_dirty_boxes() const {
    wait_idle();
    PackedInt32Array out;
    for (int cz = 0; cz < CZ_; cz++)
        for (int cy = 0; cy < CY_; cy++)
            for (int cx = 0; cx < CX_; cx++) {
                int ci = cidx(cx, cy, cz);
                if (!dirty_[ci].load(std::memory_order_relaxed)) continue;
                uint32_t b = dbox_[ci].load(std::memory_order_relaxed);
                if (!(b & BOX_VALID)) b = BOX_FULL;
                const int32_t v[9] = {cx, cy, cz, (int32_t)(b & 15u), (int32_t)((b >> 4) & 15u), (int32_t)((b >> 8) & 15u),
                    (int32_t)((b >> 12) & 15u), (int32_t)((b >> 16) & 15u), (int32_t)((b >> 20) & 15u)};
                for (int k = 0; k < 9; k++) out.push_back(v[k]);
            }
    return out;
}

void SimWorld::clear_dirty() {
    wait_idle();
    size_t nc = (size_t)CX_ * CY_ * CZ_;
    for (size_t i = 0; i < nc; i++) { dirty_[i].store(0, std::memory_order_relaxed); dbox_[i].store(0, std::memory_order_relaxed); }
}

// ---------- micro simulation (ADR 0029) ----------

static bool is_pow2(int v) { return v > 0 && (v & (v - 1)) == 0; }

bool SimWorld::set_wrapped(bool p_on) {
    wait_idle();
    if (p_on && !(is_pow2(CX_) && is_pow2(CY_) && is_pow2(CZ_))) return false;
    wrapped_ = p_on;
    ox_ = oy_ = oz_ = 0; ocx_ = ocy_ = ocz_ = 0;
    mx_ = p_on ? W_ - 1 : -1; my_ = p_on ? H_ - 1 : -1; mz_ = p_on ? D_ - 1 : -1;
    cmx_ = p_on ? CX_ - 1 : -1; cmy_ = p_on ? CY_ - 1 : -1; cmz_ = p_on ? CZ_ - 1 : -1;
    return true;
}

// ---------- debug ----------

int SimWorld::count_material(int p_material) const {
    wait_idle();
    int n = 0;
    for (const Cell &c : cells_) n += c.mat == (uint8_t)p_material;
    return n;
}

int SimWorld::count_in_box(Vector3i p_min, Vector3i p_max, int p_material) const {
    wait_idle();
    int n = 0;
    for (int z = std::max(0, p_min.z); z < std::min(D_, p_max.z); z++)
        for (int y = std::max(0, p_min.y); y < std::min(H_, p_max.y); y++)
            for (int x = std::max(0, p_min.x); x < std::min(W_, p_max.x); x++)
                n += cells_[idx(x, y, z)].mat == (uint8_t)p_material;
    return n;
}

int SimWorld::count_unsupported(int p_material) const {
    int n = 0;
    for (int z = 0; z < D_; z++)
        for (int y = 1; y < H_; y++)
            for (int x = 0; x < W_; x++)
                if (cells_[idx(x, y, z)].mat == (uint8_t)p_material && cells_[idx(x, y - 1, z)].mat == 0) n++;
    return n;
}

int SimWorld::count_burning() const {
    wait_idle();
    int n = 0;
    for (const Cell &c : cells_) n += (c.flags & FLAG_BURNING) != 0;
    return n;
}

PackedInt32Array SimWorld::sample_cells(Vector3i p_center, int p_radius, int p_count, int p_seed, const PackedByteArray &p_mask) const {
    PackedInt32Array out;
    if (busy_.load() || p_mask.size() < MAX_MATERIALS || p_radius <= 0) return out;
    const uint8_t *mask = p_mask.ptr();
    uint32_t r = (uint32_t)p_seed * 2654435761u + 1u;
    const int span = 2 * p_radius + 1;
    for (int i = 0; i < p_count; i++) {
        r ^= r << 13; r ^= r >> 17; r ^= r << 5;
        const int x = p_center.x - p_radius + (int)(r % (uint32_t)span);
        r ^= r << 13; r ^= r >> 17; r ^= r << 5;
        const int y = p_center.y - p_radius + (int)(r % (uint32_t)span);
        r ^= r << 13; r ^= r >> 17; r ^= r << 5;
        const int z = p_center.z - p_radius + (int)(r % (uint32_t)span);
        auto readable = [&](int px, int py, int pz) { return in_world(px, py, pz) && loaded_[cidx(px >> 4, py >> 4, pz >> 4)] != 0; };
        if (!readable(x, y, z)) continue;
        const Cell &c = cells_[idx(x, y, z)];
        if (!mask[c.mat] && !(c.flags & FLAG_BURNING)) continue;
        out.push_back(x); out.push_back(y); out.push_back(z); out.push_back(c.mat);
        out.push_back(readable(x, y + 1, z) ? cells_[idx(x, y + 1, z)].mat : -1);
        out.push_back(readable(x, y - 1, z) ? cells_[idx(x, y - 1, z)].mat : -1);
        out.push_back(c.flags);
    }
    return out;
}

PackedInt32Array SimWorld::surface_map(Vector2i p_min_xz, Vector2i p_count, int p_step, int p_y_base) const {
    PackedInt32Array out;
    if (busy_.load() || p_count.x <= 0 || p_count.y <= 0) return out;   // never waits: empty while a tick runs
    const int st = std::max(1, p_step);
    out.resize((int64_t)p_count.x * p_count.y);
    int32_t *w = out.ptrw();
    for (int cz = 0; cz < p_count.y; cz++)
        for (int cx = 0; cx < p_count.x; cx++) {
            const int x = p_min_xz.x + cx * st, z = p_min_xz.y + cz * st;
            int32_t e = 0;
            if (x >= 0 && z >= 0 && x < W_ && z < D_) {
                for (int y = H_ - 1; y >= 0; y--) {
                    // chunks still loading (or never: above and below the terrain's full detail) are passed over
                    if ((y & (CS - 1)) == CS - 1 && loaded_[cidx(x >> 4, y >> 4, z >> 4)] != 1) { y -= CS - 1; continue; }
                    const uint8_t m = cells_[idx(x, y, z)].mat;
                    const uint8_t ct = mats_[m].cell_type;
                    if (m == 0 || ct == CELL_GAS || ct == CELL_FIRE) continue;
                    int depth = 0;
                    if (ct == CELL_LIQUID)
                        for (int d = y; d >= 0 && depth < 255 && mats_[cells_[idx(x, d, z)].mat].cell_type == CELL_LIQUID; d--) depth++;
                    e = (int32_t)m | (depth << 8) | ((y + p_y_base) << 16);
                    break;
                }
            }
            w[cx + p_count.x * cz] = e;
        }
    return out;
}

PackedInt32Array SimWorld::layer_histogram(int p_material) const {
    PackedInt32Array h;
    h.resize(H_);
    for (int y = 0; y < H_; y++) {
        int n = 0;
        for (int z = 0; z < D_; z++)
            for (int x = 0; x < W_; x++)
                n += cells_[idx(x, y, z)].mat == (uint8_t)p_material;
        h[y] = n;
    }
    return h;
}

PackedInt32Array SimWorld::material_histogram() const {
    PackedInt32Array h;
    h.resize((int)mats_.size());
    for (int i = 0; i < h.size(); i++) h[i] = 0;
    for (const Cell &c : cells_) if (c.mat < mats_.size()) h[c.mat] = h[c.mat] + 1;
    return h;
}

Dictionary SimWorld::chunk_info(Vector3i c) const {
    wait_idle();
    Dictionary d;
    if (c.x < 0 || c.y < 0 || c.z < 0 || c.x >= CX_ || c.y >= CY_ || c.z >= CZ_) return d;
    int ci = cidx(c.x, c.y, c.z);
    d["active_next"] = (int)active_next_[ci].load();
    d["active_now"] = (int)active_now_[ci].load();
    d["dirty"] = (int)dirty_[ci].load();
    d["loaded"] = (int)loaded_[ci];
    Dictionary counts;
    int burning = 0;
    for (int z = 0; z < CS; z++)
        for (int y = 0; y < CS; y++)
            for (int x = 0; x < CS; x++) {
                int i = idx(c.x * CS + x, c.y * CS + y, c.z * CS + z);
                int m = cells_[i].mat;
                counts[m] = (int)counts.get(m, 0) + 1;
                burning += (cells_[i].flags & FLAG_BURNING) != 0;
            }
    d["materials"] = counts;
    d["burning"] = burning;
    return d;
}

Dictionary SimWorld::get_stats() const {
    Dictionary d;
    size_t nc = (size_t)CX_ * CY_ * CZ_;
    int active = 0, dirty = 0;
    for (size_t i = 0; i < nc; i++) { active += active_next_[i].load(); dirty += dirty_[i].load(); }
    d["tick"] = (int64_t)tick_;
    d["chunks"] = (int64_t)nc;
    d["active_chunks"] = active;
    d["dirty_chunks"] = dirty;
    d["threads"] = threads_;
    d["materials"] = (int)mats_.size();
    d["reactions"] = (int)reactions_.size();
    d["bytes_per_voxel"] = 6; // cells + flags + aux + moved(2)
    d["memory_mb"] = (double)(cells_.size() * 6) / 1048576.0;
    d["still_woken"] = still_woken_.load(std::memory_order_relaxed);   // ADR 0068: still liquid turned moving
    return d;
}

int SimWorld::get_active_chunk_count() const {
    size_t nc = (size_t)CX_ * CY_ * CZ_;
    int n = 0;
    for (size_t i = 0; i < nc; i++) n += active_next_[i].load(std::memory_order_relaxed);
    return n;
}

Vector3i SimWorld::get_size_voxels() const { return Vector3i(W_, H_, D_); }
