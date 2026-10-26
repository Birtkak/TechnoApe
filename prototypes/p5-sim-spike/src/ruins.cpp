#include "ruins.h"

#include <algorithm>
#include <cmath>
#include <cstdlib>

#include "rock_types.h"

namespace technosim_ruin {

namespace {

constexpr uint16_t AIR = 0;
constexpr uint16_t TERRAIN = 0xFFFE;   // the ground as FineGen makes it (voxel() returns PASS)

inline int fdiv(int a, int b) { return a >= 0 ? a / b : -((-a + b - 1) / b); }

inline void to_uv(int rot, int dx, int dz, int &u, int &v) {
    switch (rot & 3) {
        case 0: u = dx; v = dz; break;
        case 1: u = dz; v = -dx; break;
        case 2: u = -dx; v = -dz; break;
        default: u = -dz; v = dx; break;
    }
}
inline void to_uvf(int rot, float dx, float dz, float &u, float &v) {
    switch (rot & 3) {
        case 0: u = dx; v = dz; break;
        case 1: u = dz; v = -dx; break;
        case 2: u = -dx; v = -dz; break;
        default: u = -dz; v = dx; break;
    }
}
inline void from_uv(int rot, int u, int v, int &dx, int &dz) {
    switch (rot & 3) {
        case 0: dx = u; dz = v; break;
        case 1: dx = -v; dz = u; break;
        case 2: dx = -u; dz = -v; break;
        default: dx = v; dz = -u; break;
    }
}

bool square_shape(int shape) { return shape == S_WATCHTOWER || shape == S_STONE_CIRCLE || shape == S_SHRINE; }

// crypt layout: the room's floor lies D macro cells under the ruin's floor
inline int crypt_d(const Ruin &r) { return std::max(4, r.depth - 1); }

} // namespace

uint32_t hash(int a, int b, int c) {   // WorldGen::hash3
    uint32_t h = (uint32_t)a * 374761393u + (uint32_t)b * 668265263u + (uint32_t)c * 2246822519u;
    h ^= h >> 13;
    h *= 1274126177u;
    h ^= h >> 16;
    return h;
}

int pick_kind(int biome, bool coast, uint32_t h) {
    if (biome < 0 || biome >= 64) return -1;
    int total = 0;
    for (int k = 0; k < NKIND; k++)
        if (KIND_COAST[k] == coast && ((KIND_BIOMES[k] >> biome) & 1ULL)) total += KIND_WEIGHT[k];
    if (total <= 0) return -1;
    int pick = (int)(h % (uint32_t)total);
    for (int k = 0; k < NKIND; k++) {
        if (!(KIND_COAST[k] == coast && ((KIND_BIOMES[k] >> biome) & 1ULL))) continue;
        pick -= KIND_WEIGHT[k];
        if (pick < 0) return k;
    }
    return -1;
}

void shape_ruin(Ruin &r) {
    const int k = r.kind;
    const uint32_t h = hash((int)r.seed, 17, 5);
    const int U = KIND_HALF_U[k][0] + (int)(h % (uint32_t)(KIND_HALF_U[k][1] - KIND_HALF_U[k][0] + 1));
    int V = KIND_HALF_V[k][0] + (int)((h >> 8) % (uint32_t)(KIND_HALF_V[k][1] - KIND_HALF_V[k][0] + 1));
    if (square_shape(KIND_SHAPE[k])) V = U;
    r.rot = (int)((h >> 16) & 3);
    r.hw = (r.rot & 1) ? V : U;
    r.hd = (r.rot & 1) ? U : V;
    r.top = KIND_TOP[k];
    r.depth = KIND_DEPTH[k];
}

int pot_spots(const Ruin &r, PotSpot out[4]) {
    const int shape = KIND_SHAPE[r.kind];
    const int U = (r.rot & 1) ? r.hd : r.hw, V = (r.rot & 1) ? r.hw : r.hd;
    int cu[4], cv[4], n = 0, y = r.base + 1;
    switch (shape) {
        case S_WATCHTOWER: {
            const int t = std::max(2, U - 4);
            const int c[4][2] = {{-(t - 1), -(t - 1)}, {t - 1, -(t - 1)}, {-(t - 1), t - 1}, {t - 1, t - 1}};
            for (int i = 0; i < 4; i++) { cu[n] = c[i][0]; cv[n] = c[i][1]; n++; }
            break;
        }
        case S_STONE_CIRCLE: {
            const int c[4][2] = {{0, 2}, {0, -2}, {3, 0}, {-3, 0}};
            for (int i = 0; i < 4; i++) { cu[n] = c[i][0]; cv[n] = c[i][1]; n++; }
            y = r.base;
            break;
        }
        case S_CRYPT: {
            const int c[4][2] = {{-(U - 2), -(V - 2)}, {U - 2, -(V - 2)}, {-(U - 2), V - 8}, {U - 2, V - 8}};
            for (int i = 0; i < 4; i++) { cu[n] = c[i][0]; cv[n] = c[i][1]; n++; }
            y = r.base - crypt_d(r) + 1;
            break;
        }
        case S_SHRINE: {
            const int av = -(U - 3), p = U - 2;
            const int c[3][2] = {{-1, av}, {1, av}, {0, p}};
            for (int i = 0; i < 3; i++) { cu[n] = c[i][0]; cv[n] = c[i][1]; n++; }
            y = r.base + 2;
            break;
        }
        case S_WRECK: {
            const int c[3][2] = {{-2, 0}, {-4, 0}, {3, 0}};
            for (int i = 0; i < 3; i++) { cu[n] = c[i][0]; cv[n] = c[i][1]; n++; }
            y = r.base;
            break;
        }
        default: {   // house, mud house: the inside corners
            const int c[4][2] = {{-(U - 1), -(V - 1)}, {U - 1, -(V - 1)}, {-(U - 1), V - 1}, {U - 1, V - 1}};
            for (int i = 0; i < 4; i++) { cu[n] = c[i][0]; cv[n] = c[i][1]; n++; }
            break;
        }
    }
    // each spot holds a pot 3 times in 4, at least one, at most the kind's limit (<= 4)
    const int limit = std::min(4, KIND_POTS[r.kind]);
    const uint32_t h = hash((int)r.seed, 77, 13);
    const int start = (int)(h % (uint32_t)n);
    int got = 0;
    for (int i = 0; i < n && got < limit; i++) {
        const int c = (start + i) % n;
        const bool keep = ((h >> (8 + 2 * i)) & 3) != 0 || (got == 0 && i == n - 1);
        if (!keep) continue;
        int dx, dz;
        from_uv(r.rot, cu[c], cv[c], dx, dz);
        PotSpot &s = out[got++];
        s.mx = r.x + dx;
        s.mz = r.z + dz;
        s.y = y;
        s.h = hash(s.mx, s.mz, (int)r.seed + 37);
    }
    return got;
}

void prepare(const Ruin &r, Prepared &p) {
    p.r = r;
    p.shape = KIND_SHAPE[r.kind];
    p.U = (r.rot & 1) ? r.hd : r.hw;
    p.V = (r.rot & 1) ? r.hw : r.hd;
    for (int i = 0; i < 4; i++) p.mat[i] = KIND_MAT[r.kind][i];
    p.npots = pot_spots(r, p.pots);
    p.nstones = 0;
    if (p.shape == S_STONE_CIRCLE) {
        const uint32_t h = hash((int)r.seed, 41, 3);
        p.nstones = 8 + (int)(h % 5);
        const float rad = (float)p.U - 1.5f;
        for (int i = 0; i < p.nstones; i++) {
            const uint32_t sh = hash((int)r.seed, i, 43);
            const float jit = ((float)(sh & 255) / 255.0f - 0.5f) * 0.3f;
            const float a = 6.2831853f * ((float)i + jit) / (float)p.nstones;
            p.st_c[i] = std::cos(a);
            p.st_s[i] = std::sin(a);
            p.st_u[i] = rad * p.st_c[i];
            p.st_v[i] = rad * p.st_s[i];
            p.st_h[i] = 4.0f + (float)((sh >> 8) % 4) + (float)((sh >> 12) & 3) * 0.25f;
            p.st_fallen[i] = ((sh >> 16) & 3) == 0;
        }
    }
}

namespace {

struct Ctx {
    int mx, my, mz, u, v;
    float fu, fv, fy;   // macro units on the shape's axes, fy above the floor's bottom (my == base -> 0..1)
    int wx, wy, wz, fs, surf;
};

uint16_t house(const Prepared &p, const Ctx &c) {
    const Ruin &r = p.r;
    const int au = std::abs(c.u), av = std::abs(c.v);
    if (au > p.U || av > p.V) {   // rubble round the walls
        if (c.my == fdiv(c.surf, c.fs) && (hash(c.mx, c.mz, (int)r.seed + 5) & 15) == 0) return p.mat[M_EXTRA];
        return TERRAIN;
    }
    if (c.my == r.base) return (hash(c.mx, c.mz, 3) & 3) ? p.mat[M_FLOOR] : p.mat[M_TRIM];
    const bool perim = au == p.U || av == p.V, corner = au == p.U && av == p.V;
    if (perim && c.my > r.base) {
        const int seg = corner ? 99 : (au == p.U ? (c.v + p.V) / 4 + 100 * (c.u > 0) : (c.u + p.U) / 4 + 200 * (c.v > 0));
        const uint32_t sh = hash((int)r.seed, seg, 7);
        int hgt = corner ? 6 + (int)(sh % 4) : (int)(sh % 7);
        if (!corner && ((sh >> 8) & 7) == 0) hgt = 0;
        hgt = std::min(hgt, r.top);
        if (c.my <= r.base + hgt) {
            // stones go missing more toward the top; a stone stands only on the one below (none floats)
            const int ws = (int)hash(c.mx, c.mz, (int)r.seed);
            for (int k = r.base + 1; k <= c.my; k++)
                if ((hash(k, ws, 11) & 31) < (uint32_t)(k - r.base)) return AIR;
            return p.mat[M_WALL];
        }
    }
    return TERRAIN;
}

uint16_t mud_house(const Prepared &p, const Ctx &c) {
    const Ruin &r = p.r;
    const int au = std::abs(c.u), av = std::abs(c.v);
    if (au > p.U || av > p.V) {   // fallen lumps of wall
        if (c.my == fdiv(c.surf, c.fs) && (hash(c.mx, c.mz, (int)r.seed + 5) & 15) == 0) return p.mat[M_WALL];
        return TERRAIN;
    }
    if (c.my == r.base) return p.mat[M_FLOOR];
    const int H = std::max(3, r.top - 1);
    const bool perim = au == p.U || av == p.V, corner = au == p.U && av == p.V;
    if (perim) {
        if (c.v == -p.V && au <= 1) return AIR;                                                  // the doorway (its lintel fell)
        if (au == p.U && !corner && c.v % 3 == 0 && c.my == r.base + 3) return AIR;              // windows
        if (av == p.V && !corner && c.v > 0 && c.u % 3 == 0 && c.my == r.base + 3) return AIR;
        int hgt = H;
        if (av == p.V && !corner) {   // front and back walls wear down; side walls carry the beams
            const uint32_t sh = hash((int)r.seed, (c.u + p.U) / 3 + 300 * (c.v > 0), 17);
            hgt = H - (int)(sh % 4);
            if (((sh >> 8) & 7) == 0) hgt = 1;
        }
        if (c.my <= r.base + hgt) {
            // the top course is rounded off by the weather
            if (c.my == r.base + hgt && c.wy - c.my * c.fs >= 2 && (hash(c.wx, c.wz, (int)r.seed + 19) & 3) == 0) return AIR;
            return p.mat[M_WALL];
        }
        return TERRAIN;
    }
    // roof beams resting on the side walls
    if (c.my == r.base + H && (c.v + p.V) % 3 == 1 && (hash((int)r.seed, c.v, 27) & 3) != 0) {
        const float dv = c.fv - (float)c.v;
        if (c.wy - c.my * c.fs <= 2 && std::fabs(dv) <= 0.3f) return p.mat[M_TRIM];
    }
    // sand drifted in against the walls
    if (c.my == r.base + 1) {
        const int dist = std::min(p.U - au, p.V - av);
        if (dist == 1) {
            const int dh = 1 + (int)(hash(c.mx, c.mz, (int)r.seed + 33) % 3);
            if (c.wy - c.my * c.fs < dh) return p.mat[M_EXTRA];
        }
    }
    return TERRAIN;
}

uint16_t watchtower(const Prepared &p, const Ctx &c) {
    const Ruin &r = p.r;
    const int t = std::max(2, p.U - 4), H = std::max(6, r.top - 1);
    const int au = std::abs(c.u), av = std::abs(c.v), cheb = std::max(au, av);
    if (au > p.U || av > p.V) return TERRAIN;
    if (cheb == t) {   // the tower's wall, fallen toward +v
        if (c.my == r.base) return p.mat[M_WALL];
        if (c.v == -t && c.u == 0 && c.my <= r.base + 4) return AIR;                               // door
        if (au == t && c.v == 0 && (c.my == r.base + 7 || c.my == r.base + 8)) return AIR;       // arrow slits
        if (c.v == -t && c.u == 0 && (c.my == r.base + 9 || c.my == r.base + 10)) return AIR;
        int hgt;
        if (c.v == t) hgt = (int)(hash((int)r.seed, c.u, 21) % 3);
        else hgt = H - ((c.v + t) * (H - 3)) / (2 * t) - (int)(hash((int)r.seed, c.u * 7 + c.v, 23) % 3);
        if (c.my <= r.base + hgt) {
            const uint32_t vh = hash(c.my, (int)hash(c.mx, c.mz, (int)r.seed), 13);
            return (vh & 7) == 1 ? p.mat[M_TRIM] : p.mat[M_WALL];
        }
        return TERRAIN;
    }
    if (cheb < t) {
        if (c.my == r.base) return p.mat[M_FLOOR];
        // what is left of the upper floor, on the side still standing
        if (c.my == r.base + 6 && c.v < 0 && (hash(c.mx, c.mz, (int)r.seed + 3) & 3) != 0 && c.wy - c.my * c.fs < 2) return p.mat[M_EXTRA];
        return TERRAIN;
    }
    // the fallen side: a spill of stones running out from the broken wall
    if (c.v > t) {
        const int d = c.v - t, spread = t + 1 + d / 2;
        if (au <= spread) {
            const uint32_t hh = hash(c.mx, c.mz, (int)r.seed + 29);
            const int ph = std::max(0, 4 - d) - (au > t ? 1 : 0) + (int)(hh & 1);
            if (c.my >= r.base && c.my < r.base + ph) {   // a heap: solid to its top, nothing loose
                const uint32_t hv = hash(c.mx * 3 + c.my, c.mz, (int)r.seed + 30);
                return (hv & 3) == 0 ? p.mat[M_TRIM] : p.mat[M_WALL];
            }
            return TERRAIN;
        }
    }
    if (c.my == r.base && (hash(c.mx, c.mz, (int)r.seed + 31) & 31) == 0) return p.mat[M_WALL];
    return TERRAIN;
}

uint16_t stone_circle(const Prepared &p, const Ctx &c) {
    const Ruin &r = p.r;
    if (c.fy < 0.0f) return TERRAIN;
    // the altar stone in the middle
    if (std::fabs(c.fu) <= 1.0f && std::fabs(c.fv) <= 0.55f && c.fy < 1.0f) return p.mat[M_FLOOR];
    const float R = (float)p.U;
    for (int i = 0; i < p.nstones; i++) {
        const float ru = c.fu - p.st_u[i], rv = c.fv - p.st_v[i];
        const float radial = ru * p.st_c[i] + rv * p.st_s[i], tang = -ru * p.st_s[i] + rv * p.st_c[i];
        const float h = p.st_h[i];
        if (!p.st_fallen[i]) {
            if (c.fy >= h) continue;
            float half = 1.1f;
            if (c.fy > h - 1.0f) half *= 1.0f - 0.45f * (c.fy - (h - 1.0f));   // a rounded top
            if (std::fabs(radial) <= 0.6f && std::fabs(tang) <= half) {
                if (c.fy >= h - 0.3f && (hash(c.wx, c.wz, (int)r.seed + 45) & 1)) return p.mat[M_TRIM];   // moss
                return p.mat[M_WALL];
            }
        } else {
            // toppled inward, never as far as the altar or the pots beside it
            const float len = std::min(h, R - 4.5f);
            if (radial <= 0.6f && radial >= -len && std::fabs(tang) <= 1.1f && c.fy < 1.2f) {
                if (c.fy >= 0.95f && (hash(c.wx, c.wz, (int)r.seed + 47) & 1)) return p.mat[M_TRIM];
                return p.mat[M_EXTRA];
            }
        }
    }
    return TERRAIN;
}

uint16_t crypt(const Prepared &p, const Ctx &c) {
    const Ruin &r = p.r;
    const int U = p.U, V = p.V, D = crypt_d(r);
    const int floor_my = r.base - D, ceil_my = r.base - 2;
    const int au = std::abs(c.u);
    const int base_f = r.base * c.fs;
    // the stairs: down from the footprint's +v edge to the room's floor, a voxel down per voxel along
    int sf = base_f - (int)std::floor(((float)V + 0.5f - c.fv) * (float)c.fs);
    sf = std::max(sf, (floor_my + 1) * c.fs);
    const int headroom = 2 * c.fs * 2;   // 2 m
    if (au <= 1 && c.v >= V - 7 && c.v <= V) {
        if (c.wy >= sf) {
            if (sf + headroom < base_f && c.wy >= sf + headroom) {
                if (c.wy < sf + headroom + c.fs) return p.mat[M_WALL];   // the passage's roof
                return c.my >= r.base ? TERRAIN : PASS;
            }
            return AIR;
        }
        if (c.wy >= sf - c.fs) return p.mat[M_TRIM];                     // the steps
        return c.my >= r.base ? TERRAIN : PASS;
    }
    if (au == 2 && c.v >= V - 6 && c.v <= V) {   // the stair's side walls, and a low parapet round its head
        if (c.my < r.base) return (c.wy >= sf - c.fs && c.wy < sf + headroom + c.fs) ? p.mat[M_WALL] : PASS;
        if (c.my <= r.base + 1 && c.v >= V - 3) return p.mat[M_WALL];
        return TERRAIN;
    }
    const bool shell = au <= U - 1 && c.v >= -(V - 1) && c.v <= V - 7;
    if (shell && c.my >= floor_my && c.my <= ceil_my) {
        const bool room = au <= U - 2 && c.v >= -(V - 2) && c.v <= V - 8;
        if (c.my == floor_my) return p.mat[M_FLOOR];
        if (!room || c.my == ceil_my) return p.mat[M_WALL];
        // a stone coffin down the middle
        const float vc = 0.5f * (float)(-(V - 2) + (V - 8));
        const float fr = (float)(c.wy - (floor_my + 1) * c.fs) / (float)c.fs;
        if (std::fabs(c.fu) <= 0.9f && std::fabs(c.fv - vc) <= 1.9f && fr < 1.6f) return fr >= 1.4f ? p.mat[M_WALL] : p.mat[M_TRIM];
        // grit on the floor
        if (c.wy == (floor_my + 1) * c.fs && (hash(c.wx, c.wz, (int)r.seed + 43) & 7) == 0) return p.mat[M_EXTRA];
        return AIR;
    }
    if (c.my < r.base) return PASS;
    // headstones over the room
    if (c.v <= V - 9 && c.v >= -(V - 1) && au <= U - 1 && (c.u % 2) == 0 && ((c.v + V - 2) % 3) == 0 && (hash((int)r.seed, c.u * 31 + c.v, 49) & 3) != 0) {
        const float du = std::fabs(c.fu - (float)c.u), dv = std::fabs(c.fv - (float)c.v);
        if (du <= 0.42f && dv <= 0.2f && c.fy < 1.7f - (du > 0.25f ? 0.25f : 0.0f)) return p.mat[M_TRIM];
    }
    return TERRAIN;
}

uint16_t shrine(const Prepared &p, const Ctx &c) {
    const Ruin &r = p.r;
    const int U = p.U, P = U - 2, avv = -(U - 3);
    const int au = std::abs(c.u), av = std::abs(c.v), cheb = std::max(au, av);
    if (cheb > U - 1) {
        if (c.my == fdiv(c.surf, c.fs) && (hash(c.mx, c.mz, (int)r.seed + 5) & 15) == 0) return p.mat[M_WALL];
        return TERRAIN;
    }
    // the solid parts of this column and its top
    auto pillar_h = [&](int pu, int pv) { return pv < 0 ? 6 : 1 + (int)(hash((int)r.seed, pu, 51) % 4); };
    int top = r.base;   // step one
    if (cheb <= U - 2) top = r.base + 1;
    const bool pillar = au == P && av == P;
    const bool lintel = c.v == -P && au <= P;
    if (pillar) top = r.base + 1 + pillar_h(c.u, c.v);
    if (lintel) top = r.base + 8;
    const bool altar = c.u == 0 && c.v == avv;
    if (altar) top = r.base + 3;
    if (c.my == r.base) return p.mat[M_FLOOR];
    if (c.my == r.base + 1 && cheb <= U - 2) return p.mat[M_WALL];
    if (pillar && c.my <= top) return p.mat[M_WALL];
    if (altar && c.my <= top) return p.mat[M_WALL];
    if (lintel && c.my == r.base + 8) return p.mat[M_WALL];
    // overgrown: moss on every top, leaves over the pillars
    if (c.wy == (top + 1) * c.fs && (hash(c.wx, c.wz, (int)r.seed + 61) & 3) != 0) return p.mat[M_TRIM];
    for (int k = 0; k < 4; k++) {
        const int pu = (k & 1) ? P : -P, pv = (k & 2) ? P : -P;
        const float tu = c.fu - (float)pu, tv = c.fv - (float)pv;
        const float dy = c.fy - (float)(2 + pillar_h(pu, pv));
        const float d2 = tu * tu + tv * tv + dy * dy * 1.7f;
        if (d2 < 1.0f + 0.4f * (float)(hash(c.mx, c.mz, (int)r.seed + 63) & 1)) return p.mat[M_EXTRA];   // a tuft, solid (nothing loose)
        if (std::fabs(tu) <= 0.75f && std::fabs(tv) <= 0.75f && std::min(std::fabs(tu), std::fabs(tv)) <= 0.5f && dy > -3.0f && dy < 0.0f && (hash(c.wx, c.wy * 5 + c.wz, (int)r.seed + 65) & 3) == 0) return p.mat[M_EXTRA];
    }
    return TERRAIN;
}

uint16_t wreck(const Prepared &p, const Ctx &c) {
    const Ruin &r = p.r;
    const float L = (float)p.U - 0.3f, W = (float)p.V * 0.95f;
    const float keel = -2.0f, Dh = 4.5f, heel = 0.3f;
    // the mast's stump
    const float mu = (float)p.U * 0.2f;
    const float mh = 5.0f + (float)(hash((int)r.seed, 81, 1) % 3) * 0.5f;
    if ((c.fu - mu) * (c.fu - mu) + c.fv * c.fv < 0.16f && c.fy >= keel && c.fy < mh) return p.mat[M_TRIM];
    const float a = c.fu / L;
    if (std::fabs(a) >= 1.0f) return c.my >= r.base ? TERRAIN : PASS;
    const float w = W * (c.fu >= 0.0f ? std::sqrt(1.0f - a * a) : std::sqrt(1.0f - a * a * a * a));   // a fuller stern
    const float yk = c.fy - keel - c.fv * heel;                                                        // above the keel
    const float dh = Dh + std::max(0.0f, a) * 1.2f;                                                   // the bow's sheer
    if (yk < 0.0f || yk > dh) return c.my >= r.base ? TERRAIN : PASS;
    const float ws = w * std::sqrt(std::clamp(yk / 2.2f + 0.1f, 0.0f, 1.0f));
    if (std::fabs(c.fv) > ws) return c.my >= r.base ? TERRAIN : PASS;
    const bool shell = std::fabs(c.fv) > ws - 0.55f || yk < 0.55f || a < -0.95f;   // thick enough to join face to face round the curve
    const float fr = c.fu / 1.5f - std::round(c.fu / 1.5f);
    const bool rib = std::fabs(fr) * 1.5f < 0.18f && (std::fabs(c.fv) > ws - 0.75f || yk < 0.75f);
    if (rib) return p.mat[M_TRIM];
    if (shell) {
        // planks sprung by the sea: whole panels gone, more of them up the side it lies on
        // a panel spans rib to rib, so what is left of it always hangs on a rib
        const uint32_t panel = hash((int)std::floor(c.fu / 1.5f), (c.fv > 0.0f ? 7 : 0) + (int)std::floor(yk / 0.75f), (int)r.seed + 71);
        const bool gone = (panel & 3) == 0 || (yk > dh - 1.2f && c.fv > 0.0f && (panel & 4));
        if (!gone) return p.mat[M_WALL];
    }
    // deck beams across the hold
    const float fb = c.fu / 3.0f - std::round(c.fu / 3.0f);
    if (std::fabs(fb) * 3.0f < 0.3f && yk >= dh - 0.7f && yk < dh - 0.35f) return p.mat[M_TRIM];
    return c.my >= r.base ? TERRAIN : PASS;
}

} // namespace

uint16_t voxel(const Prepared &p, int fs, int wx, int wy, int wz, int surf, uint16_t ground, bool pots) {
    const Ruin &r = p.r;
    Ctx c;
    c.wx = wx; c.wy = wy; c.wz = wz; c.fs = fs; c.surf = surf;
    c.mx = fdiv(wx, fs); c.my = fdiv(wy, fs); c.mz = fdiv(wz, fs);
    to_uv(r.rot, c.mx - r.x, c.mz - r.z, c.u, c.v);
    const float inv = 1.0f / (float)fs;
    to_uvf(r.rot, ((float)wx + 0.5f) * inv - ((float)r.x + 0.5f), ((float)wz + 0.5f) * inv - ((float)r.z + 0.5f), c.fu, c.fv);
    c.fy = ((float)wy + 0.5f) * inv - (float)r.base;
    if (pots) {
        for (int i = 0; i < p.npots; i++) {
            const PotSpot &s = p.pots[i];
            if (s.mx != c.mx || s.mz != c.mz || wy < s.y * fs || wy >= s.y * fs + 7) continue;
            const uint16_t m = technosim_rock::RockTypes::pot_shape(s.h, wx - c.mx * fs, wz - c.mz * fs, wy - s.y * fs);
            if (m != AIR) return m;
        }
    }
    uint16_t m;
    switch (p.shape) {
        case S_WATCHTOWER: m = watchtower(p, c); break;
        case S_STONE_CIRCLE: m = stone_circle(p, c); break;
        case S_CRYPT: m = crypt(p, c); break;
        case S_MUD_HOUSE: m = mud_house(p, c); break;
        case S_SHRINE: m = shrine(p, c); break;
        case S_WRECK: m = wreck(p, c); break;
        default: m = house(p, c); break;
    }
    (void)ground;
    return m == TERRAIN ? PASS : m;   // the ground, and what grows over it, as FineGen makes it
}

} // namespace technosim_ruin
