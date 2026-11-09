// Relaxed cube-sphere: can straight rows cross a seam without the kink of the equi-angular grid?
// Laplacian smoothing of the cube-sphere vertex lattice on the sphere converges to the discrete harmonic map,
// which in the limit is the conformal cubed sphere (Rancic, Purser & Mesinger 1996): grid lines are orthogonal
// everywhere, so a row crosses a seam at a right angle on both sides and does not kink there. The 720 deg of
// curvature then collects at the 8 corners, where cells shrink. The blend equi-angular + w * (harmonic - equi-angular)
// trades one against the other (warp.h).
// A second family keeps the corners equi-angular: w fades to 0 within L blocks of each corner (warp.h corner_L).
//   smooth.exe [K=192] [reuse]  ->  out/harmonic.tab, results_relaxed.json   (reuse: load the table, skip relaxing)
#include "warp.h"
#include <chrono>
#include <cstdlib>
#include <string>

static const double R = 300;

static double turn_deg(V3 a, V3 b, V3 c) {  // signed turn at b, in b's tangent plane
    V3 n = norm(b);
    auto tg = [&](V3 d) { return d - n * dot(n, d); };
    V3 d1 = tg(b - a), d2 = tg(c - b);
    return std::atan2(dot(cross(d1, d2), n), dot(d1, d2)) * 180 / M_PI;
}

int main(int argc, char** argv) {
    int K = argc > 1 ? std::atoi(argv[1]) : 192;
    Warp wp;
    int it = 0;
    if (argc > 2 && std::string(argv[2]) == "reuse") {
        if (!wp.load("out/harmonic.tab") || wp.K != K) { std::fprintf(stderr, "no out/harmonic.tab for K=%d\n", K); return 1; }
    } else {
    // Vertex lattice: integer points of the cube of half-size K with step 2, so face vertex (ia, ib) sits at
    // n*K + s*(2ia-K) + t*(2ib-K). The lattice graph is bipartite, so Jacobi is damped (OMEGA < 1).
    const int S = 2 * K + 1;
    std::vector<int> idx(size_t(S) * S * S, -1);
    std::vector<I3> P;
    auto key = [&](const I3& p) { return (size_t(p[0] + K) * S + (p[1] + K)) * S + (p[2] + K); };
    auto inside = [&](const I3& p) { for (int k = 0; k < 3; ++k) if (p[k] < -K || p[k] > K) return false; return true; };
    for (int f = 0; f < 6; ++f) for (int ia = 0; ia <= K; ++ia) for (int ib = 0; ib <= K; ++ib) {
        I3 p;
        for (int k = 0; k < 3; ++k) p[k] = CubeSphere::AX[f][0][k] * K + CubeSphere::AX[f][1][k] * (2 * ia - K) + CubeSphere::AX[f][2][k] * (2 * ib - K);
        if (idx[key(p)] < 0) { idx[key(p)] = int(P.size()); P.push_back(p); }
    }
    const int V = int(P.size());
    std::vector<std::array<int, 4>> nb(V);
    std::vector<int> deg(V, 0);
    for (int v = 0; v < V; ++v) for (int k = 0; k < 3; ++k) for (int s : {-2, 2}) {
        I3 q = P[v];
        q[k] += s;
        if (inside(q) && idx[key(q)] >= 0) nb[v][deg[v]++] = idx[key(q)];
    }
    int deg3 = 0;
    for (int v = 0; v < V; ++v) deg3 += deg[v] == 3;
    std::vector<V3> ea(V), x(V), nx(V);
    for (int v = 0; v < V; ++v)
        ea[v] = norm({std::tan(M_PI / 4 * P[v][0] / K), std::tan(M_PI / 4 * P[v][1] / K), std::tan(M_PI / 4 * P[v][2] / K)});
    x = ea;
    const double OMEGA = 0.95;
    auto t0 = std::chrono::steady_clock::now();
    double change = 1;
    for (; it < 400000 && change > 1e-13; ++it) {
        change = 0;
#pragma omp parallel for reduction(max : change)
        for (int v = 0; v < V; ++v) {
            V3 m{};
            for (int k = 0; k < deg[v]; ++k) m = m + x[nb[v][k]];
            V3 y = norm(x[v] + (norm(m) - x[v]) * OMEGA);
            change = std::max(change, len(y - x[v]));
            nx[v] = y;
        }
        std::swap(x, nx);
        if (it % 20000 == 0) std::fprintf(stderr, "it %d change %.3g\n", it, change);
    }
    double secs = std::chrono::duration<double>(std::chrono::steady_clock::now() - t0).count();
    std::fprintf(stderr, "K=%d vertices=%d corners=%d iterations=%d last change %.3g, %.1f s\n", K, V, deg3, it, change, secs);

    wp.K = K;
    wp.D.resize(size_t(6) * (K + 1) * (K + 1));
    for (int f = 0; f < 6; ++f) for (int ia = 0; ia <= K; ++ia) for (int ib = 0; ib <= K; ++ib) {
        I3 p;
        for (int k = 0; k < 3; ++k) p[k] = CubeSphere::AX[f][0][k] * K + CubeSphere::AX[f][1][k] * (2 * ia - K) + CubeSphere::AX[f][2][k] * (2 * ib - K);
        int v = idx[key(p)];
        wp.at(f, ia, ib) = x[v] - ea[v];
    }
    wp.save("out/harmonic.tab");
    }

    // Measure the blend family on the game grid: N = 864 at R = 300 m (0.502 m blocks).
    const int N = 864;
    std::string J = "{\"K\":" + std::to_string(K) + ",\"iterations\":" + std::to_string(it) + ",\"N\":864,\"R\":300,\"blend\":{";
    const double BLOCK_RAD = M_PI / 2 / N;  // one block at a face centre
    struct Variant { double w, Lb; };
    bool firstv = true;
    for (Variant vr : std::vector<Variant>{{0, 0}, {0.5, 0}, {1, 0}, {1, 32}, {1, 64}, {1, 128}, {1, 256}}) {
        double w = vr.w;
        wp.w = w;
        wp.corner_L = vr.Lb * BLOCK_RAD;
        CubeSphere g(N, Proj::EquiAngular);
        wp.attach(g);
        // cell shape over face 0 (all faces are alike)
        double amin = 1e9, amax = 0, asum = 0, emin = 1e9, emax = 0, worst = 0;
        std::vector<double> areas;
        areas.reserve(size_t(N) * N);
        for (int i = 0; i < N; ++i) for (int j = 0; j < N; ++j) {
            auto q = g.corners({0, i, j});
            for (auto& c : q) c = c * R;
            double a = 0.5 * len(cross(q[2] - q[0], q[3] - q[1]));
            areas.push_back(a);
            amin = std::min(amin, a); amax = std::max(amax, a); asum += a;
            for (int k = 0; k < 4; ++k) {
                V3 e1 = q[(k + 1) % 4] - q[k], e0 = q[(k + 3) % 4] - q[k];
                emin = std::min(emin, len(e1)); emax = std::max(emax, len(e1));
                worst = std::max(worst, std::abs(std::acos(dot(e0, e1) / (len(e0) * len(e1))) * 180 / M_PI - 90));
            }
        }
        double mean = asum / areas.size();
        int off10 = 0;
        for (double a : areas) off10 += std::abs(a / mean - 1) > 0.1;
        auto edge = [&](int i, int j) { auto q = g.corners({0, i, j}); return len(q[1] - q[0]) * R; };
        // How far from a corner the blocks are small: walk the face diagonal out from the corner.
        auto reach = [&](double m) { int k = 0; while (k < N / 2 && edge(k, k) < m) ++k; return k * std::sqrt(2.0) * 0.5; };
        double r40 = reach(0.40), r45 = reach(0.45);
        double fmin = 1e9, fmax = 0;  // edge range more than 64 blocks from any corner
        for (int i = 0; i < N; i += 4) for (int j = 0; j < N; j += 4) {
            int di = std::min(i, N - 1 - i), dj = std::min(j, N - 1 - j);
            if (di * di + dj * dj < 64 * 64) continue;
            double e = edge(i, j);
            fmin = std::min(fmin, e); fmax = std::max(fmax, e);
        }
        std::fprintf(stderr, "   blocks < 0.40 m within %.0f m of a corner, < 0.45 m within %.0f m; edges %.3f-%.3f m beyond 64 blocks\n", r40, r45, fmin, fmax);
        char buf[512];
        std::snprintf(buf, sizeof buf, "%s\"w%.2f_L%d\":{\"area_max_min\":%.3f,\"edge_min_m\":%.3f,\"edge_max_m\":%.3f,\"worst_angle_off_deg\":%.1f,"
                      "\"cells_10pct_off_area\":%.3f,\"corner_block_m\":%.3f,\"block_8_from_corner_m\":%.3f,\"block_64_from_corner_m\":%.3f,"
                      "\"small_blocks_reach_m\":{\"lt_0.40\":%.0f,\"lt_0.45\":%.0f},\"edge_range_beyond_64_blocks_m\":[%.3f,%.3f],\"rows\":{",
                      firstv ? "" : ",", w, int(vr.Lb), amax / amin, emin, emax, worst, double(off10) / areas.size(), edge(0, 0), edge(16, 16), edge(128, 128), r40, r45, fmin, fmax);
        J += buf;
        firstv = false;
        std::fprintf(stderr, "w=%.2f L=%d area max/min %.2f, edges %.3f-%.3f m, worst angle %.1f deg, corner block %.3f m\n", w, int(vr.Lb), amax / amin, emin, emax, worst, edge(0, 0));
        // Rows of face 0 at j blocks from the corner edge, running along +s across the seam onto face 2.
        bool first = true;
        for (int rj : {16, 64, 128, 192, 256, 320, 432}) {
            std::vector<V3> c;
            QCell cell{0, N - 200, rj};
            I3 d = CubeSphere::axis(0, 1);
            for (int s = 0; s < 400; ++s) { c.push_back(g.center(cell)); cell = g.step(cell, d); }
            double kink = 0, win = 0, total = 0;  // crossing sits between blocks 199 and 200
            for (int k = 1; k + 1 < 400; ++k) {
                double t = turn_deg(c[k - 1], c[k], c[k + 1]);
                kink = std::max(kink, std::abs(t));
                total += t;
                if (k >= 194 && k <= 205) win += t;
            }
            std::snprintf(buf, sizeof buf, "%s\"%d\":{\"max_turn_per_block_deg\":%.2f,\"turn_12_blocks_deg\":%.1f,\"turn_400_blocks_deg\":%.1f}",
                          first ? "" : ",", rj, kink, std::abs(win), std::abs(total));
            J += buf;
            first = false;
            std::fprintf(stderr, "   row %3d from corner: max turn per block %.2f deg, 12 blocks %.1f, 400 blocks %.1f\n", rj, kink, std::abs(win), std::abs(total));
        }
        J += "}}";
    }
    J += "}}\n";
    FILE* o = std::fopen("results_relaxed.json", "w");
    std::fputs(J.c_str(), o);
    std::fclose(o);
}
