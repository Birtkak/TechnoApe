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
SimWorld::~SimWorld() { pool_.shutdown(); }

void SimWorld::_bind_methods() {
    ClassDB::bind_method(D_METHOD("configure", "chunks", "threads", "seed"), &SimWorld::configure);
    ClassDB::bind_method(D_METHOD("set_materials", "defs"), &SimWorld::set_materials);
    ClassDB::bind_method(D_METHOD("set_reactions", "reactions"), &SimWorld::set_reactions);
    ClassDB::bind_method(D_METHOD("get_material_count"), &SimWorld::get_material_count);
    ClassDB::bind_method(D_METHOD("fill_box", "min", "max", "material"), &SimWorld::fill_box);
    ClassDB::bind_method(D_METHOD("scatter", "min", "max", "material", "density", "seed"), &SimWorld::scatter);
    ClassDB::bind_method(D_METHOD("set_voxel", "pos", "material"), &SimWorld::set_voxel);
    ClassDB::bind_method(D_METHOD("get_voxel", "pos"), &SimWorld::get_voxel);
    ClassDB::bind_method(D_METHOD("ignite", "pos"), &SimWorld::ignite);
    ClassDB::bind_method(D_METHOD("mark_all_loaded", "loaded"), &SimWorld::mark_all_loaded);
    ClassDB::bind_method(D_METHOD("tick", "count"), &SimWorld::tick);
    ClassDB::bind_method(D_METHOD("get_chunk_bytes", "chunk", "depth_bytes"), &SimWorld::get_chunk_bytes, DEFVAL(1));
    ClassDB::bind_method(D_METHOD("set_chunk_bytes", "chunk", "bytes", "depth_bytes"), &SimWorld::set_chunk_bytes, DEFVAL(1));
    ClassDB::bind_method(D_METHOD("shift", "delta_chunks"), &SimWorld::shift);
    ClassDB::bind_method(D_METHOD("get_dirty_chunks"), &SimWorld::get_dirty_chunks);
    ClassDB::bind_method(D_METHOD("clear_dirty"), &SimWorld::clear_dirty);
    ClassDB::bind_method(D_METHOD("count_material", "material"), &SimWorld::count_material);
    ClassDB::bind_method(D_METHOD("count_in_box", "min", "max", "material"), &SimWorld::count_in_box);
    ClassDB::bind_method(D_METHOD("count_unsupported", "material"), &SimWorld::count_unsupported);
    ClassDB::bind_method(D_METHOD("count_burning"), &SimWorld::count_burning);
    ClassDB::bind_method(D_METHOD("layer_histogram", "material"), &SimWorld::layer_histogram);
    ClassDB::bind_method(D_METHOD("material_histogram"), &SimWorld::material_histogram);
    ClassDB::bind_method(D_METHOD("chunk_info", "chunk"), &SimWorld::chunk_info);
    ClassDB::bind_method(D_METHOD("get_stats"), &SimWorld::get_stats);
    ClassDB::bind_method(D_METHOD("get_active_chunk_count"), &SimWorld::get_active_chunk_count);
    ClassDB::bind_method(D_METHOD("get_size_voxels"), &SimWorld::get_size_voxels);
    BIND_ENUM_CONSTANT(CELL_SOLID);
    BIND_ENUM_CONSTANT(CELL_LIQUID);
    BIND_ENUM_CONSTANT(CELL_GAS);
    BIND_ENUM_CONSTANT(CELL_FIRE);
}

// ---------- setup ----------

void SimWorld::configure(Vector3i p_chunks, int p_threads, int p_seed) {
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
    for (size_t i = 0; i < nc; i++) { active_now_[i].store(0); active_next_[i].store(0); dirty_[i].store(0); }
    loaded_.assign(nc, 1); // default: everything counts as loaded (fill_box/scatter worlds); streaming callers clear it
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
    any_fire_ = false;
    for (int i = 0; i < p_defs.size() && mats_.size() < MAX_MATERIALS; i++) {
        Dictionary d = p_defs[i];
        MaterialDef m;
        String ct = d.has("cell_type") ? (String)d["cell_type"] : String("liquid");
        if (ct == "solid") m.cell_type = CELL_SOLID;
        else if (ct == "gas") m.cell_type = CELL_GAS;
        else if (ct == "fire") m.cell_type = CELL_FIRE;
        else m.cell_type = CELL_LIQUID;
        m.liquid_sand = dget_b(d, "liquid_sand", false);
        m.liquid_static = dget_b(d, "liquid_static", false);
        m.burnable = dget_b(d, "burnable", false);
        m.density = clamp8(dget_f(d, "density", 1.0) * 16.0);
        m.disp = (uint8_t)std::clamp((int)std::lround(dget_f(d, "liquid_flow_speed", 0.9) * MAX_DISP), 0, MAX_DISP);
        m.gravity_prob = clamp8(dget_f(d, "liquid_gravity", 0.5) * 2.0 * 255.0); // 0.5 -> always falls
        m.gas_up = clamp8(dget_f(d, "gas_upwards_speed", 100));
        m.gas_horizontal = clamp8(dget_f(d, "gas_horizontal_speed", 100));
        m.gas_down = clamp8(dget_f(d, "gas_downwards_speed", 0));
        m.lifetime = clamp8(dget_int(d, "lifetime", 0));
        m.fire_hp = clamp8(dget_int(d, "fire_hp", 0));
        m.temperature_of_fire = clamp8(dget_int(d, "temperature_of_fire", 10));
        m.autoignition_temperature = clamp8(dget_int(d, "autoignition_temperature", 100));
        m.on_fire_convert_to = clamp8(dget_int(d, "on_fire_convert_to_material", 0));
        m.on_fire_flame_material = clamp8(dget_int(d, "on_fire_flame_material", 0));
        m.on_fire_smoke_material = clamp8(dget_int(d, "on_fire_smoke_material", 0));
        m.generates_smoke = clamp8(dget_f(d, "generates_smoke", 0) * 12.75);
        if (m.cell_type == CELL_FIRE || m.burnable) any_fire_ = true;
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
        bool static_cell = d.cell_type == CELL_SOLID || (d.cell_type == CELL_LIQUID && d.liquid_static);
        inert_[m] = static_cell && d.lifetime == 0 && !d.burnable && !reactive_[m];
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
}

inline void SimWorld::touch(int x, int y, int z) {
    int cx = x / CS, cy = y / CS, cz = z / CS;
    int ci = cidx(cx, cy, cz);
    active_next_[ci].store(1, std::memory_order_relaxed);
    dirty_[ci].store(1, std::memory_order_relaxed);
    int lx = x & (CS - 1), ly = y & (CS - 1), lz = z & (CS - 1);
    if (lx == 0 && cx > 0) active_next_[ci - 1].store(1, std::memory_order_relaxed);
    if (lx == CS - 1 && cx < CX_ - 1) active_next_[ci + 1].store(1, std::memory_order_relaxed);
    if (ly == 0 && cy > 0) active_next_[ci - CX_].store(1, std::memory_order_relaxed);
    if (ly == CS - 1 && cy < CY_ - 1) active_next_[ci + CX_].store(1, std::memory_order_relaxed);
    if (lz == 0 && cz > 0) active_next_[ci - CX_ * CY_].store(1, std::memory_order_relaxed);
    if (lz == CS - 1 && cz < CZ_ - 1) active_next_[ci + CX_ * CY_].store(1, std::memory_order_relaxed);
}

void SimWorld::fill_box(Vector3i p_min, Vector3i p_max, int p_material) {
    uint8_t m = (uint8_t)std::clamp(p_material, 0, (int)mats_.size() - 1);
    for (int z = std::max(0, p_min.z); z < std::min(D_, p_max.z); z++)
        for (int y = std::max(0, p_min.y); y < std::min(H_, p_max.y); y++)
            for (int x = std::max(0, p_min.x); x < std::min(W_, p_max.x); x++) {
                place(idx(x, y, z), m);
                touch(x, y, z);
            }
}

void SimWorld::scatter(Vector3i p_min, Vector3i p_max, int p_material, float p_density, int p_seed) {
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
    if (!in_world(p.x, p.y, p.z)) return;
    place(idx(p.x, p.y, p.z), (uint8_t)std::clamp(p_material, 0, (int)mats_.size() - 1));
    touch(p.x, p.y, p.z);
}

int SimWorld::get_voxel(Vector3i p) const {
    if (!in_world(p.x, p.y, p.z)) return -1;
    return cells_[idx(p.x, p.y, p.z)].mat;
}

void SimWorld::mark_all_loaded(bool p_loaded) {
    std::fill(loaded_.begin(), loaded_.end(), p_loaded ? 1 : 0);
}

void SimWorld::ignite(Vector3i p) {
    if (!in_world(p.x, p.y, p.z)) return;
    int i = idx(p.x, p.y, p.z);
    const MaterialDef &m = mats_[cells_[i].mat];
    if (!m.burnable) return;
    cells_[i].flags |= FLAG_BURNING;
    cells_[i].aux = m.fire_hp;
    touch(p.x, p.y, p.z);
}

// ---------- marks ----------

inline void SimWorld::mark_cell(int x, int y, int z, Ctx &c, bool changed) {
    int ox = x / CS - c.cx, oy = y / CS - c.cy, oz = z / CS - c.cz;
    uint32_t self_bit = 1u << mark_bit(ox, oy, oz);
    c.mk.wake |= self_bit;
    if (changed) c.mk.dirty |= self_bit;
    int lx = x & (CS - 1), ly = y & (CS - 1), lz = z & (CS - 1);
    if (lx == 0 && ox > -1) c.mk.wake |= 1u << mark_bit(ox - 1, oy, oz);
    if (lx == CS - 1 && ox < 1) c.mk.wake |= 1u << mark_bit(ox + 1, oy, oz);
    if (ly == 0 && oy > -1) c.mk.wake |= 1u << mark_bit(ox, oy - 1, oz);
    if (ly == CS - 1 && oy < 1) c.mk.wake |= 1u << mark_bit(ox, oy + 1, oz);
    if (lz == 0 && oz > -1) c.mk.wake |= 1u << mark_bit(ox, oy, oz - 1);
    if (lz == CS - 1 && oz < 1) c.mk.wake |= 1u << mark_bit(ox, oy, oz + 1);
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
                if (c.mk.dirty & bit) dirty_[ci].store(1, std::memory_order_relaxed);
            }
}

// ---------- movement ----------

// Noita: a liquid/powder can displace a cell that is a liquid or gas of lower density.
inline bool SimWorld::can_displace(uint8_t mover, uint8_t target) const {
    return displace_lut_[mover * MAX_MATERIALS + target] != 0;
}

inline bool SimWorld::try_move(int sx, int sy, int sz, int tx, int ty, int tz, Ctx &c) {
    if (!is_open(tx, ty, tz)) return false;
    int ti = idx(tx, ty, tz);
    int si = idx(sx, sy, sz);
    if (moved_[ti] == stamp_) return false;
    if (!can_displace(cells_[si].mat, cells_[ti].mat)) return false;
    std::swap(cells_[ti], cells_[si]);
    moved_[ti] = stamp_;
    moved_[si] = stamp_;
    mark_cell(sx, sy, sz, c, true);
    mark_cell(tx, ty, tz, c, true);
    return true;
}

void SimWorld::update_powder(int x, int y, int z, int i, Ctx &c) {
    const MaterialDef &m = mats_[cells_[i].mat];
    uint32_t r = rnd(c);
    if ((r & 0xFF) > m.gravity_prob) return;
    if (try_move(x, y, z, x, y - 1, z, c)) { c.moved++; return; }
    int d = (r >> 8) & 3;
    for (int k = 0; k < 4; k++) {
        int dd = (d + k) & 3;
        if (try_move(x, y, z, x + DX4[dd], y - 1, z + DZ4[dd], c)) { c.moved++; return; }
    }
}

void SimWorld::update_liquid(int x, int y, int z, int i, Ctx &c) {
    const uint8_t self = cells_[i].mat;
    const MaterialDef &m = mats_[self];
    uint32_t r = rnd(c);
    if ((r & 0xFF) > m.gravity_prob) return;
    if (try_move(x, y, z, x, y - 1, z, c)) { c.moved++; return; }
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
            if (!can_displace(self, cells_[pi].mat) || moved_[pi] == stamp_) break;
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
        if (t.cell_type == CELL_LIQUID && !t.liquid_sand && !t.burnable) {
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
                && ((r >> 20) & 7) == 0) {
            cells_[ni].flags |= FLAG_BURNING;
            cells_[ni].aux = t.fire_hp;
            mark_cell(nx, ny, nz, c, true);
            c.reacted++;
        }
        // Spawn flame or smoke into an air neighbour.
        if (cells_[ni].mat == 0 && moved_[ni] != stamp_) {
            uint8_t spawn = 0;
            if (((r >> 8) & 0xFF) < m.generates_smoke && m.on_fire_smoke_material) spawn = m.on_fire_smoke_material;
            else if (((r >> 16) & 0xFF) < 160 && m.on_fire_flame_material) spawn = m.on_fire_flame_material;
            if (spawn) {
                place(ni, spawn);
                moved_[ni] = stamp_;
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
    if (moved_[ni] == stamp_) return;
    uint8_t oa = swapped ? rx.out2 : rx.out1;
    uint8_t ob = swapped ? rx.out1 : rx.out2;
    place(i, oa);
    place(ni, ob);
    moved_[i] = stamp_;
    moved_[ni] = stamp_;
    mark_cell(x, y, z, c, true);
    mark_cell(nx, ny, nz, c, true);
    c.reacted++;
}

void SimWorld::update_chunk(int ci) {
    Ctx c;
    c.cx = ci % CX_; c.cy = (ci / CX_) % CY_; c.cz = ci / (CX_ * CY_);
    c.rng = (seed_ ^ ((uint32_t)ci * 2654435761u) ^ (tick_ * 40503u)) | 1u;
    int x0 = c.cx * CS, y0 = c.cy * CS, z0 = c.cz * CS;
    const bool alt = (tick_ & 1) != 0; // alternate x/z scan direction per tick to avoid bias
    for (int y = y0; y < y0 + CS; y++)
        for (int zz = 0; zz < CS; zz++) {
            int z = z0 + (alt ? CS - 1 - zz : zz);
            for (int xx = 0; xx < CS; xx++) {
                int x = x0 + (alt ? CS - 1 - xx : xx);
                int i = idx(x, y, z);
                uint8_t m = cells_[i].mat;
                if (m == 0 || inert_[m]) continue;
                if (moved_[i] == stamp_) continue;
                const MaterialDef &md = mats_[m];
                // Lifetime (gases, short-lived liquids, fire): count down, vanish.
                if (md.lifetime && !(cells_[i].flags & FLAG_BURNING)) {
                    if (cells_[i].aux > 0) { cells_[i].aux--; mark_cell(x, y, z, c, false); }
                    if (cells_[i].aux == 0) { place(i, 0); mark_cell(x, y, z, c, true); continue; }
                }
                if (cells_[i].flags & FLAG_BURNING) update_burning(x, y, z, i, c);
                if (cells_[i].mat != m) continue; // burnt away
                switch (md.cell_type) {
                    case CELL_SOLID: break;
                    case CELL_LIQUID:
                        if (md.liquid_static) break;
                        if (md.liquid_sand) update_powder(x, y, z, i, c);
                        else update_liquid(x, y, z, i, c);
                        break;
                    case CELL_GAS: update_gas(x, y, z, i, c); break;
                    case CELL_FIRE: update_fire(x, y, z, i, c); break;
                }
                if (any_reactions_ && reactive_[cells_[i].mat] && moved_[i] != stamp_) try_react(x, y, z, i, c);
            }
        }
    flush_marks(c);
    if (c.moved) moved_count_.fetch_add(c.moved, std::memory_order_relaxed);
    if (c.reacted) react_count_.fetch_add(c.reacted, std::memory_order_relaxed);
}

Dictionary SimWorld::tick(int p_count) {
    using clock = std::chrono::steady_clock;
    size_t nc = (size_t)CX_ * CY_ * CZ_;
    std::vector<int> work;
    work.reserve(nc);
    uint64_t active_total = 0, moved_total = 0, react_total = 0;
    double max_ms = 0.0;
    auto t_start = clock::now();
    for (int n = 0; n < p_count; n++) {
        auto t0 = clock::now();
        tick_++;
        stamp_++;
        if (stamp_ == 0) { std::fill(moved_.begin(), moved_.end(), 0); stamp_ = 1; }
        for (size_t i = 0; i < nc; i++) {
            active_now_[i].store(active_next_[i].load(std::memory_order_relaxed), std::memory_order_relaxed);
            active_next_[i].store(0, std::memory_order_relaxed);
        }
        moved_count_.store(0);
        react_count_.store(0);
        for (int pass = 0; pass < 8; pass++) {
            work.clear();
            for (int ci : pass_lists_[pass])
                if (active_now_[ci].load(std::memory_order_relaxed)) work.push_back(ci);
            active_total += work.size();
            pool_.run(work, [this](int ci) { update_chunk(ci); });
        }
        moved_total += moved_count_.load();
        react_total += react_count_.load();
        double ms = std::chrono::duration<double, std::milli>(clock::now() - t0).count();
        if (ms > max_ms) max_ms = ms;
    }
    double total_ms = std::chrono::duration<double, std::milli>(clock::now() - t_start).count();
    Dictionary d;
    d["ticks"] = p_count;
    d["threads"] = threads_;
    d["total_ms"] = total_ms;
    d["ms_per_tick"] = total_ms / std::max(1, p_count);
    d["max_tick_ms"] = max_ms;
    d["active_chunks_per_tick"] = (double)active_total / std::max(1, p_count);
    d["active_cells_per_tick"] = (double)active_total * (CS * CS * CS) / std::max(1, p_count);
    d["moved_cells_per_tick"] = (double)moved_total / std::max(1, p_count);
    d["reactions_per_tick"] = (double)react_total / std::max(1, p_count);
    d["active_cells_per_ms"] = total_ms > 0 ? (double)active_total * (CS * CS * CS) / total_ms : 0.0;
    d["moved_cells_total"] = (int64_t)moved_total;
    return d;
}

// ---------- render feed ----------

PackedByteArray SimWorld::get_chunk_bytes(Vector3i c, int p_depth_bytes) const {
    PackedByteArray out;
    const int n = CS * CS * CS;
    const int bpv = p_depth_bytes == 2 ? 2 : 1;
    out.resize(n * bpv);
    uint8_t *w = out.ptrw();
    if (bpv == 2) std::memset(w, 0, n * 2);
    int x0 = c.x * CS, y0 = c.y * CS, z0 = c.z * CS;
    for (int z = 0; z < CS; z++)
        for (int x = 0; x < CS; x++)
            for (int y = 0; y < CS; y++)
                w[(y + CS * (x + CS * z)) * bpv] = cells_[idx(x0 + x, y0 + y, z0 + z)].mat;
    return out;
}

void SimWorld::set_chunk_bytes(Vector3i c, const PackedByteArray &p_bytes, int p_depth_bytes) {
    if (c.x < 0 || c.y < 0 || c.z < 0 || c.x >= CX_ || c.y >= CY_ || c.z >= CZ_) return;
    const int n = CS * CS * CS;
    const int bpv = p_depth_bytes == 2 ? 2 : 1;
    if (p_bytes.size() < n * bpv) return;
    const uint8_t *r = p_bytes.ptr();
    int x0 = c.x * CS, y0 = c.y * CS, z0 = c.z * CS;
    for (int z = 0; z < CS; z++)
        for (int x = 0; x < CS; x++)
            for (int y = 0; y < CS; y++) {
                uint8_t m = r[(y + CS * (x + CS * z)) * bpv];
                if (m >= mats_.size()) m = 0;
                place(idx(x0 + x, y0 + y, z0 + z), m);
            }
    int ci = cidx(c.x, c.y, c.z);
    active_next_[ci].store(1, std::memory_order_relaxed); // let it settle once
    loaded_[ci] = 1;
}

// Slide the simulation window by whole chunks. Cells that stay inside move in memory;
// vacated chunks are cleared to air and returned so the caller loads them from the world.
Array SimWorld::shift(Vector3i d) {
    Array vacated;
    if (d == Vector3i()) return vacated;
    const int dx = d.x * CS, dy = d.y * CS, dz = d.z * CS;
    std::vector<Cell> nc(cells_.size(), Cell{0, 0, 0, 0});
    std::vector<uint16_t> nm(moved_.size(), 0);
    // new[x,y,z] = old[x+dx, y+dy, z+dz]
    for (int z = 0; z < D_; z++) {
        int oz = z + dz;
        if (oz < 0 || oz >= D_) continue;
        for (int y = 0; y < H_; y++) {
            int oy = y + dy;
            if (oy < 0 || oy >= H_) continue;
            int xs = std::max(0, -dx), xe = std::min(W_, W_ - dx);
            if (xe <= xs) continue;
            std::memcpy(&nc[idx(xs, y, z)], &cells_[idx(xs + dx, oy, oz)], (xe - xs) * sizeof(Cell));
            std::memcpy(&nm[idx(xs, y, z)], &moved_[idx(xs + dx, oy, oz)], (xe - xs) * sizeof(uint16_t));
        }
    }
    cells_.swap(nc);
    moved_.swap(nm);
    size_t ncn = (size_t)CX_ * CY_ * CZ_;
    std::vector<uint8_t> na(ncn, 0), nd(ncn, 0), nl(ncn, 0);
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
                nl[ci] = loaded_[oi];
            }
    for (size_t i = 0; i < ncn; i++) {
        active_next_[i].store(na[i]);
        active_now_[i].store(0);
        dirty_[i].store(nd[i]);
    }
    loaded_.swap(nl); // vacated chunks are unloaded (walls) until the caller loads them
    return vacated;
}

Array SimWorld::get_dirty_chunks() const {
    Array a;
    for (int cz = 0; cz < CZ_; cz++)
        for (int cy = 0; cy < CY_; cy++)
            for (int cx = 0; cx < CX_; cx++)
                if (dirty_[cidx(cx, cy, cz)].load(std::memory_order_relaxed)) a.push_back(Vector3i(cx, cy, cz));
    return a;
}

void SimWorld::clear_dirty() {
    size_t nc = (size_t)CX_ * CY_ * CZ_;
    for (size_t i = 0; i < nc; i++) dirty_[i].store(0, std::memory_order_relaxed);
}

// ---------- debug ----------

int SimWorld::count_material(int p_material) const {
    int n = 0;
    for (const Cell &c : cells_) n += c.mat == (uint8_t)p_material;
    return n;
}

int SimWorld::count_in_box(Vector3i p_min, Vector3i p_max, int p_material) const {
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
    int n = 0;
    for (const Cell &c : cells_) n += (c.flags & FLAG_BURNING) != 0;
    return n;
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
    Dictionary d;
    if (c.x < 0 || c.y < 0 || c.z < 0 || c.x >= CX_ || c.y >= CY_ || c.z >= CZ_) return d;
    int ci = cidx(c.x, c.y, c.z);
    d["active_next"] = (int)active_next_[ci].load();
    d["active_now"] = (int)active_now_[ci].load();
    d["dirty"] = (int)dirty_[ci].load();
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
    return d;
}

int SimWorld::get_active_chunk_count() const {
    size_t nc = (size_t)CX_ * CY_ * CZ_;
    int n = 0;
    for (size_t i = 0; i < nc; i++) n += active_next_[i].load(std::memory_order_relaxed);
    return n;
}

Vector3i SimWorld::get_size_voxels() const { return Vector3i(W_, H_, D_); }
