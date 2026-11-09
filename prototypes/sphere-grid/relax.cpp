// Optimised hex layout: Lloyd relaxation (each cell centre -> centroid of its polygon) on the geodesic grid,
// then the same distortion stats as measure.cpp. Writes out/hex_relaxed.json.
#include "grid.h"
#include <cstdio>
#include <unordered_map>

static int64_t key(HCell c) { return (int64_t(c.face + 1) << 40) | (int64_t(c.a) << 20) | c.b; }

int main(int argc, char** argv) {
    int F = argc > 1 ? std::atoi(argv[1]) : 300, iters = argc > 2 ? std::atoi(argv[2]) : 100, mode = argc > 3 ? std::atoi(argv[3]) : 0;
    const double R = 300;
    HexSphere h(F);
    std::vector<HCell> cells;
    std::unordered_map<int64_t, int> id;
    cells.reserve(h.count());
    for (int f = 0; f < 20; ++f) for (int a = 0; a <= F; ++a) for (int b = 0; a + b <= F; ++b) {
        HCell c = h.canon(f, a, b, F - a - b);
        if (h.owned(f, c)) { id[key(c)] = int(cells.size()); cells.push_back(c); }
    }
    const int n = int(cells.size());
    std::vector<int> nb(size_t(n) * 6, -1);
    std::vector<V3> P(n);
    for (int k = 0; k < n; ++k) {
        HCell t[6];
        int m = h.neighbours(cells[k], t);
        P[k] = h.pos(cells[k]);
        std::vector<V3> q;
        for (int j = 0; j < m; ++j) q.push_back(h.pos(t[j]));
        sort_ccw(P[k], q);  // keep neighbours in ccw order so polygons are built the same way every iteration
        for (int j = 0; j < m; ++j) for (int i = 0; i < m; ++i) if (len(q[j] - h.pos(t[i])) < 1e-12) nb[size_t(k) * 6 + j] = id[key(t[i])];
    }
    auto stats = [&](const char* name) {
        double mean = 4 * M_PI * R * R / n, amin = 1e30, amax = 0, emin = 1e30, emax = 0, angmax = 0;
        int64_t skew10 = 0, area10 = 0, hexes = 0;
        for (int k = 0; k < n; ++k) {
            int m = nb[size_t(k) * 6 + 5] < 0 ? 5 : 6;
            if (m == 5) continue;
            ++hexes;
            V3 c = P[k];
            std::vector<V3> p;
            for (int j = 0; j < m; ++j) p.push_back(norm(c + P[nb[size_t(k) * 6 + j]] + P[nb[size_t(k) * 6 + (j + 1) % m]]));
            double area = 0, dev = 0;
            for (int j = 0; j < m; ++j) {
                V3 a = p[j], b = p[(j + 1) % m], z = p[(j + m - 1) % m];
                area += len(cross(a - c, b - c)) / 2;
                double e = len(b - a) * R;
                emin = std::min(emin, e); emax = std::max(emax, e);
                dev = std::max(dev, std::abs(std::acos(std::clamp(dot(norm(z - a), norm(b - a)), -1.0, 1.0)) * 180 / M_PI - 120));
            }
            area *= R * R;
            amin = std::min(amin, area / mean); amax = std::max(amax, area / mean);
            angmax = std::max(angmax, dev); skew10 += dev > 10; area10 += std::abs(area / mean - 1) > 0.1;
        }
        std::printf("\"%s\":{\"area_min_rel\":%.4f,\"area_max_rel\":%.4f,\"area_ratio\":%.4f,\"edge_min_m\":%.4f,\"edge_max_m\":%.4f,"
                    "\"max_angle_dev_deg\":%.2f,\"frac_angle_dev_gt10\":%.5f,\"frac_area_dev_gt10pct\":%.5f}",
                    name, amin, amax, amax / amin, emin, emax, angmax, double(skew10) / hexes, double(area10) / hexes);
    };
    std::printf("{\"F\":%d,\"iterations\":%d,\"mode\":\"%s\",", F, iters, mode ? "spring" : "lloyd");
    stats("before");
    std::printf(",");
    std::vector<V3> Q(n);
    for (int it = 0; it < iters; ++it) {
        for (int k = 0; k < n; ++k) {
            int m = nb[size_t(k) * 6 + 5] < 0 ? 5 : 6;
            V3 c = P[k], acc{};
            double tot = 0;
            for (int j = 0; j < m; ++j) {  // centroid of the polygon, weighted by triangle area
                V3 a = norm(c + P[nb[size_t(k) * 6 + j]] + P[nb[size_t(k) * 6 + (j + 1) % m]]);
                V3 b = norm(c + P[nb[size_t(k) * 6 + (j + 1) % m]] + P[nb[size_t(k) * 6 + (j + 2) % m]]);
                double w = len(cross(a - c, b - c));
                acc = acc + (c + a + b) * (w / 3); tot += w;
            }
            if (mode == 1) {  // spring: toward the mean of the neighbours (equalises edge lengths)
                V3 mn{};
                for (int j = 0; j < m; ++j) mn = mn + P[nb[size_t(k) * 6 + j]];
                Q[k] = norm(c + (norm(mn) - c) * 0.5);
            } else Q[k] = tot > 0 ? norm(acc * (1 / tot)) : c;
        }
        P.swap(Q);
    }
    stats("after");
    std::printf("}\n");
}
