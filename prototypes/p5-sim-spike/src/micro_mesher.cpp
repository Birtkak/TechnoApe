#include "micro_mesher.h"

#include <godot_cpp/classes/mesh.hpp>
#include <godot_cpp/core/class_db.hpp>
#include <godot_cpp/variant/packed_vector2_array.hpp>
#include <godot_cpp/variant/packed_vector3_array.hpp>

#include <algorithm>
#include <cstdint>
#include <vector>

using namespace godot;

void MicroMesher::_bind_methods() {
    ClassDB::bind_static_method("MicroMesher", D_METHOD("build_arrays", "cells", "size", "grid", "tiles", "atlas", "origin"), &MicroMesher::build_arrays);
    ClassDB::bind_static_method("MicroMesher", D_METHOD("build", "cells", "size", "grid", "tiles", "atlas", "origin"), &MicroMesher::build);
    ClassDB::bind_static_method("MicroMesher", D_METHOD("fill_box", "cells", "size", "min", "max", "material"), &MicroMesher::fill_box);
}

PackedInt32Array MicroMesher::fill_box(PackedInt32Array cells, Vector3i size, Vector3i mn, Vector3i mx, int material) {
    const int n = size.x * size.y * size.z;
    if (cells.size() < n) cells.resize(n);
    int32_t *c = cells.ptrw();
    for (int z = std::max(0, mn.z); z < std::min(size.z, mx.z); z++)
        for (int y = std::max(0, mn.y); y < std::min(size.y, mx.y); y++)
            for (int x = std::max(0, mn.x); x < std::min(size.x, mx.x); x++)
                c[x + size.x * (y + size.y * z)] = material;
    return cells;
}

Array MicroMesher::build_arrays(const PackedInt32Array &cells, Vector3i size, int grid, const PackedInt32Array &tiles, Vector2i atlas, Vector3 origin) {
    Array out;
    const int dims[3] = {size.x, size.y, size.z};
    const int n = dims[0] * dims[1] * dims[2];
    if (n <= 0 || cells.size() < n || grid <= 0 || atlas.x <= 0 || atlas.y <= 0) return out;
    const int32_t *c = cells.ptr();
    const int32_t *tl = tiles.ptr();
    const int ntiles = tiles.size();
    // material of a cell, 0 when air / not drawable / outside
    auto mat = [&](const int p[3]) -> int {
        if (p[0] < 0 || p[1] < 0 || p[2] < 0 || p[0] >= dims[0] || p[1] >= dims[1] || p[2] >= dims[2]) return 0;
        const int m = c[p[0] + dims[0] * (p[1] + dims[1] * p[2])];
        if (m <= 0 || m >= ntiles || tl[m] < 0) return 0;
        return m;
    };
    PackedVector3Array verts, normals;
    PackedVector2Array uvs;
    PackedInt32Array idx;
    const float inv = 1.0f / (float)grid;
    const float tu = 1.0f / (float)atlas.x, tv = 1.0f / (float)atlas.y;
    std::vector<uint64_t> mask;
    for (int d = 0; d < 3; d++) {
        const int u = (d + 1) % 3, v = (d + 2) % 3;
        const int du = dims[u], dv = dims[v];
        mask.resize((size_t)du * dv);
        for (int side = -1; side <= 1; side += 2) {
            for (int slice = 0; slice < dims[d]; slice++) {
                std::fill(mask.begin(), mask.end(), 0);
                bool anyq = false;
                for (int j = 0; j < dv; j++) {
                    for (int i = 0; i < du; i++) {
                        int p[3];
                        p[d] = slice; p[u] = i; p[v] = j;
                        const int m = mat(p);
                        if (!m) continue;
                        int q[3] = {p[0], p[1], p[2]};
                        q[d] += side;
                        if (mat(q)) continue; // covered by a solid neighbour
                        // key: material + the voxel this cell is in (quads never cross voxel borders)
                        mask[i + (size_t)j * du] = (uint64_t)m | ((uint64_t)(i / grid) << 20) | ((uint64_t)(j / grid) << 40);
                        anyq = true;
                    }
                }
                if (!anyq) continue;
                for (int j = 0; j < dv; j++) {
                    for (int i = 0; i < du;) {
                        const uint64_t k = mask[i + (size_t)j * du];
                        if (!k) { i++; continue; }
                        int w = 1;
                        while (i + w < du && mask[i + w + (size_t)j * du] == k) w++;
                        int h = 1;
                        for (; j + h < dv; h++) {
                            bool row = true;
                            for (int t = 0; t < w && row; t++) row = mask[i + t + (size_t)(j + h) * du] == k;
                            if (!row) break;
                        }
                        for (int hh = 0; hh < h; hh++)
                            for (int t = 0; t < w; t++) mask[i + t + (size_t)(j + hh) * du] = 0;
                        // ---- emit the quad
                        const int m = (int)(k & 0xFFFFF);
                        const int tile = tl[m];
                        const float tx0 = (float)(tile % atlas.x) * tu, ty0 = (float)(tile / atlas.x) * tv;
                        const int vu0 = (i / grid) * grid, vv0 = (j / grid) * grid; // voxel base along u, v
                        Vector3 nrm;
                        nrm[d] = (float)side;
                        const float plane = (float)(slice + (side > 0 ? 1 : 0));
                        Vector3 pts[4];
                        Vector2 tex[4];
                        const int cu[4] = {i, i + w, i + w, i};
                        const int cv[4] = {j, j, j + h, j + h};
                        for (int q = 0; q < 4; q++) {
                            Vector3 pp;
                            pp[d] = plane; pp[u] = (float)cu[q]; pp[v] = (float)cv[q];
                            pts[q] = origin + pp * inv;
                            // texture axes: y faces -> (x, z); x faces -> (z, up); z faces -> (x, up)
                            float fx, fy;
                            const float lu = (float)(cu[q] - vu0) * inv, lv = (float)(cv[q] - vv0) * inv;
                            if (d == 1) { fx = lv; fy = lu; }            // u = z, v = x
                            else if (d == 0) { fx = lv; fy = 1.0f - lu; } // u = y, v = z
                            else { fx = lu; fy = 1.0f - lv; }             // u = x, v = y
                            tex[q] = Vector2(tx0 + fx * tu, ty0 + fy * tv);
                        }
                        // winding: counter-clockwise seen from the normal side
                        const Vector3 cr = (pts[1] - pts[0]).cross(pts[3] - pts[0]);
                        int order[4] = {0, 1, 2, 3};
                        if (cr.dot(nrm) < 0.0f) { order[1] = 3; order[3] = 1; }
                        const int base = verts.size();
                        for (int q = 0; q < 4; q++) {
                            verts.push_back(pts[order[q]]);
                            normals.push_back(nrm);
                            uvs.push_back(tex[order[q]]);
                        }
                        idx.push_back(base); idx.push_back(base + 1); idx.push_back(base + 2);
                        idx.push_back(base); idx.push_back(base + 2); idx.push_back(base + 3);
                        i += w;
                    }
                }
            }
        }
    }
    if (idx.size() == 0) return out;
    out.resize(Mesh::ARRAY_MAX);
    out[Mesh::ARRAY_VERTEX] = verts;
    out[Mesh::ARRAY_NORMAL] = normals;
    out[Mesh::ARRAY_TEX_UV] = uvs;
    out[Mesh::ARRAY_INDEX] = idx;
    return out;
}

Ref<ArrayMesh> MicroMesher::build(const PackedInt32Array &cells, Vector3i size, int grid, const PackedInt32Array &tiles, Vector2i atlas, Vector3 origin) {
    Array arrays = build_arrays(cells, size, grid, tiles, atlas, origin);
    if (arrays.size() == 0) return Ref<ArrayMesh>();
    Ref<ArrayMesh> mesh;
    mesh.instantiate();
    mesh->add_surface_from_arrays(Mesh::PRIMITIVE_TRIANGLES, arrays);
    return mesh;
}
