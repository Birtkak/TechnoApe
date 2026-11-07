// GENERATED from game/data/ore_veins.json by game/tools/ore_vein_codegen.gd. Do not edit.
// GregTech-style ore veins (docs/design/progression.md "Ores"). Regenerate:
//   Godot_console.exe --headless --path game --script res://tools/ore_vein_codegen.gd
#pragma once

namespace technosim_ore {

static const int CELL = 192;               // voxels per vein cell (one vein at most)
static const float FILL = 0.55f;           // chance a cell holds a vein
static const float SPORADIC_CHANCE = 0.12f; // ore voxel -> sporadic ore
static const int INDICATOR_RADIUS = 44;   // voxels from the vein centre
static const int INDICATOR_MAX_DEPTH = 120; // vein top below the ground (voxels)
static const float INDICATOR_CHANCE = 0.01f; // per column at depth 0, fading to 0 at max depth

enum Id : int {
    COAL = 0,
    COPPER = 1,
    CASSITERITE = 2,
    IRON = 3,
    COPPER_IRON = 4,
    COUNT = 5
};
static const int WEIGHT_TOTAL = 100;
static const char *const NAMES[COUNT] = {"coal", "copper", "cassiterite", "iron", "copper_iron"};
static const int WEIGHT[COUNT] = {30, 25, 15, 20, 10};
static const int Y_MIN[COUNT] = {200, 160, 120, 40, 80};
static const int Y_MAX[COUNT] = {320, 280, 240, 180, 200};
static const int SIZE[COUNT] = {56, 44, 44, 44, 44};      // lens diameter (voxels)
static const int THICKNESS[COUNT] = {10, 8, 8, 8, 8}; // lens thickness at the centre (voxels)
static const int JITTER[COUNT] = {64, 70, 70, 70, 70};    // max centre offset, keeps the lens in its cell
static const float DENSITY[COUNT] = {0.6f, 0.6f, 0.6f, 0.6f, 0.6f};
// Material ids (WorldGen::Mat); -1 = none.
static const int PRIMARY[COUNT] = {16, 18, 19, 17, 17};
static const int BETWEEN[COUNT] = {16, 19, 18, 16, 18};
static const int SECONDARY[COUNT] = {16, 18, 19, 17, 18};
static const int SPORADIC[COUNT] = {17, 17, 16, 18, 19};
static const int POCKET_GAS[COUNT] = {133, -1, -1, -1, -1};       // ADR 0148: a lump of this gas instead of ore ...
static const float POCKET_GAS_CHANCE[COUNT] = {0.1f, 0.0f, 0.0f, 0.0f, 0.0f}; // ... with this chance per lump
static const int INDICATOR[COUNT] = {-1, 48, 49, 50, 50};

} // namespace technosim_ore
