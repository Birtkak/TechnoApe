// Headless checks of grid.h topology. Build + run: see README.
#include "grid.h"
#include <cassert>
#include <cstdio>
#include <set>
#include <tuple>

#define CHECK(x) do { if (!(x)) { std::printf("FAIL %s:%d %s\n", __FILE__, __LINE__, #x); return 1; } } while (0)

static int test_cube(int N) {
    CubeSphere g(N, Proj::EquiAngular);
    int64_t cells = 0, corner_cells = 0;
    double area = 0;
    for (int f = 0; f < 6; ++f) for (int i = 0; i < N; ++i) for (int j = 0; j < N; ++j) {
        QCell c{f, i, j}, nb[4];
        CHECK(g.decode(g.ipoint(c)) == c);
        g.neighbours(c, nb);
        for (int k = 0; k < 4; ++k) {
            CHECK(nb[k].face >= 0 && !(nb[k] == c));
            for (int m = 0; m < k; ++m) CHECK(N == 1 || !(nb[m] == nb[k]));
            QCell back[4];
            g.neighbours(nb[k], back);
            bool sym = false;
            for (auto& b : back) sym |= b == c;
            CHECK(sym);
        }
        // A cube corner cell: its two neighbours' "diagonal" coincides (3 cells around the vertex, not 4).
        bool ci = i == 0 || i == N - 1, cj = j == 0 || j == N - 1;
        corner_cells += ci && cj;
        auto q = g.corners(c);
        area += len(cross(q[1] - q[0], q[2] - q[0])) / 2 + len(cross(q[2] - q[0], q[3] - q[0])) / 2;
        ++cells;
    }
    CHECK(cells == g.count());
    if (N >= 2) CHECK(corner_cells == 24);
    if (N >= 32) CHECK(std::abs(area / (4 * M_PI) - 1) < 1e-3);
    // Walking straight along +s from any cell of face 0 loops round 4 faces back to the start after 4N steps.
    for (int j = 0; j < N; ++j) {
        QCell c{0, 0, j}, start = c;
        I3 d = CubeSphere::axis(0, 1);
        for (int s = 0; s < 4 * N; ++s) { c = g.step(c, d); if (s < 4 * N - 1) CHECK(!(c == start)); }
        CHECK(c == start);
    }
    return 0;
}

static int test_hex(int F) {
    HexSphere g(F);
    std::set<std::tuple<int, int, int>> seen;
    int64_t pent = 0, degree_sum = 0;
    for (int f = 0; f < 20; ++f) for (int a = 0; a <= F; ++a) for (int b = 0; a + b <= F; ++b) {
        HCell c = g.canon(f, a, b, F - a - b);
        CHECK(len(g.pos(c) - norm(g.pos({f, a, b}))) < 1e-12);  // canonical form is the same point
        if (!g.owned(f, c)) continue;
        CHECK(seen.insert({c.face, c.a, c.b}).second);
        HCell nb[6];
        int n = g.neighbours(c, nb);
        CHECK(n == 6 || (n == 5 && c.face < 0));
        pent += n == 5;
        degree_sum += n;
        for (int k = 0; k < n; ++k) {
            HCell back[6];
            int m = g.neighbours(nb[k], back);
            bool sym = false;
            for (int q = 0; q < m; ++q) sym |= back[q] == c;
            CHECK(sym);
            CHECK(len(g.pos(nb[k]) - g.pos(c)) < 2.0 / F);  // neighbours are close
        }
    }
    CHECK(int64_t(seen.size()) == g.count());
    CHECK(pent == 12);
    CHECK(degree_sum == 2 * 30LL * F * F);  // edges of the subdivided icosahedron
    return 0;
}

int main() {
    for (int N : {1, 2, 3, 8, 17, 64}) if (test_cube(N)) { std::printf("cube N=%d\n", N); return 1; }
    for (int F : {1, 2, 3, 5, 8, 21}) if (test_hex(F)) { std::printf("hex F=%d\n", F); return 1; }
    std::printf("all grid tests passed\n");
    return 0;
}
