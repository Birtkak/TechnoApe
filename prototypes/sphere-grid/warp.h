// Relaxed cube-sphere warp: the equi-angular map plus w times the displacement to the harmonic cube-sphere
// (smooth.cpp), tabulated on a (K+1)^2 vertex lattice per face and bilinearly interpolated.
// w = 0 is equi-angular, w = 1 the harmonic (conformal) cube-sphere.
#pragma once
#include "grid.h"
#include <cstdio>

struct Warp {
    int K = 0;
    double w = 0;
    double corner_L = 0;  // > 0: fade w to 0 near the 8 corners, w * (1 - exp(-(angle to corner / L)^2)), L in radians
    std::vector<V3> D;  // [face][ia][ib], ia along the face's s axis, ib along t

    V3& at(int f, int ia, int ib) { return D[(size_t(f) * (K + 1) + ia) * (K + 1) + ib]; }
    const V3& at(int f, int ia, int ib) const { return D[(size_t(f) * (K + 1) + ia) * (K + 1) + ib]; }

    V3 map(int f, double a, double b) const {
        static const CubeSphere ea(1, Proj::EquiAngular);
        V3 p = ea.to_sphere(f, a, b);
        if (w == 0 || K == 0) return p;
        double x = (a + 1) * 0.5 * K, y = (b + 1) * 0.5 * K;
        int i = std::clamp(int(x), 0, K - 1), j = std::clamp(int(y), 0, K - 1);
        double fx = x - i, fy = y - j;
        V3 d = at(f, i, j) * ((1 - fx) * (1 - fy)) + at(f, i + 1, j) * (fx * (1 - fy)) +
               at(f, i, j + 1) * ((1 - fx) * fy) + at(f, i + 1, j + 1) * (fx * fy);
        double we = w;
        if (corner_L > 0) {
            double ang = std::acos(std::min(1.0, (std::abs(p.x) + std::abs(p.y) + std::abs(p.z)) / std::sqrt(3.0)));
            we *= 1 - std::exp(-(ang / corner_L) * (ang / corner_L));
        }
        return norm(p + d * we);
    }
    void attach(CubeSphere& g) const { g.custom = [this](int f, double a, double b) { return map(f, a, b); }; }

    bool save(const char* path) const {
        FILE* o = std::fopen(path, "wb");
        if (!o) return false;
        std::fwrite(&K, sizeof K, 1, o);
        std::fwrite(D.data(), sizeof(V3), D.size(), o);
        return std::fclose(o) == 0;
    }
    bool load(const char* path) {
        FILE* f = std::fopen(path, "rb");
        if (!f || std::fread(&K, sizeof K, 1, f) != 1) return false;
        D.resize(size_t(6) * (K + 1) * (K + 1));
        bool ok = std::fread(D.data(), sizeof(V3), D.size(), f) == D.size();
        std::fclose(f);
        return ok;
    }
};
