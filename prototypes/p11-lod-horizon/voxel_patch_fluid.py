# Patch for the custom godot_voxel build: liquid surfaces are one height field (ADR 0069, part A).
# Stock VoxelBlockyFluid raises each top corner to the highest of the four cells around it, so full
# water next to air stands as a glass wall; our first patch (ADR 0045) dropped a corner next to open air
# to the cell's bottom instead. That is right for a single step (a pool's rim meets the lower surface),
# but a river slope is a step in every cell, so each cell dropped a different set of corners, cells did
# not agree on the height of the corner they share, and every disagreement drew a wedge (bug report
# 2026-10-26 15-53-53, "all these triangles in fluids").
# Now a corner's height is the mean of the surface tops of the up-to-four columns around it. A column's
# top is its highest cell of the fluid within two cells up or down of the cell being meshed, whatever sits at that
# height (water over a shelf of ground counts); an open column with no fluid in reach votes the bottom of the lowest
# fluid cell around that corner (a lip over a drop, the same from every viewer), a solid or foreign column with none
# does not vote. A column whose fluid stands three or more cells above a viewer is out of reach: such terraces (a
# generator matter) draw as vertical water faces, not as a sheet. Every cell sharing a corner computes the same
# number, so the surface is continuous: a slope is a sheet, a filled crater is flat. The flow state comes
# from the corner heights, so the shader runs downhill on every slope. A side face toward a column whose
# fluid is one cell lower is collapsed (the lower surface rises to meet this one, the face would sit
# inside the water). A cell capped by a solid block keeps the stock corners (its top is hidden; dropped
# corners drew spikes out of cave walls, bug report 2026-10-25 02-02-16). A lone cell (no fluid beside it,
# open air beside it with nothing below) lies flat as a film instead of a spike.
# The reach is two cells up and down: on a diagonal slope the columns around one corner span three heights, and
# with a one-cell reach the lowest cell could not see the highest column's top (seams of half a cell, measured by
# tools/water_surface_probe.gd). So the blocky mesher's PADDING goes from 1 to 2 (every blocky loop reads it).
# Run with the godot_voxel root as argv[1] on the stock files (git checkout them first).
import sys
root = sys.argv[1]

ph = root + '/meshers/blocky/voxel_mesher_blocky.h'
h = open(ph, encoding='utf-8').read()
old = 'static const int PADDING = 1;'
new = 'static const int PADDING = 6; // TechnoApe: the fluid height field reads six cells up and down (ADR 0069 A3)'
assert h.count(old) == 1, h.count(old)
open(ph, 'w', encoding='utf-8', newline='\n').write(h.replace(old, new, 1))

p = root + '/meshers/blocky/blocky_fluids_meshing_impl.h'
s = open(p, encoding='utf-8').read()

old = '''		const FixedArray<uint8_t, 4> corner_levels = get_corner_levels_from_fluid_levels(fluid_levels);
		const VoxelBlockyFluid::FlowState flow_state = get_fluid_flow_state_from_corner_levels(corner_levels);'''
new = '''		const FixedArray<uint8_t, 4> corner_levels = get_corner_levels_from_fluid_levels(fluid_levels);
		// TechnoApe (ADR 0069): one height field. Each corner is the mean of the surface tops of the columns
		// around it, read the same way from every cell that shares it (see voxel_patch_fluid.py).
		//  8 7 6     z      corner 0 (+x,-z): columns 1 2 4 5    corner 1 (-x,-z): columns 0 1 3 4
		//  5 4 3     |      corner 2 (-x,+z): columns 3 4 6 7    corner 3 (+x,+z): columns 4 5 7 8
		//  2 1 0  x--o
		const bool field = !(library.has_model(top_voxel_id) && !library.models[top_voxel_id].empty);
		FixedArray<float, 4> field_heights;
		uint32_t step_sides = 0; // side columns (1 3 5 7) holding this fluid one cell lower: no side face there
		VoxelBlockyFluid::FlowState field_flow = VoxelBlockyFluid::FLOW_IDLE;
		if (field) {
			const float max_level_inv = 1.f / static_cast<float>(fluid.max_level);
			auto l2h = [max_level_inv](uint8_t level) {
				return math::lerp(BakedFluid::BOTTOM_HEIGHT, BakedFluid::TOP_HEIGHT, static_cast<float>(level) * max_level_inv);
			};
			FixedArray<float, 9> tops;
			uint32_t votes = 0, same = 0, dry_open = 0;
			unsigned int k = 0;
			for (int dz = -1; dz <= 1; ++dz) {
				for (int dx = -1; dx <= 1; ++dx, ++k) {
					const uint32_t nloc = voxel_index + dx * x_jump_size + dz * z_jump_size;
					tops[k] = 0.f;
					// the column's vote: its highest cell of this fluid within two cells up or down, whatever sits at
					// this height (water over a shelf of ground is still that column's surface)
					bool found = false;
					for (int dy = VoxelMesherBlocky::PADDING; dy >= -VoxelMesherBlocky::PADDING && !found; --dy) {
						const uint32_t cid = type_buffer[nloc + dy * static_cast<int>(y_jump_size)];
						if (library.has_model(cid)) {
							const BakedModel &cm = library.models[cid];
							if (cm.fluid_index == voxel.fluid_index) {
								tops[k] = static_cast<float>(dy) + l2h(cm.fluid_level);
								votes |= 1u << k;
								if (dy < 0) {
									step_sides |= 1u << k;
								}
								found = true;
							}
						}
					}
					const uint32_t nid = type_buffer[nloc];
					const bool has = library.has_model(nid);
					if (has && library.models[nid].fluid_index == voxel.fluid_index) {
						same |= 1u << k;
					}
					if (!found && (!has || library.models[nid].empty)) {
						// open air with no fluid in reach: the surface drops here, to the bottom of the lowest fluid
						// cell around the corner (decided per corner below: the same from every viewer)
						dry_open |= 1u << k;
					}
					// solid, a plant or another fluid with no fluid above or below in reach: no vote, the surface keeps
					// its level there (a lake meets its bank flat)
				}
			}
			static const uint8_t CORNER_COLS[4][4] = { { 1, 2, 4, 5 }, { 0, 1, 3, 4 }, { 3, 4, 6, 7 }, { 4, 5, 7, 8 } };
			const bool lone = (same & 0b010'101'010) == 0 && (dry_open & 0b010'101'010) != 0;
			for (int c = 0; c < 4; ++c) {
				float sum = 0.f;
				float low = 3.f; // bottom of the lowest fluid cell voting at this corner (its own cell always votes)
				float hi_floor = -100.f; // bottom of the highest voting fluid cell: the ground under it reaches there
				int n = 0;
				for (int j = 0; j < 4; ++j) {
					const unsigned int col = CORNER_COLS[c][j];
					if ((votes >> col) & 1u) {
						sum += tops[col];
						low = math::min(low, Math::floor(tops[col]));
						hi_floor = math::max(hi_floor, Math::floor(tops[col]));
						++n;
					}
				}
				for (int j = 0; j < 4; ++j) {
					const unsigned int col = CORNER_COLS[c][j];
					if ((dry_open >> col) & 1u) {
						sum += low;
						++n;
					}
				}
				// ADR 0069 A2: a wider, gentler mean. The 2x2 mean above weighs 9 per column; the ring of twelve
				// columns around those four adds its votes with weight 3 (edge) or 1 (diagonal), a 1 3 3 1 kernel
				// on each axis. Dry and foreign columns in the ring do not vote (the rim rule stays the 2x2's).
				// Every cell sharing the corner sees the same 4x4 columns, so the surface stays one sheet.
				const int sx = (c == 0 || c == 3) ? 1 : -1;
				const int sz = (c == 2 || c == 3) ? 1 : -1;
				float ring = 0.f, ring_w = 0.f;
				for (int b = 0; b < 4; ++b) {
					for (int a = 0; a < 4; ++a) {
						if (a >= 1 && a <= 2 && b >= 1 && b <= 2) {
							continue;
						}
						const int dx = sx > 0 ? a - 1 : a - 2;
						const int dz = sz > 0 ? b - 1 : b - 2;
						const float w = static_cast<float>(((a == 0 || a == 3) ? 1 : 3) * ((b == 0 || b == 3) ? 1 : 3));
						const uint32_t nloc = voxel_index + dx * x_jump_size + dz * z_jump_size;
						for (int dy = VoxelMesherBlocky::PADDING; dy >= -VoxelMesherBlocky::PADDING; --dy) {
							const uint32_t cid = type_buffer[nloc + dy * static_cast<int>(y_jump_size)];
							if (library.has_model(cid) && library.models[cid].fluid_index == voxel.fluid_index) {
								ring += w * (static_cast<float>(dy) + l2h(library.models[cid].fluid_level));
								ring_w += w;
								break;
							}
						}
					}
				}
				// ADR 0069 A3: never below the bottom of the highest water cell at this corner. On a cascade the mean of
				// a three-cell step lies under the higher column's ground, and every stair nose poked through the sheet
				// as a diagonal slit. Every sharing cell computes the same floor, so the sheet stays one surface
				const float mean = math::max((36.f * (sum / static_cast<float>(n)) + ring) / (36.f + ring_w), hi_floor);
				field_heights[c] = lone ? 0.f : math::clamp(mean, -float(VoxelMesherBlocky::PADDING), float(VoxelMesherBlocky::PADDING) + 1.f);
			}
			const float h_min = math::min(field_heights[0], field_heights[1], field_heights[2], field_heights[3]);
			const uint8_t mask = //
					(static_cast<uint8_t>(field_heights[0] <= h_min + 0.001f) << 3) | //
					(static_cast<uint8_t>(field_heights[1] <= h_min + 0.001f) << 2) | //
					(static_cast<uint8_t>(field_heights[2] <= h_min + 0.001f) << 1) | //
					(static_cast<uint8_t>(field_heights[3] <= h_min + 0.001f) << 0);
			field_flow = g_min_corners_mask_to_flowstate[mask];
		}
		const VoxelBlockyFluid::FlowState flow_state =
				field ? field_flow : get_fluid_flow_state_from_corner_levels(corner_levels);'''
assert s.count(old) == 1, s.count(old)
s = s.replace(old, new, 1)

old = '''		FixedArray<float, 4> corner_heights = get_corner_heights_from_corner_levels(corner_levels, fluid);

		//  8 7 6     z
		//  5 4 3     |
		//  2 1 0  x--o
		// Covered neighbors need to be considered at full height
		if ((covered_neighbors & 0b000'001'011) != 0) {
			corner_heights[1] = 1.f;
		}
		if ((covered_neighbors & 0b000'100'110) != 0) {
			corner_heights[0] = 1.f;
		}
		if ((covered_neighbors & 0b011'001'000) != 0) {
			corner_heights[2] = 1.f;
		}
		if ((covered_neighbors & 0b110'100'000) != 0) {
			corner_heights[3] = 1.f;
		}
'''
new = '''		FixedArray<float, 4> corner_heights =
				field ? field_heights : get_corner_heights_from_corner_levels(corner_levels, fluid);

		//  8 7 6     z
		//  5 4 3     |
		//  2 1 0  x--o
		// Covered neighbors need to be considered at full height (stock rule, capped cells only: TechnoApe)
		if (!field) {
			if ((covered_neighbors & 0b000'001'011) != 0) {
				corner_heights[1] = 1.f;
			}
			if ((covered_neighbors & 0b000'100'110) != 0) {
				corner_heights[0] = 1.f;
			}
			if ((covered_neighbors & 0b011'001'000) != 0) {
				corner_heights[2] = 1.f;
			}
			if ((covered_neighbors & 0b110'100'000) != 0) {
				corner_heights[3] = 1.f;
			}
		}
'''
assert s.count(old) == 1, s.count(old)
s = s.replace(old, new, 1)

old = '''		// For the top side, we assume vertices are counter-clockwise, and the first is at (+x, -z)'''
new = '''		// TechnoApe: no side face toward a column whose fluid lies one cell lower; the lower surface rises to
		// meet this one at the shared corners, and the face would stand inside the water.
		if (field) {
			static const Cube::SideAxis STEP_SIDE[4] = { Cube::SIDE_NEGATIVE_Z, Cube::SIDE_NEGATIVE_X, Cube::SIDE_POSITIVE_X, Cube::SIDE_POSITIVE_Z };
			static const unsigned int STEP_COL[4] = { 1, 3, 5, 7 };
			for (int j = 0; j < 4; ++j) {
				if ((step_sides >> STEP_COL[j]) & 1u) {
					BakedModel::SideSurface &side_surface = fluid_sides[STEP_SIDE[j]][surface_index];
					side_surface.positions[2].y = 0.f;
					side_surface.positions[3].y = 0.f;
				}
			}
		}
		// For the top side, we assume vertices are counter-clockwise, and the first is at (+x, -z)'''
assert s.count(old) == 1, s.count(old)
s = s.replace(old, new, 1)

# ADR 0069 A3: cells under the top hide side faces toward a lower surface in reach (shards on cascades)
old = '''	if (fluid_top_covered) {
		// No top side'''
new = '''	if (fluid_top_covered) {
		// TechnoApe (ADR 0069 A3): a cell under the column's top cell hides its side face toward a column whose
		// fluid surface lies lower within reach: the height field's sheet slopes down over it, and the face stuck
		// up through the sheet as a row of shards on every cascade. A true drop (no fluid in reach) keeps its face.
		static const Cube::SideAxis CSIDE[4] = { Cube::SIDE_NEGATIVE_Z, Cube::SIDE_NEGATIVE_X, Cube::SIDE_POSITIVE_X, Cube::SIDE_POSITIVE_Z };
		static const int CDX[4] = { 0, -1, 1, 0 };
		static const int CDZ[4] = { -1, 0, 0, 1 };
		for (int j = 0; j < 4; ++j) {
			const int nloc = voxel_index + CDX[j] * x_jump_size + CDZ[j] * z_jump_size;
			const uint32_t nid = type_buffer[nloc];
			if (library.has_model(nid) && !library.models[nid].empty) {
				continue;
			}
			for (int dy = -1; dy >= -VoxelMesherBlocky::PADDING; --dy) {
				const uint32_t cid = type_buffer[nloc + dy * y_jump_size];
				if (library.has_model(cid) && library.models[cid].fluid_index == voxel.fluid_index) {
					fluid_sides[CSIDE[j]][surface_index].clear();
					break;
				}
			}
		}
	}

	if (fluid_top_covered) {
		// No top side'''
assert s.count(old) == 1, s.count(old)
s = s.replace(old, new, 1)

open(p, 'w', encoding='utf-8', newline='\n').write(s)
print('patched')
