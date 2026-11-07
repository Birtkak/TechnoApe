#include "shape_tool.h"

#include <godot_cpp/classes/mesh.hpp>
#include <godot_cpp/core/class_db.hpp>
#include <godot_cpp/variant/packed_vector3_array.hpp>

#include <algorithm>
#include <cmath>

using namespace godot;

namespace {
const int DX[6] = {1, -1, 0, 0, 0, 0}, DY[6] = {0, 0, 1, -1, 0, 0}, DZ[6] = {0, 0, 0, 0, 1, -1};
}

void ShapeTool::_bind_methods() {
    ClassDB::bind_method(D_METHOD("cells", "params", "lo"), &ShapeTool::cells);
    ClassDB::bind_method(D_METHOD("oriented_size", "size", "rot"), &ShapeTool::oriented_size);
    ClassDB::bind_method(D_METHOD("local_size", "world_size", "rot"), &ShapeTool::local_size);
    ClassDB::bind_method(D_METHOD("turn", "rot", "axis"), &ShapeTool::turn);
    ClassDB::bind_method(D_METHOD("mesh", "cells", "lo", "size"), &ShapeTool::mesh);
    ClassDB::bind_method(D_METHOD("count_free", "sim", "cells", "window", "mask"), &ShapeTool::count_free);
    ClassDB::bind_method(D_METHOD("fill", "sim", "cells", "material", "mats", "window", "mask"), &ShapeTool::fill);
    ClassDB::bind_method(D_METHOD("read", "sim", "lo", "size", "window"), &ShapeTool::read);
    ClassDB::bind_method(D_METHOD("count_free_by_material", "sim", "cells", "mats", "window", "mask"), &ShapeTool::count_free_by_material);
    ClassDB::bind_method(D_METHOD("select", "sim", "lo", "size", "window", "mask"), &ShapeTool::select);
    ClassDB::bind_method(D_METHOD("histogram", "sim", "cells", "window"), &ShapeTool::histogram);
    ClassDB::bind_method(D_METHOD("tally", "bytes"), &ShapeTool::tally);
    ClassDB::bind_method(D_METHOD("runs", "grid"), &ShapeTool::runs);
    ClassDB::bind_method(D_METHOD("grid_of", "cells", "material", "mats", "lo", "size"), &ShapeTool::grid_of);
    ClassDB::bind_method(D_METHOD("unruns", "runs"), &ShapeTool::unruns);
    ClassDB::bind_method(D_METHOD("trim", "size", "grid"), &ShapeTool::trim);
}

// The 24 rotations of the cube: signed permutation matrices with determinant +1, identity first.
const std::vector<ShapeTool::Mat3> &ShapeTool::rotations() {
    static std::vector<Mat3> rs;
    if (!rs.empty()) return rs;
    const int perms[6][3] = {{0, 1, 2}, {0, 2, 1}, {1, 0, 2}, {1, 2, 0}, {2, 0, 1}, {2, 1, 0}};
    for (const auto &pm : perms)
        for (int sg = 0; sg < 8; sg++) {
            Mat3 m{};
            for (int r = 0; r < 3; r++) m[r * 3 + pm[r]] = (sg >> r) & 1 ? -1 : 1;
            const int det = m[0] * (m[4] * m[8] - m[5] * m[7]) - m[1] * (m[3] * m[8] - m[5] * m[6]) + m[2] * (m[3] * m[7] - m[4] * m[6]);
            if (det == 1) rs.push_back(m);
        }
    return rs;
}

int ShapeTool::index_of(const Mat3 &m) {
    const std::vector<Mat3> &rs = rotations();
    for (size_t i = 0; i < rs.size(); i++)
        if (rs[i] == m) return (int)i;
    return 0;
}

int ShapeTool::turn(int p_rot, int p_axis) const {
    const std::vector<Mat3> &rs = rotations();
    const Mat3 &r = rs[std::clamp(p_rot, 0, 23)];
    // a quarter turn about the world axis, applied after r
    Mat3 q{};
    if (p_axis == 1) q = {0, 0, 1, 0, 1, 0, -1, 0, 0};        // about y: x -> -z, z -> x
    else if (p_axis == 0) q = {1, 0, 0, 0, 0, -1, 0, 1, 0};   // about x: y -> z
    else q = {0, -1, 0, 1, 0, 0, 0, 0, 1};                    // about z: x -> y
    Mat3 m{};
    for (int i = 0; i < 3; i++)
        for (int j = 0; j < 3; j++)
            for (int k = 0; k < 3; k++) m[i * 3 + j] += q[i * 3 + k] * r[k * 3 + j];
    return index_of(m);
}

Vector3i ShapeTool::oriented_size(Vector3i s, int p_rot) const {
    const Mat3 &r = rotations()[std::clamp(p_rot, 0, 23)];
    const int v[3] = {s.x, s.y, s.z};
    int o[3];
    for (int i = 0; i < 3; i++) o[i] = std::abs(r[i * 3]) * v[0] + std::abs(r[i * 3 + 1]) * v[1] + std::abs(r[i * 3 + 2]) * v[2];
    return Vector3i(o[0], o[1], o[2]);
}

Vector3i ShapeTool::local_size(Vector3i w, int p_rot) const {
    const Mat3 &r = rotations()[std::clamp(p_rot, 0, 23)];
    const int v[3] = {w.x, w.y, w.z};
    int o[3];   // the transpose of a permutation maps back
    for (int j = 0; j < 3; j++) o[j] = std::abs(r[j]) * v[0] + std::abs(r[3 + j]) * v[1] + std::abs(r[6 + j]) * v[2];
    return Vector3i(o[0], o[1], o[2]);
}

// Is local cell (x, y, z) of a shape of local size s part of it? Round shapes are measured at cell
// centres against ellipses that fit the box; hollow keeps a shell of that thickness (round shapes and
// boxes: the side walls, open top and bottom, like a tower; the sphere: a shell; stairs and slope: the
// layer under the walking surface).
bool ShapeTool::inside(const String &shape, int x, int y, int z, Vector3i s, int hollow, int step) const {
    const float fx = x + 0.5f, fy = y + 0.5f, fz = z + 0.5f;
    if (shape == "box" || shape == "wall") {
        if (hollow <= 0) return true;
        return x < hollow || z < hollow || x >= s.x - hollow || z >= s.z - hollow;
    }
    if (shape == "cylinder" || shape == "cone") {
        float rx = s.x * 0.5f, rz = s.z * 0.5f;
        if (shape == "cone") {
            const float k = 1.0f - y / (float)s.y;   // full at the bottom row, narrowing up
            rx *= k;
            rz *= k;
        }
        const float dx = fx - s.x * 0.5f, dz = fz - s.z * 0.5f;
        if (rx <= 0.0f || rz <= 0.0f) return false;
        if ((dx * dx) / (rx * rx) + (dz * dz) / (rz * rz) > 1.0f) return false;
        if (hollow <= 0) return true;
        const float ix = rx - hollow, iz = rz - hollow;
        return ix <= 0.0f || iz <= 0.0f || (dx * dx) / (ix * ix) + (dz * dz) / (iz * iz) > 1.0f;
    }
    if (shape == "sphere") {
        const float rx = s.x * 0.5f, ry = s.y * 0.5f, rz = s.z * 0.5f;
        const float dx = fx - rx, dy = fy - ry, dz = fz - rz;
        if ((dx * dx) / (rx * rx) + (dy * dy) / (ry * ry) + (dz * dz) / (rz * rz) > 1.0f) return false;
        if (hollow <= 0) return true;
        const float ix = rx - hollow, iy = ry - hollow, iz = rz - hollow;
        return ix <= 0.0f || iy <= 0.0f || iz <= 0.0f || (dx * dx) / (ix * ix) + (dy * dy) / (iy * iy) + (dz * dz) / (iz * iz) > 1.0f;
    }
    if (shape == "slope" || shape == "stairs") {
        // rises along +z to the full height at the far end
        float top;
        if (shape == "slope") {
            top = s.y * fz / s.z;
        } else {
            const int st = std::max(1, step);
            const int n = std::max(1, (s.y + st - 1) / st);                  // steps
            const int k = std::min(n - 1, (int)(fz * n / s.z));               // this row's step
            top = (float)std::min(s.y, (k + 1) * st);
        }
        const float surface = shape == "slope" ? top + 0.5f : top;   // a slope keeps the cell its surface cuts
        if (fy > surface) return false;
        return hollow <= 0 || fy > surface - hollow;
    }
    return false;
}

Dictionary ShapeTool::cells(const Dictionary &p, Vector3i lo) const {
    Dictionary out;
    const String shape = p.get("shape", "box");
    const Vector3i s = p.get("size", Vector3i(4, 4, 4));
    const int hollow = p.get("hollow", 0);
    const int step = p.get("step", 4);
    const int rot = std::clamp((int)p.get("rot", 0), 0, 23);
    const bool mirror = p.get("mirror", false);
    const PackedByteArray grid = p.get("grid", PackedByteArray());
    const bool is_grid = shape == "grid";
    if (s.x <= 0 || s.y <= 0 || s.z <= 0 || (is_grid && grid.size() < (int64_t)s.x * s.y * s.z)) return out;
    const Mat3 &r = rotations()[rot];
    const Vector3i os = oriented_size(s, rot);
    PackedInt32Array cs;
    PackedByteArray mats;
    for (int y = 0; y < s.y; y++)
        for (int z = 0; z < s.z; z++)
            for (int x = 0; x < s.x; x++) {
                uint8_t m = 1;
                if (is_grid) {
                    m = grid[x + s.x * (z + s.z * y)];
                    if (!m) continue;
                } else if (!inside(shape, x, y, z, s, hollow, step)) {
                    continue;
                }
                // doubled coordinates about the box centre (odd numbers), rotated, then back to cells
                const int c[3] = {2 * x + 1 - s.x, 2 * y + 1 - s.y, 2 * z + 1 - s.z};
                const int o[3] = {os.x, os.y, os.z};
                int w[3];
                for (int i = 0; i < 3; i++) {
                    const int v = r[i * 3] * c[0] + r[i * 3 + 1] * c[1] + r[i * 3 + 2] * c[2];
                    w[i] = (v + o[i] - 1) / 2;
                }
                if (mirror) w[0] = os.x - 1 - w[0];
                cs.push_back(lo.x + w[0]);
                cs.push_back(lo.y + w[1]);
                cs.push_back(lo.z + w[2]);
                if (is_grid) mats.push_back(m);
            }
    out["cells"] = cs;
    out["mats"] = mats;
    out["size"] = os;
    return out;
}

Array ShapeTool::mesh(const PackedInt32Array &p_cells, Vector3i lo, Vector3i s) const {
    Array arrays;
    if (s.x <= 0 || s.y <= 0 || s.z <= 0) return arrays;
    std::vector<uint8_t> occ((size_t)s.x * s.y * s.z, 0);
    auto at = [&](int x, int y, int z) -> int64_t {
        if (x < 0 || y < 0 || z < 0 || x >= s.x || y >= s.y || z >= s.z) return -1;
        return x + (int64_t)s.x * (y + (int64_t)s.y * z);
    };
    for (int64_t i = 0; i + 2 < p_cells.size(); i += 3) {
        const int64_t k = at(p_cells[i] - lo.x, p_cells[i + 1] - lo.y, p_cells[i + 2] - lo.z);
        if (k >= 0) occ[k] = 1;
    }
    // corners of each face, wound clockwise seen from outside (Godot's front faces)
    static const int C[6][4][3] = {
            {{1, 0, 0}, {1, 0, 1}, {1, 1, 1}, {1, 1, 0}},
            {{0, 0, 0}, {0, 1, 0}, {0, 1, 1}, {0, 0, 1}},
            {{0, 1, 0}, {1, 1, 0}, {1, 1, 1}, {0, 1, 1}},
            {{0, 0, 0}, {0, 0, 1}, {1, 0, 1}, {1, 0, 0}},
            {{0, 0, 1}, {0, 1, 1}, {1, 1, 1}, {1, 0, 1}},
            {{0, 0, 0}, {1, 0, 0}, {1, 1, 0}, {0, 1, 0}},
    };
    PackedVector3Array verts, normals;
    PackedInt32Array idx;
    for (int z = 0; z < s.z; z++)
        for (int y = 0; y < s.y; y++)
            for (int x = 0; x < s.x; x++) {
                if (!occ[at(x, y, z)]) continue;
                for (int f = 0; f < 6; f++) {
                    const int64_t n = at(x + DX[f], y + DY[f], z + DZ[f]);
                    if (n >= 0 && occ[n]) continue;
                    const int base = verts.size();
                    for (int k = 0; k < 4; k++) {
                        verts.push_back(Vector3(x + C[f][k][0], y + C[f][k][1], z + C[f][k][2]));
                        normals.push_back(Vector3(DX[f], DY[f], DZ[f]));
                    }
                    idx.push_back(base);
                    idx.push_back(base + 1);
                    idx.push_back(base + 2);
                    idx.push_back(base);
                    idx.push_back(base + 2);
                    idx.push_back(base + 3);
                }
            }
    arrays.resize(Mesh::ARRAY_MAX);
    arrays[Mesh::ARRAY_VERTEX] = verts;
    arrays[Mesh::ARRAY_NORMAL] = normals;
    arrays[Mesh::ARRAY_INDEX] = idx;
    return arrays;
}

int ShapeTool::count_free(const Ref<SimWorld> &sim, const PackedInt32Array &p_cells, Vector3i win, const PackedByteArray &mask) const {
    if (sim.is_null()) return 0;
    sim->wait_readable();   // read only: no wait for the pre-tick phase
    int n = 0;
    for (int64_t i = 0; i + 2 < p_cells.size(); i += 3) {
        const int x = p_cells[i] - win.x, y = p_cells[i + 1] - win.y, z = p_cells[i + 2] - win.z;
        if (!sim->is_open(x, y, z)) continue;
        const uint8_t m = sim->cells_[sim->idx(x, y, z)].mat;
        if (m < mask.size() && mask[m]) n++;
    }
    return n;
}

Dictionary ShapeTool::count_free_by_material(const Ref<SimWorld> &sim, const PackedInt32Array &p_cells, const PackedByteArray &mats,
        Vector3i win, const PackedByteArray &mask) const {
    Dictionary out;
    if (sim.is_null()) return out;
    sim->wait_readable();   // read only: no wait for the pre-tick phase
    int counts[256] = {};
    for (int64_t i = 0, c = 0; i + 2 < p_cells.size(); i += 3, c++) {
        const int x = p_cells[i] - win.x, y = p_cells[i + 1] - win.y, z = p_cells[i + 2] - win.z;
        if (!sim->is_open(x, y, z) || c >= mats.size()) continue;
        const uint8_t m = sim->cells_[sim->idx(x, y, z)].mat;
        if (m < mask.size() && mask[m]) counts[mats[c]]++;
    }
    for (int m = 1; m < 256; m++)
        if (counts[m]) out[m] = counts[m];
    return out;
}

int ShapeTool::fill(const Ref<SimWorld> &sim, const PackedInt32Array &p_cells, int material, const PackedByteArray &mats, Vector3i win,
        const PackedByteArray &mask) const {
    if (sim.is_null()) return 0;
    sim->wait_idle();
    int n = 0;
    for (int64_t i = 0, c = 0; i + 2 < p_cells.size(); i += 3, c++) {
        const int x = p_cells[i] - win.x, y = p_cells[i + 1] - win.y, z = p_cells[i + 2] - win.z;
        if (!sim->is_open(x, y, z)) continue;
        const uint8_t m = sim->cells_[sim->idx(x, y, z)].mat;
        if (m >= mask.size() || !mask[m]) continue;
        sim->place_cell(x, y, z, (uint8_t)(mats.is_empty() ? material : mats[c]));
        n++;
    }
    return n;
}

PackedInt32Array ShapeTool::select(const Ref<SimWorld> &sim, Vector3i lo, Vector3i s, Vector3i win, const PackedByteArray &mask) const {
    PackedInt32Array out;
    if (sim.is_null()) return out;
    sim->wait_readable();   // read only: no wait for the pre-tick phase
    for (int y = 0; y < s.y; y++)
        for (int z = 0; z < s.z; z++)
            for (int x = 0; x < s.x; x++) {
                const int lx = lo.x + x - win.x, ly = lo.y + y - win.y, lz = lo.z + z - win.z;
                if (!sim->is_open(lx, ly, lz)) continue;
                const uint8_t m = sim->cells_[sim->idx(lx, ly, lz)].mat;
                if (m >= mask.size() || !mask[m]) continue;
                out.push_back(lo.x + x);
                out.push_back(lo.y + y);
                out.push_back(lo.z + z);
            }
    return out;
}

Dictionary ShapeTool::histogram(const Ref<SimWorld> &sim, const PackedInt32Array &p_cells, Vector3i win) const {
    Dictionary out;
    if (sim.is_null()) return out;
    sim->wait_readable();   // read only: no wait for the pre-tick phase
    int counts[256] = {};
    for (int64_t i = 0; i + 2 < p_cells.size(); i += 3) {
        const int x = p_cells[i] - win.x, y = p_cells[i + 1] - win.y, z = p_cells[i + 2] - win.z;
        if (sim->is_open(x, y, z)) counts[sim->cells_[sim->idx(x, y, z)].mat]++;
    }
    for (int m = 0; m < 256; m++)
        if (counts[m]) out[m] = counts[m];
    return out;
}

Dictionary ShapeTool::tally(const PackedByteArray &p_bytes) const {
    Dictionary out;
    std::vector<int64_t> counts(65536, 0);
    const uint8_t *b = p_bytes.ptr();
    for (int64_t i = 0; i + 1 < p_bytes.size(); i += 2) counts[(uint16_t)(b[i] | (b[i + 1] << 8))]++;
    for (int m = 0; m < 65536; m++)
        if (counts[m]) out[m] = counts[m];
    return out;
}

PackedByteArray ShapeTool::grid_of(const PackedInt32Array &p_cells, int material, const PackedByteArray &mats, Vector3i lo, Vector3i s) const {
    PackedByteArray g;
    if (s.x <= 0 || s.y <= 0 || s.z <= 0) return g;
    g.resize((int64_t)s.x * s.y * s.z);
    g.fill(0);
    for (int64_t i = 0, c = 0; i + 2 < p_cells.size(); i += 3, c++) {
        const int x = p_cells[i] - lo.x, y = p_cells[i + 1] - lo.y, z = p_cells[i + 2] - lo.z;
        if (x < 0 || y < 0 || z < 0 || x >= s.x || y >= s.y || z >= s.z) continue;
        g[x + (int64_t)s.x * (z + (int64_t)s.z * y)] = (uint8_t)(mats.is_empty() ? material : mats[c]);
    }
    return g;
}

PackedInt32Array ShapeTool::runs(const PackedByteArray &g) const {
    PackedInt32Array out;
    for (int64_t i = 0; i < g.size();) {
        int64_t j = i + 1;
        while (j < g.size() && g[j] == g[i]) j++;
        out.push_back(g[i]);
        out.push_back((int32_t)(j - i));
        i = j;
    }
    return out;
}

PackedByteArray ShapeTool::unruns(const PackedInt32Array &r) const {
    int64_t n = 0;
    for (int64_t i = 1; i < r.size(); i += 2) n += std::max(0, r[i]);
    PackedByteArray out;
    out.resize(n);
    int64_t k = 0;
    for (int64_t i = 0; i + 1 < r.size(); i += 2)
        for (int c = 0; c < r[i + 1]; c++) out[k++] = (uint8_t)r[i];
    return out;
}

Dictionary ShapeTool::trim(Vector3i s, const PackedByteArray &g) const {
    Dictionary out;
    if ((int64_t)s.x * s.y * s.z != g.size()) return out;
    int lo[3] = {s.x, s.y, s.z}, hi[3] = {-1, -1, -1};
    int64_t i = 0;
    for (int y = 0; y < s.y; y++)
        for (int z = 0; z < s.z; z++)
            for (int x = 0; x < s.x; x++, i++) {
                if (!g[i]) continue;
                const int v[3] = {x, y, z};
                for (int a = 0; a < 3; a++) {
                    lo[a] = std::min(lo[a], v[a]);
                    hi[a] = std::max(hi[a], v[a]);
                }
            }
    if (hi[0] < 0) return out;
    const Vector3i n(hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1);
    PackedByteArray t;
    t.resize((int64_t)n.x * n.y * n.z);
    int64_t j = 0;
    for (int y = 0; y < n.y; y++)
        for (int z = 0; z < n.z; z++)
            for (int x = 0; x < n.x; x++) t[j++] = g[(lo[0] + x) + (int64_t)s.x * ((lo[2] + z) + (int64_t)s.z * (lo[1] + y))];
    out["size"] = n;
    out["grid"] = t;
    out["offset"] = Vector3i(lo[0], lo[1], lo[2]);
    return out;
}

PackedByteArray ShapeTool::read(const Ref<SimWorld> &sim, Vector3i lo, Vector3i s, Vector3i win) const {
    PackedByteArray out;
    if (sim.is_null() || s.x <= 0 || s.y <= 0 || s.z <= 0) return out;
    sim->wait_readable();   // read only: no wait for the pre-tick phase
    out.resize((int64_t)s.x * s.y * s.z);
    int64_t i = 0;
    for (int y = 0; y < s.y; y++)
        for (int z = 0; z < s.z; z++)
            for (int x = 0; x < s.x; x++) {
                const int lx = lo.x + x - win.x, ly = lo.y + y - win.y, lz = lo.z + z - win.z;
                out[i++] = sim->is_open(lx, ly, lz) ? sim->cells_[sim->idx(lx, ly, lz)].mat : 0;
            }
    return out;
}
