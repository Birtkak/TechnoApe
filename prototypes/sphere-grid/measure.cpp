// Measures the sphere-grid candidates: cell distortion at planet scale, neighbour lookup cost,
// straight rows (conveyors) across seams, machine footprints near corners, and dumps cells for the images.
// Writes out/grid_results.json and out/*.cells (read by render.py).
#include "grid.h"
#include <chrono>
#include <cstdio>
#include <map>
#include <set>
#include <queue>
#include <random>
#include <string>
#include <tuple>

static const double BLOCK = 0.5;  // m, the world block (ADR 0170)

struct Stats {
    double amin = 1e30, amax = 0, emin = 1e30, emax = 0, angmax = 0, aspmax = 0;
    int64_t n = 0, skew10 = 0, area10 = 0;
    void add(V3 c, const std::vector<V3>& p, double ideal, double R, double mean) {
        double area = 0, el = 1e30, eh = 0, dev = 0;
        size_t m = p.size();
        for (size_t k = 0; k < m; ++k) {
            V3 a = p[k], b = p[(k + 1) % m], z = p[(k + m - 1) % m];
            area += len(cross(a - c, b - c)) / 2;
            double e = len(b - a) * R;
            el = std::min(el, e); eh = std::max(eh, e);
            double ang = std::acos(std::clamp(dot(norm(z - a), norm(b - a)), -1.0, 1.0)) * 180 / M_PI;
            dev = std::max(dev, std::abs(ang - ideal));
        }
        area *= R * R;
        amin = std::min(amin, area / mean); amax = std::max(amax, area / mean);
        emin = std::min(emin, el); emax = std::max(emax, eh);
        angmax = std::max(angmax, dev); aspmax = std::max(aspmax, eh / el);
        ++n; skew10 += dev > 10; area10 += std::abs(area / mean - 1) > 0.1;
    }
    std::string json() const {
        char b[512];
        std::snprintf(b, sizeof b,
            "{\"area_min_rel\":%.4f,\"area_max_rel\":%.4f,\"area_ratio\":%.4f,\"edge_min_m\":%.4f,\"edge_max_m\":%.4f,"
            "\"max_angle_dev_deg\":%.2f,\"max_edge_ratio_in_cell\":%.4f,\"frac_angle_dev_gt10\":%.5f,\"frac_area_dev_gt10pct\":%.5f}",
            amin, amax, amax / amin, emin, emax, angmax, aspmax, double(skew10) / n, double(area10) / n);
        return b;
    }
};

static int cube_n(double R) { return int(std::lround(R * std::sqrt(4 * M_PI / (6 * BLOCK * BLOCK)))); }
static int hex_f(double R) { return int(std::lround(std::sqrt((4 * M_PI * R * R / (BLOCK * BLOCK) - 2) / 10))); }

static std::vector<V3> hex_poly(const HexSphere& g, HCell c) {
    HCell nb[6];
    int n = g.neighbours(c, nb);
    V3 p = g.pos(c);
    std::vector<V3> q;
    for (int k = 0; k < n; ++k) q.push_back(g.pos(nb[k]));
    sort_ccw(p, q);
    std::vector<V3> out;
    for (int k = 0; k < n; ++k) out.push_back(norm(p + q[k] + q[(k + 1) % n]));
    return out;
}

// Signed geodesic turn at cur (degrees), seen from outside.
static double turn(V3 prev, V3 cur, V3 next) {
    V3 a = cur - prev, b = next - cur;
    a = a - cur * dot(a, cur); b = b - cur * dot(b, cur);
    return std::atan2(dot(cross(a, b), cur), dot(a, b)) * 180 / M_PI;
}

template <class F> static double ns_per(F&& f, int64_t n) {
    auto t0 = std::chrono::steady_clock::now();
    f();
    return std::chrono::duration<double, std::nano>(std::chrono::steady_clock::now() - t0).count() / n;
}

int main() {
    std::string J = "{";
    // ------------------------------------------------ distortion at planet scale
    for (double R : {300.0, 1000.0}) {
        int N = cube_n(R), F = hex_f(R);
        char key[64];
        std::snprintf(key, sizeof key, "\"R%d\":{", int(R));
        J += key;
        J += "\"cube_N\":" + std::to_string(N) + ",\"cube_cells\":" + std::to_string(6LL * N * N) +
             ",\"hex_F\":" + std::to_string(F) + ",\"hex_cells\":" + std::to_string(10LL * F * F + 2) + ",";
        for (auto [proj, name] : {std::pair{Proj::Gnomonic, "cube_gnomonic"}, {Proj::EquiAngular, "cube_equiangular"},
                                  {Proj::Nowicki, "cube_nowicki"}}) {
            CubeSphere g(N, proj);
            Stats s;
            double mean = 4 * M_PI * R * R / g.count();
            for (int i = 0; i < N; ++i) for (int j = 0; j < N; ++j) {
                QCell c{0, i, j};
                auto q = g.corners(c);
                s.add(g.center(c), {q.begin(), q.end()}, 90, R, mean);
            }
            J += std::string("\"") + name + "\":" + s.json() + ",";
            std::fprintf(stderr, "R=%g %s done\n", R, name);
        }
        HexSphere h(F);
        Stats sh, sp;
        double mean = 4 * M_PI * R * R / h.count();
        for (int a = 0; a <= F; ++a) for (int b = 0; a + b <= F; ++b) {
            HCell c = h.canon(0, a, b, F - a - b);
            auto p = hex_poly(h, c);
            (p.size() == 5 ? sp : sh).add(h.pos(c), p, p.size() == 5 ? 108 : 120, R, mean);
        }
        J += "\"hex_geodesic\":" + sh.json() + ",\"hex_pentagon\":" + sp.json() + "},";
        std::fprintf(stderr, "R=%g hex done\n", R);
    }

    // ------------------------------------------------ neighbour lookup cost (all lateral neighbours of a random cell)
    {
        const int64_t M = 4'000'000;
        std::mt19937_64 rng(7);
        int N = cube_n(1000), F = hex_f(1000);
        CubeSphere g(N, Proj::EquiAngular);
        HexSphere h(F);
        std::vector<QCell> qc(M);
        std::vector<HCell> hc(M);
        for (auto& c : qc) c = {int(rng() % 6), int(rng() % N), int(rng() % N)};
        for (auto& c : hc) {
            int f = int(rng() % 20), a = int(rng() % (F + 1)), b = int(rng() % (F + 1 - a));
            c = h.canon(f, a, b, F - a - b);
        }
        // Edge cells only: the slow paths.
        std::vector<QCell> qe(M);
        std::vector<HCell> he(M);
        for (auto& c : qe) c = {int(rng() % 6), 0, int(rng() % N)};
        for (auto& c : he) { int f = int(rng() % 20), a = int(rng() % (F + 1)); c = h.canon(f, a, 0, F - a); }
        int64_t acc = 0;
        double cube_ns = ns_per([&] { for (auto& c : qc) { QCell n[4]; g.neighbours(c, n); for (auto& x : n) acc += g.index(x); } }, M);
        double cube_edge_ns = ns_per([&] { for (auto& c : qe) { QCell n[4]; g.neighbours(c, n); for (auto& x : n) acc += g.index(x); } }, M);
        double hex_ns = ns_per([&] { for (auto& c : hc) { HCell n[6]; int k = h.neighbours(c, n); for (int q = 0; q < k; ++q) acc += n[q].a * 7 + n[q].b + n[q].face; } }, M);
        double hex_edge_ns = ns_per([&] { for (auto& c : he) { HCell n[6]; int k = h.neighbours(c, n); for (int q = 0; q < k; ++q) acc += n[q].a * 7 + n[q].b + n[q].face; } }, M);
        // Cartesian baseline: 6 neighbours of a flat 3D array index with bounds checks.
        const int W = 1024;
        std::vector<std::array<int, 3>> xc(M);
        for (auto& c : xc) c = {int(rng() % W), int(rng() % W), int(rng() % W)};
        double cart_ns = ns_per([&] {
            for (auto& c : xc) {
                int64_t i = (int64_t(c[0]) * W + c[1]) * W + c[2];
                if (c[0] > 0) acc += i - W * W; if (c[0] < W - 1) acc += i + W * W;
                if (c[1] > 0) acc += i - W;     if (c[1] < W - 1) acc += i + W;
                if (c[2] > 0) acc += i - 1;     if (c[2] < W - 1) acc += i + 1;
            }
        }, M);
        char b[512];
        std::snprintf(b, sizeof b,
            "\"neighbour_ns\":{\"note\":\"all lateral neighbours of one cell, random cells, R=1000 m, -O2, one core; checksum %lld\","
            "\"cartesian_6\":%.2f,\"cube_4\":%.2f,\"cube_4_edge_cells\":%.2f,\"hex_6\":%.2f,\"hex_6_edge_cells\":%.2f},",
            (long long)(acc & 0xff), cart_ns, cube_ns, cube_edge_ns, hex_ns, hex_edge_ns);
        J += b;
    }

    // ------------------------------------------------ straight rows (conveyors) across a seam, R = 300 m
    {
        J += "\"row_kink_deg\":{\"note\":\"total turn of a straight row of blocks across a seam (sum over the 12 blocks around it), by distance (blocks) of the crossing from the nearest corner/pentagon; interior = max turn per block inside a face\",";
        int N = cube_n(300);
        std::vector<int> dists{0, 1, 2, 4, 8, 16, 32, 64, 128, N / 2};
        for (auto [proj, name] : {std::pair{Proj::Gnomonic, "cube_gnomonic"}, {Proj::EquiAngular, "cube_equiangular"},
                                  {Proj::Nowicki, "cube_nowicki"}}) {
            CubeSphere g(N, proj);
            J += std::string("\"") + name + "\":{";
            double interior = 0;
            for (int j : dists) {
                QCell c{0, N - 6, j};
                I3 d = CubeSphere::axis(0, 1);
                std::vector<V3> pts;
                for (int s = 0; s < 12; ++s) { pts.push_back(g.center(c)); c = g.step(c, d); }
                double mx = 0;
                for (size_t k = 1; k + 1 < pts.size(); ++k) mx += turn(pts[k - 1], pts[k], pts[k + 1]);
            mx = std::abs(mx);
                J += "\"" + std::to_string(j) + "\":" + std::to_string(mx) + ",";
            }
            for (int j : {N / 7, N / 3}) for (int i = 1; i + 1 < N; i += 13)
                interior = std::max(interior, std::abs(turn(g.center({0, i - 1, j}), g.center({0, i, j}), g.center({0, i + 1, j}))));
            J += "\"interior\":" + std::to_string(interior) + "},";
        }
        int F = hex_f(300);
        HexSphere h(F);
        J += "\"hex_geodesic\":{";
        for (int a0 : {1, 2, 4, 8, 16, 32, 64, 128, F / 2}) {
            // Row along lattice step (0,+1,-1) keeps w0 = a0 and crosses the edge w2 = 0 at a0 blocks from corner 1.
            HCell prev = {0, a0, F - a0 - 7}, cur = h.canon(0, a0, F - a0 - 6, 6);
            std::vector<V3> pts{h.pos(prev), h.pos(cur)};
            for (int s = 0; s < 12; ++s) {
                HCell nb[6];
                int n = h.neighbours(cur, nb);
                V3 back = norm(h.pos(prev) - h.pos(cur));
                int best = 0;
                double bd = 2;
                for (int k = 0; k < n; ++k) { double d = dot(norm(h.pos(nb[k]) - h.pos(cur)), back); if (d < bd) { bd = d; best = k; } }
                prev = cur; cur = nb[best];
                pts.push_back(h.pos(cur));
            }
            double mx = 0;
            for (size_t k = 1; k + 1 < pts.size(); ++k) mx += turn(pts[k - 1], pts[k], pts[k + 1]);
            mx = std::abs(mx);
            J += "\"" + std::to_string(a0) + "\":" + std::to_string(mx) + ",";
        }
        J += "\"interior\":0}},";
    }

    // ------------------------------------------------ machine footprints near corners / pentagons
    {
        const int N = 64;
        CubeSphere g(N, Proj::EquiAngular);
        J += "\"footprint\":{\"note\":\"origins where a W x W (cube) or radius-k (hex) footprint does not stamp as a regular patch, and the max distance (blocks) of such an origin from the nearest corner/pentagon\",";
        for (int W : {2, 4, 8, 16}) {
            int64_t bad = 0;
            int maxd = 0;
            for (int f = 0; f < 6; ++f) for (int i = 0; i < N; ++i) for (int j = 0; j < N; ++j) {
                QCell o{f, i, j};
                auto stamp = [&](int first, int second) {
                    std::set<std::tuple<int, int, int>> s;
                    I3 d1 = CubeSphere::axis(f, first), d2 = CubeSphere::axis(f, second);
                    QCell r = o;
                    for (int u = 0; u < W; ++u) {
                        QCell c = r;
                        I3 dd = d2;
                        for (int v = 0; v < W; ++v) { s.insert({c.face, c.i, c.j}); c = g.step(c, dd); }
                        r = g.step(r, d1);
                    }
                    return s;
                };
                auto A = stamp(1, 2), B = stamp(2, 1);
                if (A != B || int64_t(A.size()) != int64_t(W) * W) {
                    ++bad;
                    int d = std::min({std::max(i, j), std::max(N - 1 - i, j), std::max(i, N - 1 - j), std::max(N - 1 - i, N - 1 - j)});
                    maxd = std::max(maxd, d);
                }
            }
            J += "\"cube_W" + std::to_string(W) + "\":{\"bad_origins\":" + std::to_string(bad) + ",\"max_dist_from_corner\":" + std::to_string(maxd) + "},";
        }
        const int F = 32;
        HexSphere h(F);
        std::map<std::tuple<int, int, int>, int> id;
        std::vector<HCell> cells;
        for (int f = 0; f < 20; ++f) for (int a = 0; a <= F; ++a) for (int b = 0; a + b <= F; ++b) {
            HCell c = h.canon(f, a, b, F - a - b);
            if (h.owned(f, c)) { id[{c.face, c.a, c.b}] = int(cells.size()); cells.push_back(c); }
        }
        auto nbr = [&](int k, std::vector<int>& out) {
            HCell nb[6]; int n = h.neighbours(cells[k], nb); out.clear();
            for (int q = 0; q < n; ++q) out.push_back(id[{nb[q].face, nb[q].a, nb[q].b}]);
        };
        std::vector<int> dist(cells.size(), -1), tmp;
        std::queue<int> qu;
        for (size_t k = 0; k < cells.size(); ++k) if (cells[k].face < 0) { dist[k] = 0; qu.push(int(k)); }
        while (!qu.empty()) { int k = qu.front(); qu.pop(); nbr(k, tmp); for (int m : tmp) if (dist[m] < 0) { dist[m] = dist[k] + 1; qu.push(m); } }
        for (int R : {1, 2, 4, 8}) {
            int64_t bad = 0; int maxd = 0;
            for (size_t o = 0; o < cells.size(); ++o) {
                std::map<int, int> seen{{int(o), 0}};
                std::queue<int> q2; q2.push(int(o));
                while (!q2.empty()) { int k = q2.front(); q2.pop(); if (seen[k] == R) continue; nbr(k, tmp); for (int m : tmp) if (!seen.count(m)) { seen[m] = seen[k] + 1; q2.push(m); } }
                if (int64_t(seen.size()) != 3LL * R * (R + 1) + 1) { ++bad; maxd = std::max(maxd, dist[o]); }
            }
            J += "\"hex_k" + std::to_string(R) + "\":{\"bad_origins\":" + std::to_string(bad) + ",\"max_dist_from_pentagon\":" + std::to_string(maxd) + "},";
        }
        J.back() = '}';
        J += ",";
    }
    J.back() = '}';
    FILE* f = std::fopen("out/grid_results.json", "w");
    std::fputs(J.c_str(), f);
    std::fclose(f);

    // ------------------------------------------------ cell dumps for render.py: "value x y z x y z ..." per line
    auto dump_cube = [](const char* path, int N, Proj p, V3 dir, double cosr) {
        CubeSphere g(N, p);
        FILE* o = std::fopen(path, "w");
        double mean = 4 * M_PI / g.count();
        for (int f = 0; f < 6; ++f) for (int i = 0; i < N; ++i) for (int j = 0; j < N; ++j) {
            QCell c{f, i, j};
            if (dot(g.center(c), dir) < cosr) continue;
            auto q = g.corners(c);
            double a = len(cross(q[1] - q[0], q[2] - q[0])) / 2 + len(cross(q[2] - q[0], q[3] - q[0])) / 2;
            std::fprintf(o, "%.5f", a / mean);
            for (auto& v : q) std::fprintf(o, " %.9f %.9f %.9f", v.x, v.y, v.z);
            std::fprintf(o, "\n");
        }
        std::fclose(o);
    };
    auto dump_hex = [](const char* path, int F, V3 dir, double cosr) {
        HexSphere h(F);
        FILE* o = std::fopen(path, "w");
        double mean = 4 * M_PI / h.count();
        for (int f = 0; f < 20; ++f) for (int a = 0; a <= F; ++a) for (int b = 0; a + b <= F; ++b) {
            HCell c = h.canon(f, a, b, F - a - b);
            if (!h.owned(f, c) || dot(h.pos(c), dir) < cosr) continue;
            auto p = hex_poly(h, c);
            V3 ctr = h.pos(c);
            double ar = 0;
            for (size_t k = 0; k < p.size(); ++k) ar += len(cross(p[k] - ctr, p[(k + 1) % p.size()] - ctr)) / 2;
            std::fprintf(o, "%.5f", ar / mean);
            for (auto& v : p) std::fprintf(o, " %.9f %.9f %.9f", v.x, v.y, v.z);
            std::fprintf(o, "\n");
        }
        std::fclose(o);
    };
    const double R = 300, view = std::cos(9.0 / R);  // 9 m around the view point
    int N = cube_n(R), F = hex_f(R);
    HexSphere h0(1);
    V3 patch = norm({1, 0.35, 0.2});
    dump_cube("out/cube_patch.cells", N, Proj::EquiAngular, patch, view);
    dump_cube("out/cube_seam.cells", N, Proj::EquiAngular, norm({1, 1, 0}), view);
    dump_cube("out/cube_corner.cells", N, Proj::EquiAngular, norm({1, 1, 1}), view);
    dump_cube("out/cube_orbit_gnomonic.cells", 12, Proj::Gnomonic, {1, 0, 0}, -2);
    dump_cube("out/cube_orbit_equiangular.cells", 12, Proj::EquiAngular, {1, 0, 0}, -2);
    dump_cube("out/cube_orbit_nowicki.cells", 12, Proj::Nowicki, {1, 0, 0}, -2);
    dump_hex("out/hex_patch.cells", F, norm(h0.V[h0.T[0][0]] * 0.5 + h0.V[h0.T[0][1]] * 0.3 + h0.V[h0.T[0][2]] * 0.2), view);
    dump_hex("out/hex_seam.cells", F, norm(h0.V[h0.T[0][0]] + h0.V[h0.T[0][1]]), view);
    dump_hex("out/hex_pentagon.cells", F, h0.V[h0.T[0][0]], view);
    dump_hex("out/hex_orbit.cells", 8, {1, 0, 0}, -2);
    std::fprintf(stderr, "done\n");
    return 0;
}
