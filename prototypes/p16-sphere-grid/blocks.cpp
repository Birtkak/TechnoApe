// Dumps the exposed faces of 0.5 m quad-prism blocks on the equi-angular cube-sphere around a point, with
// noise terrain, for the Godot look check (godot/look.gd). Geometric radial layers: layer l sits at R(1+B/R)^l.
// Output: one quad per line "mat nx ny nz x y z x y z x y z x y z" in metres relative to the view point.
//   mat: 0 grass top, 1 dirt side, 2 stone side/top, 3 sand top; +4 = top on a face seam, +8 = cube corner cell, +16 = on the marked row
#include "grid.h"
#include <cstdio>
#include <cstdlib>
#include <set>

static const double R = 300, B = 0.5;

static uint32_t hsh(int x, int y, int z) {
    uint32_t h = uint32_t(x) * 374761393u + uint32_t(y) * 668265263u + uint32_t(z) * 2147483647u;
    h = (h ^ (h >> 13)) * 1274126177u;
    return h ^ (h >> 16);
}
static double vnoise(V3 p) {  // value noise on the integer lattice of p
    int X = int(std::floor(p.x)), Y = int(std::floor(p.y)), Z = int(std::floor(p.z));
    double fx = p.x - X, fy = p.y - Y, fz = p.z - Z;
    auto s = [](double t) { return t * t * (3 - 2 * t); };
    fx = s(fx); fy = s(fy); fz = s(fz);
    double v[2][2][2];
    for (int i = 0; i < 2; ++i) for (int j = 0; j < 2; ++j) for (int k = 0; k < 2; ++k) v[i][j][k] = (hsh(X + i, Y + j, Z + k) & 0xffff) / 65535.0;
    auto L = [](double a, double b, double t) { return a + (b - a) * t; };
    return L(L(L(v[0][0][0], v[1][0][0], fx), L(v[0][1][0], v[1][1][0], fx), fy),
             L(L(v[0][0][1], v[1][0][1], fx), L(v[0][1][1], v[1][1][1], fx), fy), fz);
}
static double height_m(V3 u) {  // terrain height above R, metres, in [-4, 5]
    V3 p = u * R;
    return 6 * (vnoise(p * (1 / 24.0)) - 0.5) * 1.6 + 2 * (vnoise(p * (1 / 6.0)) - 0.5) + 0.5 * (vnoise(p * 0.7) - 0.5);
}
static int layer_of(double h) { return int(std::floor(std::log(1 + h / R) / std::log(1 + B / R))); }
static double radius_of(int l) { return R * std::pow(1 + B / R, l); }

int main(int argc, char** argv) {
    if (argc < 5) { std::fprintf(stderr, "blocks.exe out dx dy dz [radius_m] [row_j]\n"); return 1; }
    V3 dir = norm({std::atof(argv[2]), std::atof(argv[3]), std::atof(argv[4])});
    double rad = argc > 5 ? std::atof(argv[5]) : 40;
    int N = 864;  // 0.502 m blocks at R = 300 m, 27 chunks of 32
    CubeSphere g(N, Proj::EquiAngular);
    // row_j: mark a straight row of 80 blocks on face 0 at j = row_j blocks from the corner edge, running along +s
    // across the seam to face 2 (as measure.cpp's seam turn), and put the eye on it 6 m before the seam, looking along it.
    std::set<int64_t> row;
    V3 fwd{0, 0, 0};
    if (argc > 6) {
        int rj = std::atoi(argv[6]);
        QCell c{0, N - 41, rj};
        I3 d = CubeSphere::axis(0, 1);
        for (int s = 0; s < 80; ++s) { row.insert(g.index(c)); c = g.step(c, d); }
        dir = g.center({0, N - 13, rj});
        fwd = g.center({0, N - 1, rj}) - dir;
    }
    FILE* o = std::fopen(argv[1], "w");
    V3 eye = dir * (R + height_m(dir) + 1.7);
    std::fprintf(o, "eye %.6f %.6f %.6f\n", eye.x, eye.y, eye.z);
    if (argc > 6) std::fprintf(o, "fwd %.6f %.6f %.6f\n", fwd.x, fwd.y, fwd.z);
    double cosr = std::cos(rad / R);
    auto put = [&](int mat, V3 n, V3 a, V3 b, V3 c, V3 d) {
        std::fprintf(o, "%d %.4f %.4f %.4f", mat, n.x, n.y, n.z);
        for (V3 v : {a, b, c, d}) { V3 w = v - eye; std::fprintf(o, " %.4f %.4f %.4f", w.x, w.y, w.z); }
        std::fprintf(o, "\n");
    };
    int64_t quads = 0;
    for (int f = 0; f < 6; ++f) for (int i = 0; i < N; ++i) for (int j = 0; j < N; ++j) {
        QCell c{f, i, j};
        V3 ctr = g.center(c);
        if (dot(ctr, dir) < cosr) continue;
        int top = layer_of(height_m(ctr));
        auto q = g.corners(c);
        double rt = radius_of(top + 1);
        int topmat = height_m(ctr) < -1.0 ? 3 : (height_m(ctr) > 3.5 ? 2 : 0);
        // marker for the look check: +4 on a top along a face seam, +8 on the 3 cells of a cube corner
        int mark = (i == 0 || i == N - 1) + (j == 0 || j == N - 1);
        put(topmat + 4 * mark + 16 * row.count(g.index(c)), ctr, q[0] * rt, q[1] * rt, q[2] * rt, q[3] * rt);
        QCell nb[4];
        g.neighbours(c, nb);
        for (int k = 0; k < 4; ++k) {  // side faces down to the neighbour's top
            int nt = layer_of(height_m(g.center(nb[k])));
            // side k lies between corners k and k+1 (corner order matches neighbour order +s,+t,-s,-t)
            V3 a = q[(k + 1) % 4], b = q[(k + 2) % 4];
            for (int l = top; l > nt; --l) {
                double r0 = radius_of(l), r1 = radius_of(l + 1);
                int mat = (top - l) >= 3 || topmat == 2 ? 2 : 1;
                V3 n = norm(cross(b - a, ctr));
                put(mat, n, a * r0, a * r1, b * r1, b * r0);
                ++quads;
            }
        }
        ++quads;
    }
    std::fclose(o);
    std::fprintf(stderr, "%lld quads\n", (long long)quads);
}
