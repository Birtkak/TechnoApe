#pragma once
// Rock kinds in the world (ADR 0057): provinces of a few hundred metres, each a stack of tilted strata,
// so cliffs and cave walls show bands; pyrite nodules, ochre patches in the soil, ore bias per host
// rock; and the clutter pieces, cave crystals and ruin pots of part E. FineGen calls this through a few
// small hooks (generate_block, floor_features, clutter_voxel); the data is rock_table.gen.h, generated
// from game/data/rock_types.json. Deterministic from the seed; no Godot types.

#include <cstdint>

#include "FastNoiseLite.h"
#include "rock_table.gen.h"

namespace technosim_rock {

// The strata at one column: its province and the boundaries between its layers (absolute voxel y);
// layer k lies above b[k] (and below b[k - 1]), the last layer goes down to bedrock.
struct RockCol {
    int prov = 0;
    float b[MAX_LAYERS - 1] = {};
};

class RockTypes {
public:
    enum { KIND_BASE = 100 };   // FineGen FStamp kinds of these clutter pieces: KIND_BASE + Shape

    void configure(int seed, int sea_voxels);

    // Strata of a macro lattice point (fine coordinates), h = its surface height in voxels.
    RockCol column(int wx, int wz, float h) const;
    // A fine column between four lattice corners: boundaries interpolated, the province of `near`
    // (the corner FineGen picks for the column's other discrete attributes).
    static RockCol blend(const RockCol &c00, const RockCol &c10, const RockCol &c01, const RockCol &c11, float tx, float tz, const RockCol &near);

    int rock_at(const RockCol &c, int wy) const;                          // Rock index
    uint16_t at(const RockCol &c, int wx, int wy, int wz) const;          // material of stone at a voxel (pyrite nodules too)
    uint16_t loose(const RockCol &c, int h) const;                        // loose stones lying on this column's surface rock
    uint16_t soil(const RockCol &c, uint16_t under, int h, int wx, int wz) const;   // ochre patches in place of dirt
    float ore_bias(const RockCol &c, int wy, int vein) const;
    // A deep block filled with one rock: true and its material when every corner column holds the same
    // layer of the same province over [y0, y1).
    bool uniform(const RockCol *cols, int n, int y0, int y1, uint16_t &out) const;

    // Clutter (one roll per FineGen clutter cell, hs its hash): kind KIND_BASE + shape and material.
    bool clutter_pick(int biome, bool beach, bool snowy, bool forest, uint32_t hs, int &kind, uint16_t &mat) const;
    // The piece's voxel at (dx, dy, dz) from its centre column and ground (dy = 0 the first voxel above).
    static uint16_t clutter_voxel(int kind, uint32_t seed, uint16_t mat, int dx, int dy, int dz);

    // Cave crystals: a cluster's column height at (wx, wz) (0 = none) and the cluster's roll (0..999).
    int crystal_height(int wx, int wz, int &roll) const;
    static bool crystal_allowed(int roll, int depth, int cave_biome);

    // Ruin pots: the material of voxel (lx, lz) (0..3) of a pot's 0.5 m cell, dy voxels above the floor
    // it stands on; h = the pot's seed (its look). AIR outside the pot. Where pots stand: ruins.cpp.
    static uint16_t pot_shape(uint32_t h, int lx, int lz, int dy);

private:
    int seed_ = 0, sea_ = 0;
    FastNoiseLite n_warp_, n_thick_, n_wob_, n_ochre_;
};

} // namespace technosim_rock
