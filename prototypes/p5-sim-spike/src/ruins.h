#pragma once
// Ruins (ADR 0062): what kind stands in a ruin cell and the voxels of each kind's shape. WorldGen picks
// the place, size and kind (ruin_at_cell, from the data in ruin_table.gen.h, generated from
// game/data/ruins.json); FineGen flattens the ground under it and asks voxel() for every voxel of its
// region. Shapes are laid out in 0.5 m macro cells on the ruin's own axes (u, v), turned by rot; a few
// (standing stones, the wreck's hull, the crypt's stairs) are shaped per 12.5 cm voxel. Pots: at most
// KIND_POTS (<= 4) spots per ruin, never side by side. Deterministic; no Godot types.

#include <cstdint>

#include "ruin_table.gen.h"

namespace technosim_ruin {

// A ruin in macro cells: centre (x, z), half extents along world x / z (hw, hd), floor level base.
struct Ruin {
    int x, z, hw, hd, base;
    uint32_t seed;
    int kind = 0;    // index into the KIND_* tables
    int rot = 0;     // 0..3: quarter turns of the shape's (u, v) axes
    int top = 10;    // macro cells above base the shape may reach
    int depth = 0;   // macro cells below base the shape may reach
};

struct PotSpot { int mx, mz, y; uint32_t h; };   // macro cell, the macro y it stands on, its seed

// Everything voxel() needs for one ruin, worked out once per block.
struct Prepared {
    Ruin r;
    int shape = 0;
    int U = 0, V = 0;                 // half extents on the shape's own axes
    uint16_t mat[4] = {};             // wall, trim, floor, extra
    PotSpot pots[4];
    int npots = 0;
    // stone circle: up to 12 stones (centre u, v; radial direction; height; fallen)
    int nstones = 0;
    float st_u[12] = {}, st_v[12] = {}, st_c[12] = {}, st_s[12] = {}, st_h[12] = {};
    bool st_fallen[12] = {};
};

static constexpr uint16_t PASS = 0xFFFF;   // voxel(): leave it to the normal generator

uint32_t hash(int a, int b, int c);
// The kind for a ruin centred on biome `biome` (coast: a shore 0.5-2 m above the sea), -1 = none.
int pick_kind(int biome, bool coast, uint32_t h);
// Fills size, rotation, reach of r from its kind and seed (r.kind and r.seed set).
void shape_ruin(Ruin &r);
// Up to 4 pot spots of the ruin (at most KIND_POTS of its kind); returns the count.
int pot_spots(const Ruin &r, PotSpot out[4]);
void prepare(const Ruin &r, Prepared &p);
// Whether macro y `my` is inside the ruin's vertical reach.
inline bool in_reach(const Ruin &r, int my) { return my >= r.base - r.depth && my <= r.base + r.top; }
// The material of fine voxel (wx, wy, wz) of the ruin's region (its footprint plus 2 macro cells),
// fs = fine voxels per macro cell, surf = the fine surface height of the column (rubble lies on it),
// ground = its top material (unused), pots = place pot voxels (LOD 0). PASS wherever the shape leaves
// the ground, and what grows on it, to FineGen.
uint16_t voxel(const Prepared &p, int fs, int wx, int wy, int wz, int surf, uint16_t ground, bool pots);

} // namespace technosim_ruin
