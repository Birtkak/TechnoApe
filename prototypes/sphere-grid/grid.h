// p16 sphere grids: engine-independent topology + geometry for the planet grid candidates.
// Header only, no dependencies. Unit sphere; multiply by the radius for metres.
//   CubeSphere: 6 faces of N x N quads (gnomonic, equi-angular or Nowicki warp).
//   HexSphere:  geodesic grid, dual of an icosahedron subdivided F times (10F^2+2 cells, 12 pentagons).
// Option (c) rounded cube and (d) Cartesian + radial gravity are plain 3D arrays and need no code here.
#pragma once
#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif
#include <array>
#include <cmath>
#include <cstdint>
#include <vector>
#include <algorithm>
#include <functional>

struct V3 { double x = 0, y = 0, z = 0; };
inline V3 operator+(V3 a, V3 b) { return {a.x + b.x, a.y + b.y, a.z + b.z}; }
inline V3 operator-(V3 a, V3 b) { return {a.x - b.x, a.y - b.y, a.z - b.z}; }
inline V3 operator*(V3 a, double s) { return {a.x * s, a.y * s, a.z * s}; }
inline double dot(V3 a, V3 b) { return a.x * b.x + a.y * b.y + a.z * b.z; }
inline V3 cross(V3 a, V3 b) { return {a.y * b.z - a.z * b.y, a.z * b.x - a.x * b.z, a.x * b.y - a.y * b.x}; }
inline double len(V3 a) { return std::sqrt(dot(a, a)); }
inline V3 norm(V3 a) { return a * (1.0 / len(a)); }

using I3 = std::array<int, 3>;
inline int idot(const I3& a, const I3& b) { return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]; }

// ---------------------------------------------------------------- cube-sphere
enum class Proj { Gnomonic, EquiAngular, Nowicki };

struct QCell {
    int face, i, j;
    bool operator==(const QCell& o) const { return face == o.face && i == o.i && j == o.j; }
};

struct CubeSphere {
    int N;
    Proj proj;
    // [face][n, s, t]: face normal and the two in-face axes, s x t = n.
    static constexpr int AX[6][3][3] = {
        {{1, 0, 0}, {0, 1, 0}, {0, 0, 1}},  {{-1, 0, 0}, {0, 0, 1}, {0, 1, 0}},
        {{0, 1, 0}, {0, 0, 1}, {1, 0, 0}},  {{0, -1, 0}, {1, 0, 0}, {0, 0, 1}},
        {{0, 0, 1}, {1, 0, 0}, {0, 1, 0}},  {{0, 0, -1}, {0, 1, 0}, {1, 0, 0}}};

    CubeSphere(int n, Proj p) : N(n), proj(p) {}
    int64_t count() const { return 6LL * N * N; }
    int64_t index(QCell c) const { return (int64_t(c.face) * N + c.i) * N + c.j; }

    static I3 axis(int face, int k) { return {AX[face][k][0], AX[face][k][1], AX[face][k][2]}; }
    static int face_of_axis(const I3& n) {
        for (int f = 0; f < 6; ++f) if (idot(axis(f, 0), n) == 1) return f;
        return -1;
    }

    // Cell centre as an integer point on the cube of half-size N (in-face coordinates are odd offsets).
    I3 ipoint(QCell c) const {
        I3 p;
        for (int k = 0; k < 3; ++k)
            p[k] = AX[c.face][0][k] * N + AX[c.face][1][k] * (2 * c.i + 1 - N) + AX[c.face][2][k] * (2 * c.j + 1 - N);
        return p;
    }
    QCell decode(const I3& p) const {
        for (int f = 0; f < 6; ++f)
            if (idot(p, axis(f, 0)) == N)
                return {f, (idot(p, axis(f, 1)) + N - 1) / 2, (idot(p, axis(f, 2)) + N - 1) / 2};
        return {-1, 0, 0};
    }

    // Step one cell along the 3D axis direction d (must be tangent to the cell's face).
    // Returns the new cell; d is transported (it becomes -n_old when the step folds over an edge).
    QCell step(QCell c, I3& d) const {
        I3 n = axis(c.face, 0), p = ipoint(c);
        int along = idot(p, d);
        if (along + 2 <= N - 1) {  // stays on the face
            for (int k = 0; k < 3; ++k) p[k] += 2 * d[k];
            return decode(p);
        }
        for (int k = 0; k < 3; ++k) p[k] += d[k] - n[k];
        d = {-n[0], -n[1], -n[2]};
        return decode(p);
    }
    // The 4 lateral neighbours in order +s, +t, -s, -t (counter-clockwise seen from outside).
    void neighbours(QCell c, QCell out[4]) const {
        if (c.i > 0 && c.i < N - 1 && c.j > 0 && c.j < N - 1) {  // fast path: interior of the face
            out[0] = {c.face, c.i + 1, c.j}; out[1] = {c.face, c.i, c.j + 1};
            out[2] = {c.face, c.i - 1, c.j}; out[3] = {c.face, c.i, c.j - 1};
            return;
        }
        I3 s = axis(c.face, 1), t = axis(c.face, 2);
        I3 d[4] = {s, t, {-s[0], -s[1], -s[2]}, {-t[0], -t[1], -t[2]}};
        for (int k = 0; k < 4; ++k) out[k] = step(c, d[k]);
    }

    std::function<V3(int, double, double)> custom;  // overrides the warp when set (warp.h)
    V3 to_sphere(int f, double a, double b) const {  // a, b in [-1, 1] across the face
        if (custom) return custom(f, a, b);
        if (proj == Proj::EquiAngular) { a = std::tan(a * M_PI / 4); b = std::tan(b * M_PI / 4); }
        V3 n{double(AX[f][0][0]), double(AX[f][0][1]), double(AX[f][0][2])};
        V3 s{double(AX[f][1][0]), double(AX[f][1][1]), double(AX[f][1][2])};
        V3 t{double(AX[f][2][0]), double(AX[f][2][1]), double(AX[f][2][2])};
        V3 p = n + s * a + t * b;
        if (proj == Proj::Nowicki) {  // "spherified cube" (Nowicki 2009): near-uniform, rows are not great circles
            double x2 = p.x * p.x, y2 = p.y * p.y, z2 = p.z * p.z;
            return {p.x * std::sqrt(1 - y2 / 2 - z2 / 2 + y2 * z2 / 3), p.y * std::sqrt(1 - z2 / 2 - x2 / 2 + z2 * x2 / 3),
                    p.z * std::sqrt(1 - x2 / 2 - y2 / 2 + x2 * y2 / 3)};
        }
        return norm(p);
    }
    V3 center(QCell c) const { return to_sphere(c.face, (2.0 * c.i + 1 - N) / N, (2.0 * c.j + 1 - N) / N); }
    // Corners counter-clockwise from outside.
    std::array<V3, 4> corners(QCell c) const {
        auto u = [&](int i) { return (2.0 * i - N) / N; };
        return {to_sphere(c.face, u(c.i), u(c.j)), to_sphere(c.face, u(c.i + 1), u(c.j)),
                to_sphere(c.face, u(c.i + 1), u(c.j + 1)), to_sphere(c.face, u(c.i), u(c.j + 1))};
    }
};

// ---------------------------------------------------------------- geodesic hex sphere
// A cell is a vertex of the icosahedron subdivided F times: (face, a, b) with barycentric weights
// (a, b, F-a-b) on the face's corners. face == -1 means icosahedron vertex a (a pentagon).
// Cells on a face edge belong to the lower-numbered of the two faces (canonical form).
struct HCell {
    int face, a, b;
    bool operator==(const HCell& o) const { return face == o.face && a == o.a && b == o.b; }
};

struct HexSphere {
    int F;
    std::array<V3, 12> V;
    std::array<std::array<int, 3>, 20> T;  // face -> vertex ids, counter-clockwise from outside
    int ef[12][12][2];                     // edge (u, v) -> its two faces
    std::array<std::array<int, 5>, 12> vf; // vertex -> its five faces
    std::array<int, 12> vmin;              // lowest face around each vertex (owner of the pentagon)

    // Lattice steps in counter-clockwise order (weights on corners 0, 1, 2 of the face).
    static constexpr int D[6][3] = {{0, -1, 1}, {1, -1, 0}, {1, 0, -1}, {0, 1, -1}, {-1, 1, 0}, {-1, 0, 1}};

    explicit HexSphere(int f) : F(f) {
        const double p = (1 + std::sqrt(5.0)) / 2;
        int k = 0;
        for (int s1 : {-1, 1}) for (int s2 : {-1, 1}) {
            V[k++] = {0, double(s1), s2 * p};
            V[k++] = {double(s1), s2 * p, 0};
            V[k++] = {s2 * p, 0, double(s1)};
        }
        for (auto& v : V) v = norm(v);
        double edge = 1e9;  // shortest vertex distance = icosahedron edge
        for (int i = 0; i < 12; ++i) for (int j = i + 1; j < 12; ++j) edge = std::min(edge, len(V[i] - V[j]));
        auto adj = [&](int i, int j) { return std::abs(len(V[i] - V[j]) - edge) < 1e-9; };
        int nf = 0;
        for (int i = 0; i < 12; ++i) for (int j = i + 1; j < 12; ++j) for (int l = j + 1; l < 12; ++l)
            if (adj(i, j) && adj(j, l) && adj(i, l)) {
                std::array<int, 3> t{i, j, l};
                if (dot(cross(V[j] - V[i], V[l] - V[i]), V[i]) < 0) std::swap(t[1], t[2]);
                T[nf++] = t;
            }
        for (auto& r : ef) for (auto& c : r) c[0] = c[1] = -1;
        std::array<int, 12> nvf{};
        for (int f2 = 0; f2 < 20; ++f2)
            for (int m = 0; m < 3; ++m) {
                int u = T[f2][m], v = T[f2][(m + 1) % 3];
                int* e = ef[std::min(u, v)][std::max(u, v)];
                (e[0] < 0 ? e[0] : e[1]) = f2;
                vf[u][nvf[u]++] = f2;
            }
        for (int v = 0; v < 12; ++v) vmin[v] = *std::min_element(vf[v].begin(), vf[v].end());
    }
    int64_t count() const { return 10LL * F * F + 2; }
    const int* edge_faces(int u, int v) const { return ef[std::min(u, v)][std::max(u, v)]; }

    HCell canon(int f, int w0, int w1, int w2) const {
        int w[3] = {w0, w1, w2};
        int zeros = (w0 == 0) + (w1 == 0) + (w2 == 0);
        if (zeros == 2) for (int m = 0; m < 3; ++m) if (w[m] == F) return {-1, T[f][m], 0};
        if (zeros == 1) {
            int z = w0 == 0 ? 0 : (w1 == 0 ? 1 : 2);
            int u = T[f][(z + 1) % 3], v = T[f][(z + 2) % 3];
            const int* e = edge_faces(u, v);
            int g = std::min(e[0], e[1]);
            if (g == f) return {f, w0, w1};
            int W[3];
            for (int m = 0; m < 3; ++m)
                W[m] = T[g][m] == u ? w[(z + 1) % 3] : (T[g][m] == v ? w[(z + 2) % 3] : 0);
            return {g, W[0], W[1]};
        }
        return {f, w0, w1};
    }
    // All (face, weights) views of a cell: 1 inside a face, 2 on an edge, 5 at a pentagon.
    int views(HCell c, int fo[5], int wo[5][3]) const {
        if (c.face < 0) {
            for (int m = 0; m < 5; ++m) {
                int g = vf[c.a][m];
                fo[m] = g;
                for (int q = 0; q < 3; ++q) wo[m][q] = T[g][q] == c.a ? F : 0;
            }
            return 5;
        }
        int w[3] = {c.a, c.b, F - c.a - c.b};
        fo[0] = c.face;
        for (int q = 0; q < 3; ++q) wo[0][q] = w[q];
        int z = w[0] == 0 ? 0 : (w[1] == 0 ? 1 : (w[2] == 0 ? 2 : -1));
        if (z < 0) return 1;
        int u = T[c.face][(z + 1) % 3], v = T[c.face][(z + 2) % 3];
        const int* e = edge_faces(u, v);
        int g = e[0] == c.face ? e[1] : e[0];
        fo[1] = g;
        for (int q = 0; q < 3; ++q) wo[1][q] = T[g][q] == u ? w[(z + 1) % 3] : (T[g][q] == v ? w[(z + 2) % 3] : 0);
        return 2;
    }
    // Lateral neighbours; returns 6, or 5 for a pentagon. Interior cells come out counter-clockwise.
    int neighbours(HCell c, HCell out[6]) const {
        int w2 = F - c.a - c.b;
        if (c.face >= 0 && c.a >= 2 && c.b >= 2 && w2 >= 2) {  // fast path: no edge within reach
            for (int k = 0; k < 6; ++k) out[k] = {c.face, c.a + D[k][0], c.b + D[k][1]};
            return 6;
        }
        int fo[5], wo[5][3], n = 0;
        int nv = views(c, fo, wo);
        for (int v = 0; v < nv; ++v)
            for (int k = 0; k < 6; ++k) {
                int a = wo[v][0] + D[k][0], b = wo[v][1] + D[k][1], d = wo[v][2] + D[k][2];
                if (a < 0 || b < 0 || d < 0) continue;
                HCell h = canon(fo[v], a, b, d);
                bool dup = false;
                for (int m = 0; m < n; ++m) dup |= out[m] == h;
                if (!dup) out[n++] = h;
            }
        return n;
    }
    V3 pos(HCell c) const {
        if (c.face < 0) return V[c.a];
        const auto& t = T[c.face];
        return norm(V[t[0]] * double(c.a) + V[t[1]] * double(c.b) + V[t[2]] * double(F - c.a - c.b));
    }
    // Canonical cell is owned by this face (used to enumerate every cell once).
    bool owned(int f, HCell c) const { return c.face == f || (c.face < 0 && vmin[c.a] == f); }
};

// Sort points counter-clockwise around centre c (seen from outside).
inline void sort_ccw(V3 c, std::vector<V3>& p) {
    V3 ref = std::abs(c.x) < 0.9 ? V3{1, 0, 0} : V3{0, 1, 0};
    V3 u = norm(cross(ref, c)), v = cross(c, u);
    std::sort(p.begin(), p.end(), [&](V3 a, V3 b) {
        return std::atan2(dot(a - c, v), dot(a - c, u)) < std::atan2(dot(b - c, v), dot(b - c, u));
    });
}
