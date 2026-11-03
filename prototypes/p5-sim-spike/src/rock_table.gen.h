// GENERATED from game/data/rock_types.json by game/tools/rock_codegen.gd. Do not edit.
// Rock provinces, strata, ore bias and clutter pieces (ADR 0057). Regenerate:
//   Godot_console.exe --headless --path game --script res://tools/rock_codegen.gd
#pragma once

namespace technosim_rock {

static const int PROVINCE_CELL = 3360;   // voxels
static const int WARP = 720;            // voxels
static const int MAX_LAYERS = 4;
enum Rock : int { R_STONE, R_GRANITE, R_DIORITE, R_LIMESTONE, R_SLATE, R_MARBLE, R_CHALK, R_BASALT, NROCK };
static const int ROCK_MAT[NROCK] = {1, 160, 161, 162, 163, 164, 165, 85};
static const int LOOSE_MAT[NROCK] = {37, 181, 182, 183, 184, 185, 186, 187};

static const int NPROV = 6;
static const int PROV_WEIGHT_TOTAL = 18;
static const char *const PROV_NAMES[NPROV] = {"grey_hills", "granite_shield", "limestone_basin", "chalk_downs", "volcanic", "metamorphic"};
static const int PROV_WEIGHT[NPROV] = {5, 3, 4, 2, 2, 2};
static const int PROV_TOP[NPROV] = {96, 32, 64, 48, 80, 48};      // top boundary above sea level (voxels)
static const float PROV_FOLLOW[NPROV] = {0.6f, 0.6f, 0.6f, 0.6f, 0.6f, 0.6f}; // share of the land's height the boundaries rise with
static const float PROV_DIP[NPROV] = {0.06f, 0.04f, 0.08f, 0.05f, 0.03f, 0.14f};
static const float PROV_WOBBLE[NPROV] = {16.0f, 24.0f, 12.0f, 12.0f, 32.0f, 20.0f};  // voxels
static const int PROV_NLAYERS[NPROV] = {2, 2, 4, 3, 3, 3};
static const int LAYER_ROCK[NPROV][MAX_LAYERS] = {{R_STONE, R_GRANITE, R_GRANITE, R_GRANITE}, {R_GRANITE, R_DIORITE, R_DIORITE, R_DIORITE}, {R_LIMESTONE, R_SLATE, R_STONE, R_GRANITE}, {R_CHALK, R_LIMESTONE, R_STONE, R_STONE}, {R_BASALT, R_STONE, R_DIORITE, R_DIORITE}, {R_SLATE, R_MARBLE, R_DIORITE, R_DIORITE}};
static const int THICK_MIN[NPROV][MAX_LAYERS] = {{240, 0, 0, 0}, {320, 0, 0, 0}, {96, 64, 160, 0}, {80, 80, 0, 0}, {160, 160, 0, 0}, {64, 80, 0, 0}};  // voxels
static const int THICK_MAX[NPROV][MAX_LAYERS] = {{560, 0, 0, 0}, {720, 0, 0, 0}, {280, 200, 400, 0}, {240, 200, 0, 0}, {440, 320, 0, 0}, {176, 240, 0, 0}};

// ORE_BIAS[rock][vein]: factor on the vein's nugget density in that host rock
static const int NVEIN = 5;
static const float ORE_BIAS[NROCK][NVEIN] = {{1.0f, 1.2f, 1.0f, 1.0f, 1.0f}, {0.4f, 1.0f, 2.0f, 0.6f, 1.0f}, {0.5f, 1.4f, 1.2f, 1.0f, 1.0f}, {1.6f, 0.7f, 0.4f, 1.3f, 1.0f}, {1.6f, 1.0f, 1.0f, 1.5f, 1.2f}, {0.6f, 1.0f, 1.0f, 0.7f, 1.0f}, {1.3f, 0.5f, 0.3f, 1.0f, 0.5f}, {0.5f, 1.6f, 0.5f, 1.2f, 1.6f}};

static const int PYRITE = 200, RED_OCHRE = 201, YELLOW_OCHRE = 202, CAVE_CRYSTAL = 197;
static const int CLAY_POT = 198, CLAY_POT_CRACKED = 199;
static const int PYRITE_PER_MILLE = 6;
static const unsigned PYRITE_ROCKS = 80u;  // bit per Rock
static const unsigned OCHRE_ROCKS = 88u;
static const float OCHRE_SHARE = 0.08f;
static const int BED_PARTNER[NROCK] = {R_DIORITE, R_DIORITE, R_GRANITE, R_CHALK, R_STONE, R_LIMESTONE, R_LIMESTONE, R_STONE};
static const int BED_BAND = 12;   // voxels
static const float BED_SHARE = 0.35f;

enum Where : int { W_ANY, W_OPEN, W_FOREST, W_BEACH, W_SNOW };
enum Shape : int { S_STRAWBERRY, S_NUTS, S_PINE_CONE, S_BONES, S_FEATHER, S_DRIFTWOOD, S_ANTLER, S_SHARD };
static const int NCLUT = 9;
static const int CLUT_MAT[NCLUT] = {188, 189, 190, 193, 191, 192, 194, 195, 196};
static const int CLUT_SHAPE[NCLUT] = {S_STRAWBERRY, S_NUTS, S_NUTS, S_PINE_CONE, S_BONES, S_FEATHER, S_DRIFTWOOD, S_ANTLER, S_SHARD};
static const int CLUT_WHERE[NCLUT] = {W_OPEN, W_FOREST, W_FOREST, W_FOREST, W_ANY, W_ANY, W_BEACH, W_FOREST, W_SNOW};
static const int NBIOME = 41;
static const float CLUT_CHANCE[NCLUT][NBIOME] = {{0.0f, 0.0f, 0.0f, 0.006f, 0.006f, 0.0f, 0.0f, 0.006f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.006f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.006f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.006f, 0.006f, 0.0f, 0.0f}, {0.0f, 0.0f, 0.0f, 0.0f, 0.005f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.005f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.005f, 0.0f, 0.0f, 0.0f}, {0.0f, 0.0f, 0.0f, 0.0f, 0.008f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.008f, 0.0f, 0.0f, 0.0f, 0.0f, 0.008f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f}, {0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.02f, 0.02f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f}, {0.0f, 0.0f, 0.0f, 0.002f, 0.0f, 0.002f, 0.002f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.002f, 0.0f, 0.0f, 0.002f, 0.0f, 0.002f, 0.0f, 0.0f, 0.002f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.002f, 0.0f}, {0.0f, 0.0f, 0.0025f, 0.0025f, 0.0025f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0025f, 0.0025f, 0.0025f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0025f, 0.0f, 0.0025f}, {0.0f, 0.02f, 0.02f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.02f, 0.02f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.02f}, {0.0f, 0.0f, 0.0f, 0.0f, 0.0015f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0015f, 0.0015f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0015f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0015f, 0.0f, 0.0f, 0.0f}, {0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.01f, 0.0f, 0.0f, 0.01f, 0.0f, 0.01f, 0.0f, 0.01f, 0.01f, 0.01f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.01f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f}};

static const int CRYSTAL_PER_MILLE = 30, CRYSTAL_CAVE_PER_MILLE = 250;
static const int CRYSTAL_MIN_DEPTH = 96;  // voxels
static const int CRYSTAL_CAVE_BIOME = 34;

} // namespace technosim_rock
