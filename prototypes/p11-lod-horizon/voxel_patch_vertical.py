# --- flatter LOD boxes: a vertical ratio for every LOD (fast flight vs generation) ---
# Part of the TechnoApe godot_voxel patches: run after voxel_patch_remesh.py.
# VoxelLodTerrain's clipboxes are cubes: the full-detail box reaches as far up
# and down as it does sideways, so a flight at 20 m/s generated rock and sky
# blocks far faster than six workers could (98k queued tasks, bug report
# 2026-10-25 "load way too slow"). VoxelViewer.view_distance_vertical_ratio only
# shapes the last LOD. VoxelLodTerrain.lod_vertical_ratio (default 1 = stock)
# scales the vertical half-size of the mesh box of every LOD but the first, and
# of its data box with two blocks to spare (the data box must hold the mesh box
# and its neighbours). LOD 0 stays a cube: the game's simulation bubble copies
# its data 16 m up and down.
import sys
root = sys.argv[1]

p = root + '/terrain/variable_lod/voxel_lod_terrain_update_data.h'
s = open(p, encoding='utf-8').read()
old = '\t\tfloat edit_remesh_rate = 0.f;\n'
assert s.count(old) == 1
s = s.replace(old, old + '\t\t// TechnoApe P11: vertical / horizontal half-size of every LOD box (1 = cubes)\n\t\tfloat lod_vertical_ratio = 1.f;\n', 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)

p = root + '/terrain/variable_lod/voxel_lod_terrain_update_clipbox_streaming.cpp'
s = open(p, encoding='utf-8').read()
old = '''		int lod_chunk_size,
		Vector3i max_view_distance_voxels
) {
	int ld;'''
assert s.count(old) == 1
s = s.replace(old, '''		int lod_chunk_size,
		Vector3i max_view_distance_voxels,
		float vertical_ratio = 1.f,
		int vertical_spare = 0
) {
	int ld;''', 1)
old = '''	Vector3i ld3(ld, ld, ld);
	if (lod_index == lod_count - 1) {'''
assert s.count(old) == 1
s = s.replace(old, '''	Vector3i ld3(ld, ld, ld);
	if (vertical_ratio < 0.999f && lod_index > 0) {
		// TechnoApe P11: flatter boxes, at least two chunks up and down. LOD 0 stays a cube: the sim
		// bubble copies its full-detail data and needs it all the way up and down
		ld3.y = math::max(2, static_cast<int>(Math::ceil(static_cast<float>(ld) * vertical_ratio))) + vertical_spare;
	}
	if (lod_index == lod_count - 1) {''', 1)
old = '''						lod_mesh_block_size,
						Vector3i(
								paired_viewer.state.view_distance_voxels.horizontal,
								paired_viewer.state.view_distance_voxels.vertical,
								paired_viewer.state.view_distance_voxels.horizontal
						)
				);'''
assert s.count(old) == 1
s = s.replace(old, '''						lod_mesh_block_size,
						Vector3i(
								paired_viewer.state.view_distance_voxels.horizontal,
								paired_viewer.state.view_distance_voxels.vertical,
								paired_viewer.state.view_distance_voxels.horizontal
						),
						volume_settings.lod_vertical_ratio
				);''', 1)
old = '''						lod_data_block_size,
						Vector3i(
								paired_viewer.state.view_distance_voxels.horizontal,
								paired_viewer.state.view_distance_voxels.vertical,
								paired_viewer.state.view_distance_voxels.horizontal
						)
				);'''
assert s.count(old) == 1
s = s.replace(old, '''						lod_data_block_size,
						Vector3i(
								paired_viewer.state.view_distance_voxels.horizontal,
								paired_viewer.state.view_distance_voxels.vertical,
								paired_viewer.state.view_distance_voxels.horizontal
						),
						volume_settings.lod_vertical_ratio, 2
				);''', 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)

p = root + '/terrain/variable_lod/voxel_lod_terrain.h'
s = open(p, encoding='utf-8').read()
old = '\tvoid set_edit_remesh_rate(float rate);\n\tfloat get_edit_remesh_rate() const;\n'
assert s.count(old) == 1
s = s.replace(old, old + '''	// TechnoApe P11: vertical / horizontal half-size of every LOD box (1 = cubes)
	void set_lod_vertical_ratio(float ratio);
	float get_lod_vertical_ratio() const;
''', 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)

p = root + '/terrain/variable_lod/voxel_lod_terrain.cpp'
s = open(p, encoding='utf-8').read()
old = '''float VoxelLodTerrain::get_edit_remesh_rate() const {
	return _update_data->settings.edit_remesh_rate;
}'''
assert s.count(old) == 1
s = s.replace(old, old + '''

void VoxelLodTerrain::set_lod_vertical_ratio(float ratio) {
	_update_data->wait_for_end_of_task();
	_update_data->settings.lod_vertical_ratio = math::clamp(ratio, 0.1f, 1.f);
}

float VoxelLodTerrain::get_lod_vertical_ratio() const {
	return _update_data->settings.lod_vertical_ratio;
}''', 1)
old = '''	ClassDB::bind_method(D_METHOD("set_edit_remesh_rate", "rate"), &Self::set_edit_remesh_rate);'''
assert s.count(old) == 1
s = s.replace(old, old + '''
	ClassDB::bind_method(D_METHOD("get_lod_vertical_ratio"), &Self::get_lod_vertical_ratio);
	ClassDB::bind_method(D_METHOD("set_lod_vertical_ratio", "ratio"), &Self::set_lod_vertical_ratio);''', 1)
old = '''	ADD_PROPERTY(PropertyInfo(Variant::FLOAT, "edit_remesh_rate"), "set_edit_remesh_rate", "get_edit_remesh_rate");'''
assert s.count(old) == 1
s = s.replace(old, old + '''
	ADD_PROPERTY(PropertyInfo(Variant::FLOAT, "lod_vertical_ratio"), "set_lod_vertical_ratio", "get_lod_vertical_ratio");''', 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)
print('patched lod-vertical-ratio')
