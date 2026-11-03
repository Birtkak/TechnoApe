#include "network_solver.h"

#include <godot_cpp/core/class_db.hpp>

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <unordered_map>
#include <unordered_set>
#include <vector>

using namespace godot;

void NetworkSolver::_bind_methods() {
    ClassDB::bind_method(D_METHOD("solve", "boxes", "roles", "amounts"), &NetworkSolver::solve);
    ClassDB::bind_method(D_METHOD("stats"), &NetworkSolver::stats);
    ClassDB::bind_method(D_METHOD("solve_dc", "boxes", "kinds", "a", "b"), &NetworkSolver::solve_dc);
    ClassDB::bind_method(D_METHOD("dc_stats"), &NetworkSolver::dc_stats);
    BIND_ENUM_CONSTANT(CONDUCTOR);
    BIND_ENUM_CONSTANT(SOURCE);
    BIND_ENUM_CONSTANT(SINK);
    BIND_ENUM_CONSTANT(SINK_LOW);
    BIND_ENUM_CONSTANT(DC_CONDUCTOR);
    BIND_ENUM_CONSTANT(DC_SOURCE);
    BIND_ENUM_CONSTANT(DC_LOAD);
}

static inline uint64_t key_of(int x, int y, int z) {
    return ((uint64_t)(uint32_t)(x + (1 << 20)) << 42) | ((uint64_t)(uint32_t)(y + (1 << 20)) << 21) | (uint64_t)(uint32_t)(z + (1 << 20));
}

static int find(std::vector<int> &p, int i) {
    while (p[i] != i) { p[i] = p[p[i]]; i = p[i]; }
    return i;
}

// every block cell (4 voxels) a node covers -> the node; boxes touch when a cell's 6-neighbour is another node's
void NetworkSolver::connect(const PackedInt32Array &boxes, int n, std::vector<int> &parent, std::vector<std::pair<int, int>> &edges) {
    std::unordered_map<uint64_t, int> owner;
    owner.reserve(n * 2);
    parent.assign(n, 0);
    for (int i = 0; i < n; i++) {
        parent[i] = i;
        const int *b = boxes.ptr() + i * 6;
        for (int z = b[2]; z < b[2] + b[5]; z += 4)
            for (int y = b[1]; y < b[1] + b[4]; y += 4)
                for (int x = b[0]; x < b[0] + b[3]; x += 4) owner.emplace(key_of(x, y, z), i);
    }
    static const int D[6][3] = {{4, 0, 0}, {-4, 0, 0}, {0, 4, 0}, {0, -4, 0}, {0, 0, 4}, {0, 0, -4}};
    std::unordered_set<uint64_t> seen;
    for (int i = 0; i < n; i++) {
        const int *b = boxes.ptr() + i * 6;
        for (int z = b[2]; z < b[2] + b[5]; z += 4)
            for (int y = b[1]; y < b[1] + b[4]; y += 4)
                for (int x = b[0]; x < b[0] + b[3]; x += 4)
                    for (auto &d : D) {
                        auto it = owner.find(key_of(x + d[0], y + d[1], z + d[2]));
                        if (it == owner.end() || it->second == i) continue;
                        const int j = it->second;
                        const uint64_t ek = ((uint64_t)std::min(i, j) << 32) | (uint32_t)std::max(i, j);
                        if (seen.insert(ek).second) edges.emplace_back(i, j);
                        const int a = find(parent, i), c = find(parent, j);
                        if (a != c) parent[a] = c;
                    }
    }
}

PackedFloat32Array NetworkSolver::solve(const PackedInt32Array &boxes, const PackedInt32Array &roles, const PackedFloat32Array &amounts) {
    const int n = std::min((int)(boxes.size() / 6), (int)std::min(roles.size(), amounts.size()));
    PackedFloat32Array out;
    out.resize(n * 2);
    stats_ = Dictionary();
    if (n == 0) return out;
    std::vector<int> parent;
    std::vector<std::pair<int, int>> edges;
    connect(boxes, n, parent, edges);
    // per component: supply, demand, the weakest conductor
    std::vector<double> supply(n, 0), demand(n, 0), low(n, 0), cap(n, 1e30);
    std::vector<int> comp(n);
    for (int i = 0; i < n; i++) {
        comp[i] = find(parent, i);
        const int r = roles[i];
        if (r == SOURCE) supply[comp[i]] += amounts[i];
        else if (r == SINK) demand[comp[i]] += amounts[i];
        else if (r == SINK_LOW) low[comp[i]] += amounts[i];
        else cap[comp[i]] = std::min(cap[comp[i]], (double)amounts[i]);
    }
    // the sinks first; low sinks (tanks filling) get what is left under the cap
    auto flow_of = [&](int c) { return std::min(std::min(supply[c], demand[c]), cap[c]); };
    auto low_of = [&](int c) { const double f = flow_of(c); return std::min(std::min(supply[c] - f, low[c]), cap[c] - f); };
    double s_all = 0, d_all = 0, f_all = 0;
    int count = 0;
    for (int i = 0; i < n; i++) {
        if (comp[i] != i) continue;
        count++;
        s_all += supply[i]; d_all += demand[i];
        f_all += flow_of(i) + low_of(i);
    }
    // sinks are filled in node order until the component's flow is spent (a machine runs whole or waits);
    // sources and low sinks share proportionally
    std::vector<double> left(n, 0);
    for (int i = 0; i < n; i++) if (comp[i] == i) left[i] = flow_of(i);
    for (int i = 0; i < n; i++) {
        const int c = comp[i];
        const double flow = flow_of(c), fl = low_of(c);
        float f = 0;
        if (roles[i] == SOURCE && supply[c] > 0) f = (float)((flow + fl) * amounts[i] / supply[c]);
        else if (roles[i] == SINK) { f = (float)std::min((double)amounts[i], left[c]); left[c] -= f; }
        else if (roles[i] == SINK_LOW && low[c] > 0) f = (float)(fl * amounts[i] / low[c]);
        out[i * 2] = (float)c;
        out[i * 2 + 1] = f;
    }
    stats_["count"] = count;
    stats_["supply"] = s_all;
    stats_["demand"] = d_all;
    stats_["flow"] = f_all;
    return out;
}

// ---------- the DC circuit (ADR 0123) ----------

PackedFloat32Array NetworkSolver::solve_dc(const PackedInt32Array &boxes, const PackedInt32Array &kinds, const PackedFloat32Array &pa, const PackedFloat32Array &pb) {
    const int n = std::min((int)(boxes.size() / 6), (int)std::min(kinds.size(), std::min(pa.size(), pb.size())));
    PackedFloat32Array out;
    out.resize(n * 4);
    dc_stats_ = Dictionary();
    if (n == 0) return out;
    std::vector<int> parent;
    std::vector<std::pair<int, int>> edges;
    connect(boxes, n, parent, edges);
    const double BUS = 1e-4;   // ohms across a machine's or source's own terminals
    // a node's own series resistance; an edge between touching nodes carries half of each
    std::vector<double> rnode(n);
    for (int i = 0; i < n; i++) rnode[i] = kinds[i] == DC_CONDUCTOR ? std::max((double)pa[i], 1e-6) : BUS;
    // adjacency with edge resistance
    struct E { int to; double r; };
    std::vector<std::vector<E>> adj(n);
    for (auto &e : edges) {
        const double r = 0.5 * (rnode[e.first] + rnode[e.second]);
        adj[e.first].push_back({e.second, r});
        adj[e.second].push_back({e.first, r});
    }
    // collapse runs of plain conductors with exactly two neighbours into one resistor between their ends
    std::vector<char> removed(n, 0);
    std::vector<int> chain_a(n, -1), chain_b(n, -1);      // for a removed node: the ends of its run
    std::vector<double> chain_ra(n, 0), chain_rt(n, 0);   // resistance from end a to this node's centre, and a to b
    for (int i = 0; i < n; i++) {
        if (kinds[i] != DC_CONDUCTOR || removed[i] || adj[i].size() != 2) continue;
        // walk left and right to the run's ends
        auto walk = [&](int from, int via, double &racc, std::vector<int> &path) {
            int prev = from, cur = via;
            while (kinds[cur] == DC_CONDUCTOR && adj[cur].size() == 2 && !removed[cur]) {
                path.push_back(cur);
                const E &e0 = adj[cur][0], &e1 = adj[cur][1];
                const int next = e0.to == prev ? e1.to : e0.to;
                racc += e0.to == prev ? e1.r : e0.r;
                if (next == cur || next == from) break;   // a loop of conductors: leave it to the solver
                prev = cur; cur = next;
            }
            return cur;
        };
        std::vector<int> left, right;
        double rl = adj[i][0].r, rr = adj[i][1].r;
        const int a = walk(i, adj[i][0].to, rl, left);
        const int b = walk(i, adj[i][1].to, rr, right);
        if (a == b || a == i || b == i) continue;   // a ring: not a chain
        // the run: left reversed, i, right; total resistance between a and b
        std::vector<int> run(left.rbegin(), left.rend());
        run.push_back(i);
        run.insert(run.end(), right.begin(), right.end());
        const double total = rl + rr;
        double acc = 0;
        for (size_t k = 0; k < run.size(); k++) {
            const int u = run[k];
            removed[u] = 1;
            chain_a[u] = a; chain_b[u] = b; chain_rt[u] = total;
            // resistance from a to this node's centre: the edge from the previous node, then half of this node
            const int prev = k == 0 ? a : run[k - 1];
            double er = 0;
            for (auto &e : adj[u]) if (e.to == prev) { er = e.r; break; }
            acc += er;
            chain_ra[u] = acc;   // acc is at u's centre: er already holds half of u
        }
        // replace the run in a's and b's adjacency with one edge
        auto relink = [&](int end, int first) {
            for (auto &e : adj[end]) if (e.to == first) { e.to = end == a ? b : a; e.r = total; }
        };
        relink(a, run.front());
        relink(b, run.back());
    }
    // index the kept nodes
    std::vector<int> idx(n, -1);
    std::vector<int> kept;
    for (int i = 0; i < n; i++) if (!removed[i]) { idx[i] = (int)kept.size(); kept.push_back(i); }
    const int m = (int)kept.size();
    // G v = rhs: G_ii = sum of edge conductances + the node's own load conductance or 1/r_internal
    std::vector<std::vector<std::pair<int, double>>> G(m);
    std::vector<double> diag(m, 0), rhs(m, 0);
    for (int k = 0; k < m; k++) {
        const int i = kept[k];
        for (auto &e : adj[i]) {
            if (idx[e.to] < 0) continue;
            const double g = 1.0 / e.r;
            G[k].push_back({idx[e.to], -g});
            diag[k] += g;
        }
        if (kinds[i] == DC_SOURCE) {
            const double r = std::max((double)pb[i], 1e-4);
            diag[k] += 1.0 / r;
            rhs[k] += pa[i] / r;
        } else if (kinds[i] == DC_LOAD) {
            diag[k] += std::max((double)pa[i], 0.0);
        }
        if (diag[k] <= 0) diag[k] = 1e-9;   // an isolated conductor: nothing drives it
    }
    // conjugate gradient with a Jacobi preconditioner (G is symmetric positive definite once a source or load
    // ties each component down; a component with neither floats at 0 V)
    std::vector<double> v(m, 0), r(m), z(m), p(m), q(m);
    auto mul = [&](const std::vector<double> &x, std::vector<double> &y) {
        for (int k = 0; k < m; k++) {
            double s = diag[k] * x[k];
            for (auto &e : G[k]) s += e.second * x[e.first];
            y[k] = s;
        }
    };
    for (int k = 0; k < m; k++) { r[k] = rhs[k]; z[k] = r[k] / diag[k]; p[k] = z[k]; }
    double rz = 0, rr0 = 0;
    for (int k = 0; k < m; k++) { rz += r[k] * z[k]; rr0 += r[k] * r[k]; }
    int iters = 0;
    const double tol2 = std::max(rr0, 1e-30) * 1e-16;
    for (; iters < 4000 && rz != 0; iters++) {
        mul(p, q);
        double pq = 0;
        for (int k = 0; k < m; k++) pq += p[k] * q[k];
        if (pq <= 0) break;
        const double alpha = rz / pq;
        double rr = 0;
        for (int k = 0; k < m; k++) { v[k] += alpha * p[k]; r[k] -= alpha * q[k]; rr += r[k] * r[k]; }
        if (rr < tol2) { iters++; break; }
        double rz2 = 0;
        for (int k = 0; k < m; k++) { z[k] = r[k] / diag[k]; rz2 += r[k] * z[k]; }
        const double beta = rz2 / rz;
        rz = rz2;
        for (int k = 0; k < m; k++) p[k] = z[k] + beta * p[k];
    }
    // results: voltage, current, power per node
    std::vector<int> comp(n);
    for (int i = 0; i < n; i++) comp[i] = find(parent, i);
    double delivered = 0;
    int count = 0;
    for (int i = 0; i < n; i++) if (comp[i] == i) count++;
    for (int i = 0; i < n; i++) {
        double volts = 0, amps = 0, watts = 0;
        if (removed[i]) {
            const double va = v[idx[chain_a[i]]], vb = v[idx[chain_b[i]]];
            amps = std::fabs(va - vb) / std::max(chain_rt[i], 1e-9);
            volts = va + (vb - va) * (chain_ra[i] / std::max(chain_rt[i], 1e-9));
        } else {
            const int k = idx[i];
            volts = v[k];
            if (kinds[i] == DC_SOURCE) {
                const double rr = std::max((double)pb[i], 1e-4);
                amps = (pa[i] - volts) / rr;     // positive: delivering; negative: taking (a battery charging)
                watts = volts * amps;
                if (watts > 0) delivered += watts;
            } else if (kinds[i] == DC_LOAD) {
                amps = volts * pa[i];
                watts = volts * amps;
            } else {
                for (auto &e : adj[i]) {
                    if (idx[e.to] < 0) continue;
                    amps = std::max(amps, std::fabs(volts - v[idx[e.to]]) / e.r);
                }
            }
        }
        out[i * 4] = (float)comp[i];
        out[i * 4 + 1] = (float)volts;
        out[i * 4 + 2] = (float)amps;
        out[i * 4 + 3] = (float)watts;
    }
    dc_stats_["count"] = count;
    dc_stats_["reduced_nodes"] = m;
    dc_stats_["iterations"] = iters;
    dc_stats_["delivered_watts"] = delivered;
    return out;
}
