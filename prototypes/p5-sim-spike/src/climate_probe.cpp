#include "climate_probe.h"

#include <godot_cpp/core/class_db.hpp>

#include <algorithm>

using namespace godot;

void ClimateProbe::_bind_methods() {
    ClassDB::bind_method(D_METHOD("probe", "sim", "start", "half", "cap", "open", "heat", "heat_radius", "heat_falloff"), &ClimateProbe::probe);
}

Dictionary ClimateProbe::probe(const Ref<SimWorld> &sim, Vector3i start, Vector3i half, int cap, const PackedByteArray &open,
        const PackedFloat32Array &heat, int heat_radius, float falloff) {
    Dictionary out;
    out["enclosed"] = false;
    out["cells"] = 0;
    out["heat"] = 0.0;
    out["heat_cells"] = 0;
    if (sim.is_null()) return out;
    // no wait for a running tick (SimWorld::get_voxel): an approximate reading, and waiting stalled play 10-14 ms in a fire
    auto mat = [&](int x, int y, int z) -> int {   // -1 outside the window or unloaded
        if (!sim->is_open(x, y, z)) return -1;
        return sim->cells_[sim->idx(x, y, z)].mat;
    };
    auto is_open_cell = [&](int m) { return m >= 0 && m < open.size() && open[m]; };

    // heat: every heat cell within the radius
    double h = 0.0;
    int hc = 0;
    if (!heat.is_empty() && heat_radius > 0) {
        const float f2 = std::max(falloff, 0.5f) * std::max(falloff, 0.5f);
        for (int z = -heat_radius; z <= heat_radius; z++)
            for (int y = -heat_radius; y <= heat_radius; y++)
                for (int x = -heat_radius; x <= heat_radius; x++) {
                    const int m = mat(start.x + x, start.y + y, start.z + z);
                    if (m <= 0 || m >= heat.size() || heat[m] <= 0.0f) continue;
                    const float d2 = (float)(x * x + y * y + z * z);
                    h += heat[m] / (1.0f + d2 / f2);
                    hc++;
                }
    }
    out["heat"] = h;
    out["heat_cells"] = hc;

    // the room: a fill through open cells inside the box [start - half, start + half]
    Vector3i s = start;
    if (!is_open_cell(mat(s.x, s.y, s.z))) {
        s.y += 1;   // the head in a plant or a slab: try just above
        if (!is_open_cell(mat(s.x, s.y, s.z))) return out;
    }
    // no roof overhead within the box: outdoors, no fill needed (the common case, and the expensive one)
    bool roof = false;
    for (int k = 1; k <= half.y && !roof; k++) {
        const int m = mat(s.x, s.y + k, s.z);
        if (m < 0) break;
        roof = !is_open_cell(m);
    }
    if (!roof) return out;
    const int bx = 2 * half.x + 1, by = 2 * half.y + 1, bz = 2 * half.z + 1;
    const size_t n = (size_t)bx * by * bz;
    if (stamp_.size() != n) {
        stamp_.assign(n, 0);
        gen_ = 0;
    }
    if (++gen_ == 0) {   // wrapped: clear once
        std::fill(stamp_.begin(), stamp_.end(), 0);
        gen_ = 1;
    }
    auto at = [&](int x, int y, int z) -> int64_t {   // box index of a window cell, -1 outside the box
        const int lx = x - (start.x - half.x), ly = y - (start.y - half.y), lz = z - (start.z - half.z);
        if (lx < 0 || ly < 0 || lz < 0 || lx >= bx || ly >= by || lz >= bz) return -1;
        return lx + (int64_t)bx * (ly + (int64_t)by * lz);
    };
    static const int D[6][3] = {{1, 0, 0}, {-1, 0, 0}, {0, 1, 0}, {0, -1, 0}, {0, 0, 1}, {0, 0, -1}};
    queue_.clear();
    stamp_[at(s.x, s.y, s.z)] = gen_;
    queue_.push_back(s.x);
    queue_.push_back(s.y);
    queue_.push_back(s.z);
    bool escaped = false;
    size_t head = 0;
    int filled = 0;
    while (head < queue_.size() && !escaped) {
        const int x = queue_[head], y = queue_[head + 1], z = queue_[head + 2];
        head += 3;
        if (++filled > cap) {
            escaped = true;
            break;
        }
        for (const auto &d : D) {
            const int nx = x + d[0], ny = y + d[1], nz = z + d[2];
            const int m = mat(nx, ny, nz);
            if (m < 0) {   // the window's edge or a chunk still loading: open to the world
                escaped = true;
                break;
            }
            if (!is_open_cell(m)) continue;
            const int64_t k = at(nx, ny, nz);
            if (k < 0) {   // left the box: too big to be a room
                escaped = true;
                break;
            }
            if (stamp_[k] == gen_) continue;
            stamp_[k] = gen_;
            queue_.push_back(nx);
            queue_.push_back(ny);
            queue_.push_back(nz);
        }
    }
    out["enclosed"] = !escaped;
    out["cells"] = filled;
    return out;
}
