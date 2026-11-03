# --- where a missing mesh block is stuck (ADR 0136) ---
# Part of the TechnoApe godot_voxel patches: run after voxel_patch_budget.py and voxel_patch_shell.py.
# VoxelLodTerrain.debug_classify_mesh_blocks(PackedInt32Array xyz) -> PackedByteArray: for each LOD 0 mesh block
# position (x, y, z triples) the pipeline stage it is in, so the game's /holes census (debug_capture.gd) can say
# why ground in view is not drawn:
#   0 drawn (mesh, visible)          1 mesh but hidden
#   2 the first missing data block of its padded area is not even requested (outside the box / shell)
#   3 ... is generating     4 data complete, mesh not sent yet (never updated / pending)
#   5 mesh task sent (meshing)       6 meshed, waiting to be applied on the main thread (upload budget)
#   7 up to date but no mesh (the mesher made nothing)   8 not in the mesh map (not requested)
import sys
root = sys.argv[1]

def patch(rel, old, new):
    p = root + '/' + rel
    s = open(p, encoding='utf-8').read()
    if s.count(new) == 1 and (new.count(old) or s.count(old) == 0):
        return  # already done
    assert s.count(old) == 1, (rel, old)
    s = s.replace(old, new, 1)
    open(p, 'w', encoding='utf-8', newline='\n').write(s)

H = 'terrain/variable_lod/voxel_lod_terrain.h'
C = 'terrain/variable_lod/voxel_lod_terrain.cpp'
patch(H, '''	Dictionary debug_get_mesh_block_info(Vector3 fbpos, int lod_index) const;
''', '''	Dictionary debug_get_mesh_block_info(Vector3 fbpos, int lod_index) const;
	PackedByteArray debug_classify_mesh_blocks(PackedInt32Array xyz) const; // TechnoApe ADR 0136
''')
patch(C, '''Dictionary VoxelLodTerrain::debug_get_mesh_block_info(Vector3 fbpos, int lod_index) const {
''', '''// TechnoApe ADR 0136: the pipeline stage of each LOD 0 mesh block (codes in voxel_patch_holes.py)
PackedByteArray VoxelLodTerrain::debug_classify_mesh_blocks(PackedInt32Array xyz) const {
	PackedByteArray out;
	const int n = xyz.size() / 3;
	out.resize(n);
	const VoxelMeshMap<VoxelMeshBlockVLT> &mesh_map = _mesh_maps_per_lod[0];
	const VoxelLodTerrainUpdateData::Lod &lod = _update_data->state.lods[0];
	const int shift = get_mesh_block_size_pow2() - get_data_block_size_pow2();
	for (int i = 0; i < n; ++i) {
		const Vector3i bpos(xyz[i * 3], xyz[i * 3 + 1], xyz[i * 3 + 2]);
		const VoxelMeshBlockVLT *block = mesh_map.get_block(bpos);
		if (block != nullptr && block->has_mesh()) {
			out.set(i, block->is_visible() ? 0 : 1);
			continue;
		}
		int state = -1;
		{
			RWLockRead rlock(lod.mesh_map_state.map_lock);
			auto it = lod.mesh_map_state.map.find(bpos);
			if (it != lod.mesh_map_state.map.end()) {
				state = it->second.state;
			}
		}
		if (state < 0) {
			out.set(i, 8);
			continue;
		}
		if (state == VoxelLodTerrainUpdateData::MESH_UPDATE_SENT) {
			const bool waiting = _queued_main_thread_mesh_updates[0].count(bpos) > 0 ||
					_deferred_mesh_updates_per_lod[0].count(bpos) > 0;
			out.set(i, waiting ? 6 : 5);
			continue;
		}
		if (state == VoxelLodTerrainUpdateData::MESH_UP_TO_DATE) {
			out.set(i, 7);
			continue;
		}
		// not meshed yet: is its data (the mesh block's data blocks and one ring around) all there?
		int code = 4;
		const Vector3i d0 = (bpos << shift) - Vector3i(1, 1, 1);
		const int e = (1 << shift) + 2;
		for (int z = 0; z < e && code == 4; ++z) {
			for (int y = 0; y < e && code == 4; ++y) {
				for (int x = 0; x < e && code == 4; ++x) {
					const Vector3i d = d0 + Vector3i(x, y, z);
					if (_data->has_block(d, 0)) {
						continue;
					}
					MutexLock lock(lod.loading_blocks_mutex);
					code = lod.has_loading_block(d) ? 3 : 2;
				}
			}
		}
		out.set(i, code);
	}
	return out;
}

Dictionary VoxelLodTerrain::debug_get_mesh_block_info(Vector3 fbpos, int lod_index) const {
''')
patch(C, '''	ClassDB::bind_method(D_METHOD("debug_get_mesh_block_info", "block_pos", "lod"), &Self::debug_get_mesh_block_info);
''', '''	ClassDB::bind_method(D_METHOD("debug_get_mesh_block_info", "block_pos", "lod"), &Self::debug_get_mesh_block_info);
	ClassDB::bind_method(D_METHOD("debug_classify_mesh_blocks", "xyz"), &Self::debug_classify_mesh_blocks);
''')
