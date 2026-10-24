# Patch for the custom godot_voxel build: liquid surfaces slope down to open air instead of
# standing as glass walls (P13 water look, Lay of the Land).
# Stock VoxelBlockyFluid raises each top corner to the highest of the four cells around it, so full
# water next to air stays flat up to a vertical see-through wall: every pool edge over a lower bank
# and every step of a river surface showed as a staircase of walls (bug report 2026-10-24, "voxel
# shaped holes on the bank"). Here a corner touching an open cell (air: no model or an empty one)
# drops to the cell's bottom: a step in a surface becomes a ramp that meets the lower
# surface (its edge rises to full height through the covered-neighbour rule), a pool over a lower
# bank slopes to its rim. Corners against solid ground stay where they were, so lakes meet their
# banks flat. The flow state follows the ramps (the shader scrolls down them).
# Run with the godot_voxel root as argv[1].
import sys
root = sys.argv[1]

p = root + '/meshers/blocky/blocky_fluids_meshing_impl.h'
s = open(p, encoding='utf-8').read()

old = '''		FixedArray<uint8_t, 9> fluid_levels;
		uint32_t covered_neighbors = 0;'''
new = '''		FixedArray<uint8_t, 9> fluid_levels;
		uint32_t covered_neighbors = 0;
		uint32_t open_neighbors = 0; // TechnoApe: air (no model or an empty one) beside the fluid
		uint32_t fluid_neighbors = 0; // TechnoApe: the same fluid beside it'''
assert s.count(old) == 1, s.count(old)
s = s.replace(old, new, 1)

old = '''					if (nm.fluid_index == voxel.fluid_index) {
						fluid_levels[i] = nm.fluid_level;
'''
new = '''					if (nm.fluid_index == voxel.fluid_index) {
						fluid_levels[i] = nm.fluid_level;
						fluid_neighbors |= (1 << i); // TechnoApe
'''
assert s.count(old) == 1, s.count(old)
s = s.replace(old, new, 1)

old = '''					} else {
						fluid_levels[i] = 0;
					}
				} else {
					fluid_levels[i] = 0;
				}

				++i;'''
new = '''					} else {
						fluid_levels[i] = 0;
						if (nm.empty) { // only air: a plant standing in the water is no hole in it
							open_neighbors |= (1 << i);
						}
					}
				} else {
					fluid_levels[i] = 0;
					open_neighbors |= (1 << i);
				}

				++i;'''
assert s.count(old) == 1, s.count(old)
s = s.replace(old, new, 1)

old = '''		const FixedArray<uint8_t, 4> corner_levels = get_corner_levels_from_fluid_levels(fluid_levels);'''
new = '''		FixedArray<uint8_t, 4> corner_levels = get_corner_levels_from_fluid_levels(fluid_levels);
		// TechnoApe: a corner drops to the very bottom when, in the 2x2 cells around it, an open cell sits
		// side by side with fluid (an open cell touching it only diagonally is hidden: no dimple). The
		// rule reads the same from each cell sharing the corner, so their surfaces stay joined.
		//  8 7 6     z      corner 1: diagonal 0, sides 1 3    corner 0: diagonal 2, sides 1 5
		//  5 4 3     |      corner 2: diagonal 6, sides 3 7    corner 3: diagonal 8, sides 5 7
		//  2 1 0  x--o
		open_neighbors &= ~covered_neighbors;
		FixedArray<bool, 4> open_corner;
		{
			const uint32_t o = open_neighbors, f = fluid_neighbors;
			auto drops = [o, f](int d, int a, int b) {
				return ((o >> a) & 1) || ((o >> b) & 1) || (((o >> d) & 1) && (((f >> a) & 1) || ((f >> b) & 1)));
			};
			open_corner[1] = drops(0, 1, 3);
			open_corner[0] = drops(2, 1, 5);
			open_corner[2] = drops(6, 3, 7);
			open_corner[3] = drops(8, 5, 7);
			for (int c = 0; c < 4; ++c) {
				if (open_corner[c]) {
					corner_levels[c] = 0;
				}
			}
		}'''
assert s.count(old) == 1, s.count(old)
s = s.replace(old, new, 1)

# open corners reach the cell bottom (level 0 alone is BOTTOM_HEIGHT, 1/16 up: a thin dark wall at each step)
old = '''		FixedArray<float, 4> corner_heights = get_corner_heights_from_corner_levels(corner_levels, fluid);'''
new = '''		FixedArray<float, 4> corner_heights = get_corner_heights_from_corner_levels(corner_levels, fluid);
		for (int c = 0; c < 4; ++c) {
			if (open_corner[c]) {
				corner_heights[c] = 0.f; // TechnoApe
			}
		}'''
assert s.count(old) == 1, s.count(old)
s = s.replace(old, new, 1)

open(p, 'w', encoding='utf-8', newline='\n').write(s)
print('patched')
