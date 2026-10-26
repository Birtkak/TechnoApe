#include "rock_types.h"

#include <algorithm>
#include <cmath>
#include <cstdlib>

namespace technosim_rock {

namespace {
constexpr uint16_t AIR = 0, DIRT = 2;

inline int floor_div(int a, int b) { return a >= 0 ? a / b : -((-a + b - 1) / b); }
inline uint32_t hash3(int a, int b, int c) {   // WorldGen::hash3
    uint32_t h = (uint32_t)a * 374761393u + (uint32_t)b * 668265263u + (uint32_t)c * 2246822519u;
    h ^= h >> 13;
    h *= 1274126177u;
    h ^= h >> 16;
    return h;
}
inline float u01(uint32_t h) { return (float)(h & 0xFFFF) / 65535.0f; }

void setup(FastNoiseLite &n, int seed, float freq, int octaves) {
    n.SetSeed(seed);
    n.SetNoiseType(FastNoiseLite::NoiseType_OpenSimplex2);
    n.SetFrequency(freq);
    n.SetFractalType(octaves > 1 ? FastNoiseLite::FractalType_FBm : FastNoiseLite::FractalType_None);
    n.SetFractalOctaves(octaves);
}
} // namespace

void RockTypes::configure(int seed, int sea_voxels) {
    seed_ = seed;
    sea_ = sea_voxels;
    setup(n_warp_, seed + 901, 1.0f / 5000.0f, 2);   // province borders wander over ~600 m (no slivers)
    setup(n_thick_, seed + 902, 1.0f / 3000.0f, 1);  // layers thicken and thin over ~400 m
    setup(n_wob_, seed + 903, 1.0f / 900.0f, 2);     // boundaries undulate gently over ~110 m
    setup(n_ochre_, seed + 904, 1.0f / 40.0f, 1);    // ochre patches ~5 m
}

RockCol RockTypes::column(int wx, int wz, float h) const {
    RockCol c;
    const float fx = (float)wx, fz = (float)wz;
    const float ux = fx + n_warp_.GetNoise(fx, fz) * WARP, uz = fz + n_warp_.GetNoise(fx + 7919.0f, fz - 3571.0f) * WARP;
    const int cx0 = floor_div((int)std::floor(ux), PROVINCE_CELL), cz0 = floor_div((int)std::floor(uz), PROVINCE_CELL);
    float best = 1e30f, sx = 0.0f, sz = 0.0f;
    uint32_t bh = 0;
    for (int j = -1; j <= 1; j++)
        for (int i = -1; i <= 1; i++) {
            const uint32_t hh = hash3(cx0 + i, cz0 + j, seed_ + 911);
            const float px = ((float)(cx0 + i) + 0.15f + 0.7f * u01(hh)) * PROVINCE_CELL;
            const float pz = ((float)(cz0 + j) + 0.15f + 0.7f * u01(hh >> 16)) * PROVINCE_CELL;
            const float d = (ux - px) * (ux - px) + (uz - pz) * (uz - pz);
            if (d < best) { best = d; sx = px; sz = pz; bh = hash3(cx0 + i, cz0 + j, seed_ + 913); }
        }
    int pick = (int)(bh % PROV_WEIGHT_TOTAL), p = 0;
    while (p < NPROV - 1 && pick >= PROV_WEIGHT[p]) pick -= PROV_WEIGHT[p++];
    c.prov = p;
    const float an = u01(bh >> 8) * 6.2832f;
    const float tilt = PROV_DIP[p] * ((fx - sx) * std::cos(an) + (fz - sz) * std::sin(an));
    float y = (float)(sea_ + PROV_TOP[p]) + PROV_FOLLOW[p] * (h - (float)sea_) + tilt + n_wob_.GetNoise(fx, fz) * PROV_WOBBLE[p];
    for (int k = 0; k < MAX_LAYERS - 1; k++) {
        if (k < PROV_NLAYERS[p] - 1) {
            const float t = 0.5f + 0.5f * n_thick_.GetNoise(fx + 1000.0f * (float)k, fz - 1300.0f * (float)k);
            y -= (float)THICK_MIN[p][k] + (float)(THICK_MAX[p][k] - THICK_MIN[p][k]) * t;
            c.b[k] = y;
        } else {
            c.b[k] = -1e9f;
        }
    }
    return c;
}

RockCol RockTypes::blend(const RockCol &c00, const RockCol &c10, const RockCol &c01, const RockCol &c11, float tx, float tz, const RockCol &near) {
    RockCol c;
    c.prov = near.prov;
    for (int k = 0; k < MAX_LAYERS - 1; k++) {
        // only corners of the same province blend: a province border is a fault, its strata do not meet
        float sum = 0.0f, w = 0.0f;
        const RockCol *cs[4] = {&c00, &c10, &c01, &c11};
        const float ws[4] = {(1.0f - tx) * (1.0f - tz), tx * (1.0f - tz), (1.0f - tx) * tz, tx * tz};
        for (int i = 0; i < 4; i++)
            if (cs[i]->prov == near.prov) { sum += ws[i] * cs[i]->b[k]; w += ws[i]; }
        c.b[k] = w > 0.0f ? sum / w : near.b[k];
    }
    return c;
}

int RockTypes::rock_at(const RockCol &c, int wy) const {
    int layer = 0;
    const float fy = (float)wy;
    while (layer < PROV_NLAYERS[c.prov] - 1 && fy < c.b[layer]) layer++;
    return LAYER_ROCK[c.prov][layer];
}

uint16_t RockTypes::at(const RockCol &c, int wx, int wy, int wz) const {
    const int r = rock_at(c, wy);
    if (PYRITE_ROCKS & (1u << r)) {
        // a 2x2x2 nodule in the middle of a 0.5 m cell, now and then
        const int ox = wx & 3, oy = wy & 3, oz = wz & 3;
        if (ox >= 1 && ox <= 2 && oy >= 1 && oy <= 2 && oz >= 1 && oz <= 2 && hash3(wx >> 2, (wy >> 2) * 977 + (wz >> 2), seed_ + 917) % 1000 < (uint32_t)PYRITE_PER_MILLE)
            return (uint16_t)PYRITE;
    }
    return (uint16_t)ROCK_MAT[r];
}

uint16_t RockTypes::loose(const RockCol &c, int h) const { return (uint16_t)LOOSE_MAT[rock_at(c, h - 1)]; }

uint16_t RockTypes::soil(const RockCol &c, uint16_t under, int h, int wx, int wz) const {
    if (under != DIRT || !(OCHRE_ROCKS & (1u << rock_at(c, h - 16)))) return under;
    const float n = n_ochre_.GetNoise((float)wx, (float)wz);
    if (n < 1.0f - 2.6f * OCHRE_SHARE) return under;   // ~OCHRE_SHARE of the area
    return n_ochre_.GetNoise((float)wx * 0.3f + 500.0f, (float)wz * 0.3f) > 0.0f ? (uint16_t)RED_OCHRE : (uint16_t)YELLOW_OCHRE;
}

float RockTypes::ore_bias(const RockCol &c, int wy, int vein) const {
    return (vein >= 0 && vein < NVEIN) ? ORE_BIAS[rock_at(c, wy)][vein] : 1.0f;
}

bool RockTypes::uniform(const RockCol *cols, int n, int y0, int y1, uint16_t &out) const {
    const int p = cols[0].prov;
    int layer = -1;
    for (int i = 0; i < n; i++) {
        if (cols[i].prov != p) return false;
        int lt = 0, lb = 0;
        while (lt < PROV_NLAYERS[p] - 1 && (float)(y1 - 1) < cols[i].b[lt]) lt++;
        while (lb < PROV_NLAYERS[p] - 1 && (float)y0 < cols[i].b[lb]) lb++;
        if (lt != lb || (layer >= 0 && lt != layer)) return false;
        layer = lt;
    }
    out = (uint16_t)ROCK_MAT[LAYER_ROCK[p][layer]];
    return true;
}

bool RockTypes::clutter_pick(int biome, bool beach, bool snowy, bool forest, uint32_t hs, int &kind, uint16_t &mat) const {
    if (biome < 0 || biome >= NBIOME) return false;
    const float roll = (float)((hs >> 9) & 0xFFFFF) / (float)0x100000;
    float acc = 0.0f;
    for (int i = 0; i < NCLUT; i++) {
        const float ch = CLUT_CHANCE[i][biome];
        if (ch <= 0.0f) continue;
        bool ok;
        switch (CLUT_WHERE[i]) {
            case W_OPEN: ok = !forest && !snowy && !beach; break;
            case W_FOREST: ok = forest && !snowy; break;
            case W_BEACH: ok = beach; break;
            case W_SNOW: ok = snowy; break;
            default: ok = !snowy || biome == 0; break;
        }
        if (!ok) continue;
        acc += ch;
        if (roll < acc) {
            kind = KIND_BASE + CLUT_SHAPE[i];
            mat = (uint16_t)CLUT_MAT[i];
            return true;
        }
    }
    return false;
}

uint16_t RockTypes::clutter_voxel(int kind, uint32_t seed, uint16_t mat, int dx, int dy, int dz) {
    // a along the piece, b across it; turned and mirrored per piece
    const bool turn = seed & 1, flip = (seed >> 1) & 1;
    int a = turn ? dz : dx, b = turn ? dx : dz;
    if (flip) a = -a;
    const int aa = std::abs(a), ab = std::abs(b);
    switch (kind - KIND_BASE) {
        case S_STRAWBERRY: {
            // a low rosette of leaves (plant_stem), three or four berries on and beside it: one piece
            constexpr uint16_t LEAF = 111;
            if (dy == 0 && aa <= 1 && ab <= 1 && !(aa == 1 && ab == 1 && ((seed >> (2 + a + 3 * b + 4)) & 1))) {
                if ((a == 1 && b == 0) || (a == -1 && b == 1)) return mat;
                return LEAF;
            }
            if (dy == 0 && a == 2 && b == 0) return mat;                         // hanging over the rim
            if (dy == 1 && ((a == 0 && b == 0) || (a == -1 && b == 0 && ((seed >> 9) & 1)))) return mat;
            if (dy == 1 && a == 1 && b == -1) return LEAF;
            return AIR;
        }
        case S_NUTS: {
            // a little heap: 5-7 nuts on the ground, one or two on top
            if (dy == 0 && aa <= 1 && ab <= 1 && (aa + ab < 2 || ((seed >> (3 + (a + 1) * 3 + (b + 1))) & 1))) return mat;
            if (dy == 1 && a == 0 && (b == 0 || (b == 1 && ((seed >> 14) & 1)))) return mat;
            return AIR;
        }
        case S_PINE_CONE: {
            // lying on its side: 3 long, 2 wide, a ridge of scales on top
            if (dy == 0 && aa <= 1 && b >= 0 && b <= 1) return mat;
            if (dy == 1 && aa <= 1 && b == (int)((seed >> 4) & 1) && a != 1) return mat;
            return AIR;
        }
        case S_BONES: {
            // a long bone with knobbed ends, and a second one across it now and then
            if (dy == 0 && b == 0 && aa <= 2) return mat;
            if (dy == 0 && aa == 2 && ab == 1) return mat;
            if ((seed >> 5) & 1) {
                if (dy == 1 && a == 0 && ab <= 1) return mat;
                if (dy == 0 && a == 0 && b == 2) return mat;
            }
            return AIR;
        }
        case S_FEATHER: {
            // flat on the ground: a quill and a vane to one side, a curl at the tip
            if (dy != 0) return AIR;
            if (b == 0 && aa <= 2) return mat;
            if (b == 1 && a >= -1 && a <= 1) return mat;
            if (b == 2 && a == 0 && ((seed >> 6) & 1)) return mat;
            return AIR;
        }
        case S_DRIFTWOOD: {
            // a weathered log, two thick, a broken stub on top
            if (dy == 0 && aa <= 2 && b >= 0 && b <= 1) return mat;
            if (dy == 1 && aa <= 1 && b == 0) return mat;
            if (dy == 1 && a == 2 && b == 1 && ((seed >> 7) & 1)) return mat;
            if (dy == 2 && a == -1 && b == 0 && ((seed >> 8) & 1)) return mat;
            return AIR;
        }
        case S_ANTLER: {
            // a beam on the ground with tines rising from it
            if (dy == 0 && b == 0 && aa <= 2) return mat;
            if (dy == 0 && a == -2 && b == 1) return mat;                       // the burr
            if ((dy == 1 || dy == 2) && a == -1 && b == 0) return mat;
            if (dy == 1 && a == 1 && b == 0) return mat;
            if (dy == 2 && a == 1 && b == -1) return mat;
            if (dy == 1 && a == 2 && b == 0) return mat;
            if (dy == 2 && a == 2 && b == 0 && ((seed >> 9) & 1)) return mat;
            return AIR;
        }
        case S_SHARD: {
            // ice spikes leaning out of the snow: a tall one, two smaller
            const int hc = 3 + (int)((seed >> 4) % 3);
            if (a == 0 && b == 0 && dy < hc) return mat;
            if (a == 1 && b == 0 && dy < hc - 2) return mat;
            if (a == 0 && b == 1 && dy < hc - 1) return mat;
            if (a == -1 && b == -1 && dy < 1 + (int)((seed >> 7) & 1)) return mat;
            return AIR;
        }
    }
    return AIR;
}

int RockTypes::crystal_height(int wx, int wz, int &roll) const {
    // one cluster at most per 1.5 m cell, centred away from the cell edge
    constexpr int CC = 12;
    const int cx = floor_div(wx, CC), cz = floor_div(wz, CC);
    const uint32_t h = hash3(cx, cz, seed_ + 919);
    roll = (int)(h % 1000);
    if (roll >= std::max(CRYSTAL_PER_MILLE, CRYSTAL_CAVE_PER_MILLE)) return 0;
    const int px = cx * CC + 4 + (int)((h >> 10) % 4), pz = cz * CC + 4 + (int)((h >> 14) % 4);
    const int dx = wx - px, dz = wz - pz;
    const int adx = std::abs(dx), adz = std::abs(dz);
    if (adx > 2 || adz > 2) return 0;
    const int top = 4 + (int)((h >> 18) % 3);   // the middle spike: 4-6 voxels
    if (dx == 0 && dz == 0) return top;
    if (adx + adz == 1) return ((h >> (20 + (dx + 1) + 3 * (dz + 1))) & 1) ? top - 2 : top - 1;
    if (adx == 1 && adz == 1) return ((h >> (24 + (dx + 1) + (dz + 1))) & 1) ? 1 : 0;
    if (adx + adz == 2) return ((h >> (28 + (dx + 2) / 2 + (dz + 2))) & 1) ? 2 : 0;   // a few outliers
    return 0;
}

bool RockTypes::crystal_allowed(int roll, int depth, int cave_biome) {
    if (depth < CRYSTAL_MIN_DEPTH) return false;
    return roll < CRYSTAL_PER_MILLE || (cave_biome == CRYSTAL_CAVE_BIOME && roll < CRYSTAL_CAVE_PER_MILLE);
}

uint16_t RockTypes::ruin_pot_voxel(int rx, int rz, int hw, int hd, uint32_t rseed, int mx, int mz, int lx, int lz, int dy) {
    // Pots stand in the ruin's inside corners only: 4 at most per ruin, never side by side.
    const int ax = mx - rx, az = mz - rz;
    if (std::abs(ax) >= hw || std::abs(az) >= hd) return AIR;   // inside the walls only
    const bool corner = std::abs(ax) == hw - 1 && std::abs(az) == hd - 1;
    const uint32_t h = hash3(mx, mz, (int)rseed + 37);
    if (!(corner && (h & 3) != 0)) return AIR;
    const uint16_t m = ((h >> 3) % 3) == 0 ? (uint16_t)CLAY_POT_CRACKED : (uint16_t)CLAY_POT;
    // shapes over the 4x4 voxels of the cell: r = distance class from its middle (0 inner 2x2, 1 edge, 2 corner)
    const int ex = (lx == 0 || lx == 3) ? 1 : 0, ez = (lz == 0 || lz == 3) ? 1 : 0;
    const int r = ex + ez;
    const bool chipped = m == CLAY_POT_CRACKED && ((h >> (8 + lx + 4 * lz)) & 1) && ((h >> 24) & 1);
    switch ((h >> 5) % 3) {
        case 0:   // amphora: narrow foot, wide belly, neck, two handles
            if (dy == 0) return r == 0 ? m : AIR;
            if (dy >= 1 && dy <= 3) return (r < 2 && !(chipped && dy == 2 && r == 1)) ? m : AIR;
            if (dy == 4) return r == 0 ? m : AIR;
            if (dy == 5) return (r == 0 && (lx + lz) % 2 == 0) || (r == 1 && ((lx == 0 && lz == 1) || (lx == 3 && lz == 2))) ? m : AIR;
            if (dy == 6) return r == 0 && !chipped ? m : AIR;
            return AIR;
        case 1:   // jar: squat and wide, an open mouth
            if (dy <= 2) return (r < 2 || dy == 1) && !(chipped && dy == 2) ? m : AIR;
            if (dy == 3) return r == 1 ? m : AIR;
            return AIR;
        default:  // urn: a foot, a belly, a flared rim
            if (dy == 0) return r == 0 ? m : AIR;
            if (dy == 1 || dy == 2) return r < 2 ? m : AIR;
            if (dy == 3) return r == 0 ? m : AIR;
            if (dy == 4) return r < 2 && !(chipped && r == 1) ? m : AIR;
            return AIR;
    }
}

} // namespace technosim_rock
