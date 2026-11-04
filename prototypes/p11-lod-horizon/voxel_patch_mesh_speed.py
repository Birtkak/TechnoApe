# --- a cheaper blocky mesher for the flight's surface blocks (ADR 0135) ---
# Part of the TechnoApe godot_voxel patches: run after voxel_patch_ao_pages.py (before voxel_patch_throughput.py).
# A flight at 20 m/s meshes ~160 surface mesh blocks (64^3) a second at ~16 ms each: as much worker time as all of
# generation. Two parts of VoxelMesherBlocky's generate_mesh, both with the same triangles as before:
# - the merge pass (ADR 0106) scanned every voxel of the padded block once per side (6 x 76^3 mask cells) to find
#   ~3 000 marked faces, and cleared the 2.6 MB mask first: marked faces are now listed per side as they are marked
#   and visited in the full scan's order (sorted by slice, row, column), and the mask is only grown (the merge pass
#   zeroes every mark it uses, so it is all zero again at the end);
# - a plain opaque cube (no inside geometry, not a fluid, no cutouts, culls neighbours, transparency 0, the same
#   side pattern on all six sides) between six such neighbours draws nothing: it is skipped with seven byte lookups
#   instead of six library visibility checks.
import sys
root = sys.argv[1]

def patch(rel, old, new):
    p = root + '/' + rel
    s = open(p, encoding='utf-8').read()
    if new in s:   # already applied
        return
    assert s.count(old) == 1, (rel, old)
    s = s.replace(old, new, 1)
    open(p, 'w', encoding='utf-8', newline='\n').write(s)

B = 'meshers/blocky/voxel_mesher_blocky.cpp'
patch(B, '''// per model, the sides that may merge (0xff = not computed yet)''', '''// TechnoApe ADR 0135: per side, the voxel indices of the faces marked for the merge pass
FixedArray<StdVector<uint64_t>, Cube::SIDE_COUNT> &get_tls_merge_marked() {
	static thread_local FixedArray<StdVector<uint64_t>, Cube::SIDE_COUNT> tls_merge_marked;
	return tls_merge_marked;
}

// TechnoApe ADR 0135: by voxel id, 1 = a plain opaque cube (see build_plain_cubes)
StdVector<uint8_t> &get_tls_plain_cubes() {
	static thread_local StdVector<uint8_t> tls_plain_cubes;
	return tls_plain_cubes;
}

// Models between which no face is ever drawn: not empty, culls neighbours, transparency 0, no inside geometry, not a
// fluid, no cutouts, and every side with one shared pattern (two such neighbours have equal patterns on the faces they
// share, so is_face_visible() is false for both). 65536 entries so any 16-bit id indexes it.
void build_plain_cubes(const BakedLibrary &library, StdVector<uint8_t> &plain) {
	plain.assign(65536, 0);
	int pattern = -1;
	for (unsigned int id = 0; id < library.models.size() && id < plain.size(); ++id) {
		const BakedModel &v = library.models[id];
		const BakedModel::Model &m = v.model;
		if (v.empty || !v.culls_neighbors || v.transparency_index != 0 || v.fluid_index != NULL_FLUID_INDEX ||
			v.cutout_sides_enabled || m.full_sides_mask != 0x3f) {
			continue;
		}
		bool inside = false;
		for (unsigned int s = 0; s < m.surface_count; ++s) {
			inside = inside || m.surfaces[s].positions.size() != 0;
		}
		bool same = true;
		for (unsigned int side = 1; side < Cube::SIDE_COUNT; ++side) {
			same = same && m.side_pattern_indices[side] == m.side_pattern_indices[0];
		}
		if (inside || !same) {
			continue;
		}
		if (pattern < 0) {
			pattern = int(m.side_pattern_indices[0]);
		}
		plain[id] = int(m.side_pattern_indices[0]) == pattern ? 1 : 0;
	}
}

// per model, the sides that may merge (0xff = not computed yet)''')
patch(B, '''	if (merging) {
		merge_mask.assign(type_buffer.size() * Cube::SIDE_COUNT, 0);
		merge_sides.assign(library.models.size(), 0xff);
	}''', '''	// TechnoApe ADR 0135: the merge pass zeroes every mark it uses, so the mask only grows (it was cleared each time)
	FixedArray<StdVector<uint64_t>, Cube::SIDE_COUNT> &merge_marked = get_tls_merge_marked();
	if (merging) {
		if (merge_mask.size() < type_buffer.size() * Cube::SIDE_COUNT) {
			merge_mask.resize(type_buffer.size() * Cube::SIDE_COUNT, 0);
		}
		merge_sides.assign(library.models.size(), 0xff);
		for (unsigned int side = 0; side < Cube::SIDE_COUNT; ++side) {
			merge_marked[side].clear();
		}
	}
	StdVector<uint8_t> &plain = get_tls_plain_cubes();
	build_plain_cubes(library, plain);''')
patch(B, '''				if (voxel_id == AIR_ID || !library.has_model(voxel_id)) {
					continue;
				}
''', '''				if (voxel_id == AIR_ID || !library.has_model(voxel_id)) {
					continue;
				}
				// TechnoApe ADR 0135: buried in plain cubes, nothing to draw
				if (plain[voxel_id] && plain[type_buffer[voxel_index + 1]] && plain[type_buffer[voxel_index - 1]] &&
					plain[type_buffer[voxel_index + row_size]] && plain[type_buffer[voxel_index - row_size]] &&
					plain[type_buffer[voxel_index + deck_size]] && plain[type_buffer[voxel_index - deck_size]]) {
					continue;
				}
''')
patch(B, '''								merge_mask[voxel_index * Cube::SIDE_COUNT + side] = code;
								continue;''', '''								merge_mask[voxel_index * Cube::SIDE_COUNT + side] = code;
								merge_marked[side].push_back(voxel_index); // TechnoApe ADR 0135
								continue;''')
patch(B, '''		int p[3];
		for (p[na] = lo[na]; p[na] < hi[na]; ++p[na]) {
			for (p[va] = lo[va]; p[va] < hi[va]; ++p[va]) {
				for (p[ua] = lo[ua]; p[ua] < hi[ua]; ++p[ua]) {
					const int i0 = p[0] * row_size + p[1] + p[2] * deck_size;
					const uint8_t code = merge_mask[i0 * ns + side];
					if (code == 0) {
						continue;
					}''', '''		// TechnoApe ADR 0135: only the marked faces, in the full scan's order (slice na, row va, column ua)
		StdVector<uint64_t> &marked = merge_marked[side];
		for (uint64_t &m : marked) {
			const uint32_t i = uint32_t(m);
			const int q[3] = { int(i / row_size) % block_size.x, int(i % row_size), int(i / deck_size) };
			m = (uint64_t(q[na]) << 52) | (uint64_t(q[va]) << 42) | (uint64_t(q[ua]) << 32) | i;
		}
		std::sort(marked.begin(), marked.end());
		for (const uint64_t m : marked) {
			const int i0 = int(uint32_t(m));
			int p[3] = { (i0 / row_size) % block_size.x, i0 % row_size, i0 / deck_size };
			{
				{
					const uint8_t code = merge_mask[i0 * ns + side];
					if (code == 0) {
						continue;
					}''')
patch(B, '''#include "voxel_mesher_blocky.h"
''', '''#include "voxel_mesher_blocky.h"
#include <algorithm>
''')
print('mesh speed patched')
