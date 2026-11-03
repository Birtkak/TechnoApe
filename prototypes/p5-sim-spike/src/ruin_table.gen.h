// GENERATED from game/data/ruins.json by game/tools/ruin_codegen.gd. Do not edit.
// Ruin kinds (ADR 0062). Regenerate:
//   Godot_console.exe --headless --path game --script res://tools/ruin_codegen.gd
#pragma once

#include <cstdint>

namespace technosim_ruin {

enum Shape : int { S_HOUSE, S_WATCHTOWER, S_STONE_CIRCLE, S_CRYPT, S_MUD_HOUSE, S_SHRINE, S_WRECK, S_MINE, S_CAMP, NSHAPE };
enum KindMat : int { M_WALL, M_TRIM, M_FLOOR, M_EXTRA };
static const int NKIND = 9;
static const char *const KIND_NAMES[NKIND] = {"house", "watchtower", "stone_circle", "crypt", "mud_house", "shrine", "mine", "camp", "wreck"};
static const int KIND_SHAPE[NKIND] = {S_HOUSE, S_WATCHTOWER, S_STONE_CIRCLE, S_CRYPT, S_MUD_HOUSE, S_SHRINE, S_MINE, S_CAMP, S_WRECK};
static const int KIND_WEIGHT[NKIND] = {10, 6, 5, 5, 10, 6, 5, 6, 1};
static const bool KIND_COAST[NKIND] = {false, false, false, false, false, false, false, false, true};
// bit b set = biome id b (world_gen.h / biome_table.gen.h) may hold the kind
static const uint64_t KIND_BIOMES[NKIND] = {0x00000060051a00d8ULL, 0x00000060011a00c8ULL, 0x00000040015200c8ULL, 0x00000060042c0098ULL, 0x0000000002500020ULL, 0x0000002004ac0010ULL, 0x00000020075c0098ULL, 0x0000006004fc0098ULL, 0x000001000000c006ULL};
// sizes in macro cells (0.5 m)
static const int KIND_HALF_U[NKIND][2] = {{5, 10}, {6, 7}, {10, 14}, {4, 5}, {5, 8}, {4, 5}, {4, 5}, {5, 6}, {7, 9}};
static const int KIND_HALF_V[NKIND][2] = {{5, 10}, {6, 7}, {10, 14}, {9, 10}, {4, 7}, {4, 5}, {13, 14}, {5, 6}, {3, 4}};
static const int KIND_TOP[NKIND] = {10, 15, 12, 3, 7, 10, 9, 4, 7};
static const int KIND_DEPTH[NKIND] = {0, 0, 0, 8, 0, 0, 9, 0, 3};
// material ids: wall, trim, floor, extra
static const uint16_t KIND_MAT[NKIND][4] = {{1, 22, 1, 109}, {109, 173, 22, 36}, {160, 46, 1, 160}, {173, 109, 177, 22}, {84, 78, 24, 21}, {173, 46, 109, 20}, {36, 234, 22, 22}, {109, 6, 8, 20}, {36, 6, 36, 21}};
static const int KIND_POTS[NKIND] = {4, 2, 2, 4, 4, 2, 3, 3, 3};

} // namespace technosim_ruin
