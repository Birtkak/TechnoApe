# --- the full-detail box follows the ground (ADR 0105) ---
# Part of the TechnoApe godot_voxel patches: run after voxel_patch_remesh.py and voxel_patch_unload.py.
# A cube of full-detail voxels costs its volume: at 100 m every cave and ore 100 m under the player was generated,
# kept in memory (7.6 GB walking) and meshed and drawn (33 M triangles for a full 72 m box). With
# VoxelLodTerrain.set_ground_shell(provider, below, above, near) a viewer that draws (requires_visuals) keeps a
# data column of its box only from (the lowest ground of its 3 x 3 columns - below) to (the highest ground, water
# or tree of those columns + above), plus every block within `near` of the viewer in any direction. provider is
# called from the update task with (x, z, size) in voxels of a data column and returns Vector2i(lowest, highest).
# Data-only viewers (the simulation bubble) keep their whole box. Mesh blocks whose data never loads (deep under
# the ground, high in the sky) are simply never meshed.
import sys
root = sys.argv[1]

def patch(rel, old, new):
    p = root + '/' + rel
    s = open(p, encoding='utf-8').read()
    assert s.count(old) == 1, (rel, old)
    s = s.replace(old, new, 1)
    open(p, 'w', encoding='utf-8', newline='\n').write(s)

D = 'terrain/variable_lod/voxel_lod_terrain_update_data.h'
patch(D, '''		float edit_remesh_rate = 0.f;
''', '''		float edit_remesh_rate = 0.f;
		// TechnoApe ADR 0105: the box follows the ground (set_ground_shell); an invalid provider = the whole box
		Callable shell_provider;
		int shell_below = 0, shell_above = 0, shell_near = 0; // voxels
''')
patch(D, '''		ClipboxStreamingState clipbox_streaming;''', '''		ClipboxStreamingState clipbox_streaming;
		// TechnoApe ADR 0105: each data column's (lowest, highest) ground from shell_provider, by column (blocks)
		StdUnorderedMap<uint64_t, std::pair<int, int>> shell_ground;''')

H = 'terrain/variable_lod/voxel_lod_terrain.h'
patch(H, '''	void set_edit_remesh_rate(float rate);''', '''	void set_edit_remesh_rate(float rate);
	// TechnoApe ADR 0105: see voxel_patch_shell.py; set before the terrain first streams
	void set_ground_shell(const Callable &provider, int below_voxels, int above_voxels, int near_voxels);''')

C = 'terrain/variable_lod/voxel_lod_terrain.cpp'
patch(C, '''float VoxelLodTerrain::get_edit_remesh_rate() const {
	return _update_data->settings.edit_remesh_rate;
}
''', '''float VoxelLodTerrain::get_edit_remesh_rate() const {
	return _update_data->settings.edit_remesh_rate;
}

void VoxelLodTerrain::set_ground_shell(const Callable &provider, int below_voxels, int above_voxels, int near_voxels) {
	_update_data->wait_for_end_of_task();
	VoxelLodTerrainUpdateData::Settings &s = _update_data->settings;
	s.shell_provider = provider;
	s.shell_below = math::max(below_voxels, 0);
	s.shell_above = math::max(above_voxels, 0);
	s.shell_near = math::max(near_voxels, 0);
	_update_data->state.shell_ground.clear();
}
''')
patch(C, '''	ClassDB::bind_method(D_METHOD("set_edit_remesh_rate", "rate"), &Self::set_edit_remesh_rate);''', '''	ClassDB::bind_method(D_METHOD("set_edit_remesh_rate", "rate"), &Self::set_edit_remesh_rate);
	ClassDB::bind_method(
			D_METHOD("set_ground_shell", "provider", "below_voxels", "above_voxels", "near_voxels"), &Self::set_ground_shell
	);''')

U = 'terrain/variable_lod/voxel_lod_terrain_update_clipbox_streaming.cpp'
patch(U, '''void process_data_blocks_sliding_box(''', '''// TechnoApe ADR 0105: the ground shell (voxel_patch_shell.py). Up to two [y0, y1) block ranges per column.
struct ShellRanges {
	int y[4];
	int n = 0;
	void add(int a, int b) {
		if (a >= b) {
			return;
		}
		// merge with an overlapping or touching range (at most two ranges ever)
		for (int i = 0; i < n; i += 2) {
			if (a <= y[i + 1] && b >= y[i]) {
				y[i] = math::min(y[i], a);
				y[i + 1] = math::max(y[i + 1], b);
				if (n == 4 && y[0] <= y[3] && y[2] <= y[1]) {
					y[0] = math::min(y[0], y[2]);
					y[1] = math::max(y[1], y[3]);
					n = 2;
				}
				return;
			}
		}
		y[n] = a;
		y[n + 1] = b;
		n += 2;
	}
};

// The column's (lowest, highest) ground in voxels, from the provider (memoized; deterministic, so dropping the
// memo when it grows changes nothing).
std::pair<int, int> shell_column_ground(
		VoxelLodTerrainUpdateData::State &state,
		const VoxelLodTerrainUpdateData::Settings &settings,
		int bx,
		int bz,
		int block_size
) {
	const uint64_t key = (uint64_t(uint32_t(bx)) << 32) | uint32_t(bz);
	auto it = state.shell_ground.find(key);
	if (it != state.shell_ground.end()) {
		return it->second;
	}
	if (state.shell_ground.size() > 400000) {
		state.shell_ground.clear();
	}
	const ::godot::Vector2i r = settings.shell_provider.call(bx * block_size, bz * block_size, block_size);
	const std::pair<int, int> g{ r.x, r.y };
	state.shell_ground.insert({ key, g });
	return g;
}

// The block ranges of column (x, z) a viewer at block vpos keeps inside box.
ShellRanges shell_column_ranges(
		VoxelLodTerrainUpdateData::State &state,
		const VoxelLodTerrainUpdateData::Settings &settings,
		const Box3i &box,
		Vector3i vpos,
		int near_blocks,
		int x,
		int z,
		int block_size_po2
) {
	ShellRanges r;
	if (x < box.position.x || x >= box.position.x + box.size.x || z < box.position.z ||
		z >= box.position.z + box.size.z) {
		return r;
	}
	const int bs = 1 << block_size_po2;
	int lo = INT32_MAX, hi = INT32_MIN;
	for (int dz = -1; dz <= 1; ++dz) {
		for (int dx = -1; dx <= 1; ++dx) {
			const std::pair<int, int> g = shell_column_ground(state, settings, x + dx, z + dz, bs);
			lo = math::min(lo, g.first);
			hi = math::max(hi, g.second);
		}
	}
	const int y0 = box.position.y, y1 = box.position.y + box.size.y;
	r.add(math::max(y0, math::floordiv(lo - settings.shell_below, bs)),
		  math::min(y1, math::floordiv(hi + settings.shell_above, bs) + 1));
	if (math::abs(x - vpos.x) <= near_blocks && math::abs(z - vpos.z) <= near_blocks) {
		r.add(math::max(y0, vpos.y - near_blocks), math::min(y1, vpos.y + near_blocks + 1));
	}
	return r;
}

// Calls f(Box3i) for every column piece in `a` and not in `b`.
template <typename F>
void shell_ranges_minus(const ShellRanges &a, const ShellRanges &b, int x, int z, F f) {
	for (int i = 0; i < a.n; i += 2) {
		int s = a.y[i];
		const int e = a.y[i + 1];
		while (s < e) {
			// the first range of b that covers or follows s
			int cut_s = e, cut_e = e;
			for (int k = 0; k < b.n; k += 2) {
				if (b.y[k + 1] > s && b.y[k] < cut_s) {
					cut_s = math::max(b.y[k], s);
					cut_e = b.y[k + 1];
				}
			}
			if (cut_s > s) {
				f(Box3i(Vector3i(x, s, z), Vector3i(1, math::min(cut_s, e) - s, 1)));
			}
			s = math::max(cut_e, cut_s);
		}
	}
}

void process_data_blocks_sliding_box(''')

patch(U, '''			if (prev_data_box != new_data_box) {
				// Detect blocks to load.
				if (can_load) {
					tls_missing_blocks.clear();

					new_data_box.difference(prev_data_box, [&data, lod_index](Box3i box_to_load) {
						data.view_area(box_to_load, lod_index, &tls_missing_blocks, nullptr, nullptr);
					});
''', '''			// TechnoApe ADR 0105: a drawing viewer's box follows the ground (voxel_patch_shell.py)
			const bool shell = lod_index == 0 && settings.shell_near > 0 && settings.shell_provider.is_valid() &&
					paired_viewer.state.requires_visuals;
			const int near_blocks = settings.shell_near >> lod_data_block_size_po2;
			const Vector3i shell_vpos = paired_viewer.state.local_position_voxels >> lod_data_block_size_po2;
			const Vector3i shell_vpos_prev = paired_viewer.prev_state.local_position_voxels >> lod_data_block_size_po2;
			// every column piece the shell gains (gained = true) or loses between the previous and this update
			auto shell_pieces = [&](bool gained, auto f) {
				Box3i cols = new_data_box;
				if (cols.size == Vector3i()) {
					cols = prev_data_box;
				} else if (prev_data_box.size != Vector3i()) {
					cols.merge_with(prev_data_box);
				}
				for (int z = cols.position.z; z < cols.position.z + cols.size.z; ++z) {
					for (int x = cols.position.x; x < cols.position.x + cols.size.x; ++x) {
						const ShellRanges now = shell_column_ranges(
								state, settings, new_data_box, shell_vpos, near_blocks, x, z, lod_data_block_size_po2
						);
						const ShellRanges was = shell_column_ranges(
								state, settings, prev_data_box, shell_vpos_prev, near_blocks, x, z, lod_data_block_size_po2
						);
						if (gained) {
							shell_ranges_minus(now, was, x, z, f);
						} else {
							shell_ranges_minus(was, now, x, z, f);
						}
					}
				}
			};

			if (prev_data_box != new_data_box || (shell && shell_vpos != shell_vpos_prev)) {
				// Detect blocks to load.
				if (can_load) {
					tls_missing_blocks.clear();

					auto view = [&data, lod_index](Box3i box_to_load) {
						data.view_area(box_to_load, lod_index, &tls_missing_blocks, nullptr, nullptr);
					};
					if (shell) {
						shell_pieces(true, view);
					} else {
						new_data_box.difference(prev_data_box, view);
					}
''')
patch(U, '''					prev_data_box.difference(new_data_box, [&data, blocks_to_save, lod_index](Box3i box_to_remove) {
						data.unview_area(
								box_to_remove,
								lod_index,
								&tls_found_blocks_positions,
								&tls_missing_blocks,
								blocks_to_save
						);
					});
''', '''					auto unview = [&data, blocks_to_save, lod_index](Box3i box_to_remove) {
						data.unview_area(
								box_to_remove,
								lod_index,
								&tls_found_blocks_positions,
								&tls_missing_blocks,
								blocks_to_save
						);
					};
					if (shell) {
						shell_pieces(false, unview);
					} else {
						prev_data_box.difference(new_data_box, unview);
					}
''')
