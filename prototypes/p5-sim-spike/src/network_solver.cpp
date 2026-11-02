#include "network_solver.h"

#include <godot_cpp/core/class_db.hpp>

#include <algorithm>
#include <cstdint>
#include <unordered_map>
#include <vector>

using namespace godot;

void NetworkSolver::_bind_methods() {
    ClassDB::bind_method(D_METHOD("solve", "boxes", "roles", "amounts"), &NetworkSolver::solve);
    ClassDB::bind_method(D_METHOD("stats"), &NetworkSolver::stats);
    BIND_ENUM_CONSTANT(CONDUCTOR);
    BIND_ENUM_CONSTANT(SOURCE);
    BIND_ENUM_CONSTANT(SINK);
}

static inline uint64_t key_of(int x, int y, int z) {
    return ((uint64_t)(uint32_t)(x + (1 << 20)) << 42) | ((uint64_t)(uint32_t)(y + (1 << 20)) << 21) | (uint64_t)(uint32_t)(z + (1 << 20));
}

static int find(std::vector<int> &p, int i) {
    while (p[i] != i) { p[i] = p[p[i]]; i = p[i]; }
    return i;
}

PackedFloat32Array NetworkSolver::solve(const PackedInt32Array &boxes, const PackedInt32Array &roles, const PackedFloat32Array &amounts) {
    const int n = std::min((int)(boxes.size() / 6), (int)std::min(roles.size(), amounts.size()));
    PackedFloat32Array out;
    out.resize(n * 2);
    stats_ = Dictionary();
    if (n == 0) return out;
    // every block cell (4 voxels) a node covers -> the node; boxes touch when a cell's 6-neighbour is another node's
    std::unordered_map<uint64_t, int> owner;
    owner.reserve(n * 2);
    std::vector<int> parent(n);
    for (int i = 0; i < n; i++) {
        parent[i] = i;
        const int *b = boxes.ptr() + i * 6;
        for (int z = b[2]; z < b[2] + b[5]; z += 4)
            for (int y = b[1]; y < b[1] + b[4]; y += 4)
                for (int x = b[0]; x < b[0] + b[3]; x += 4) owner.emplace(key_of(x, y, z), i);
    }
    static const int D[6][3] = {{4, 0, 0}, {-4, 0, 0}, {0, 4, 0}, {0, -4, 0}, {0, 0, 4}, {0, 0, -4}};
    for (int i = 0; i < n; i++) {
        const int *b = boxes.ptr() + i * 6;
        for (int z = b[2]; z < b[2] + b[5]; z += 4)
            for (int y = b[1]; y < b[1] + b[4]; y += 4)
                for (int x = b[0]; x < b[0] + b[3]; x += 4)
                    for (auto &d : D) {
                        auto it = owner.find(key_of(x + d[0], y + d[1], z + d[2]));
                        if (it == owner.end() || it->second == i) continue;
                        const int a = find(parent, i), c = find(parent, it->second);
                        if (a != c) parent[a] = c;
                    }
    }
    // per component: supply, demand, the weakest conductor
    std::vector<double> supply(n, 0), demand(n, 0), cap(n, 1e30);
    std::vector<int> comp(n);
    for (int i = 0; i < n; i++) {
        comp[i] = find(parent, i);
        const int r = roles[i];
        if (r == SOURCE) supply[comp[i]] += amounts[i];
        else if (r == SINK) demand[comp[i]] += amounts[i];
        else cap[comp[i]] = std::min(cap[comp[i]], (double)amounts[i]);
    }
    double s_all = 0, d_all = 0, f_all = 0;
    int count = 0;
    for (int i = 0; i < n; i++) {
        if (comp[i] != i) continue;
        count++;
        s_all += supply[i]; d_all += demand[i];
        f_all += std::min(std::min(supply[i], demand[i]), cap[i]);
    }
    for (int i = 0; i < n; i++) {
        const int c = comp[i];
        const double flow = std::min(std::min(supply[c], demand[c]), cap[c]);
        float f = 0;
        if (roles[i] == SOURCE && supply[c] > 0) f = (float)(flow * amounts[i] / supply[c]);
        else if (roles[i] == SINK && demand[c] > 0) f = (float)(flow * amounts[i] / demand[c]);
        out[i * 2] = (float)c;
        out[i * 2 + 1] = f;
    }
    stats_["count"] = count;
    stats_["supply"] = s_all;
    stats_["demand"] = d_all;
    stats_["flow"] = f_all;
    return out;
}
