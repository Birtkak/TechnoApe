# --- clipbox unloading without the quadratic scan (the 100 m box's 200 ms stalls) ---
# Part of the TechnoApe godot_voxel patches: run after voxel_patch_vertical.py.
# When the box slides, every block of the slab it leaves that is still waiting for generation is
# unreferenced, and stock godot_voxel searched the whole list of blocks requested in the same update
# for each of them (to drop a request that was never sent). In flight at 100 m both lists hold ~10k
# blocks: 10^8 compares under loading_blocks_mutex, and the main thread waited ~210 ms for that lock
# on the next finished generation task (measured 2026-10-28 with voxel_patch_profile.py). The dropped
# positions now go into a set and the request list is filtered once per LOD.
import sys
root = sys.argv[1]

def patch(rel, old, new):
    p = root + '/' + rel
    s = open(p, encoding='utf-8').read()
    assert s.count(old) == 1, (rel, old)
    s = s.replace(old, new, 1)
    open(p, 'w', encoding='utf-8', newline='\n').write(s)

C = 'terrain/variable_lod/voxel_lod_terrain_update_clipbox_streaming.cpp'
patch(C, '''		StdUnorderedMap<Vector3i, VoxelLodTerrainUpdateData::LoadingDataBlock> &loading_blocks,
		StdVector<VoxelLodTerrainUpdateData::BlockToLoad> &data_blocks_to_load,
		Vector3i bpos,
		unsigned int lod_index
) {''', '''		StdUnorderedMap<Vector3i, VoxelLodTerrainUpdateData::LoadingDataBlock> &loading_blocks,
		StdUnorderedSet<Vector3i> &dropped_requests,
		Vector3i bpos
) {''')
patch(C, '''		loading_blocks.erase(loading_block_it);

		// Also remove from blocks about to be added to the loading queue
		VoxelLodTerrainUpdateData::BlockLocation bloc{ bpos, static_cast<uint8_t>(lod_index) };
		for (size_t i = 0; i < data_blocks_to_load.size(); ++i) {
			if (data_blocks_to_load[i].loc == bloc) {
				data_blocks_to_load[i] = data_blocks_to_load.back();
				data_blocks_to_load.pop_back();
				// We don't touch the cancellation token since tasks haven't been spawned yet for
				// these
				break;
			}
		}
	}''', '''		loading_blocks.erase(loading_block_it);

		// Also remove from blocks about to be added to the loading queue (filtered once by the caller)
		dropped_requests.insert(bpos);
	}''')
patch(C, '''					if (tls_missing_blocks.size() > 0) {
						MutexLock mlock(lod.loading_blocks_mutex);
						for (const Vector3i bpos : tls_missing_blocks) {
							unreference_data_block_from_loading_lists(
									lod.loading_blocks, data_blocks_to_load, bpos, lod_index
							);
						}
					}''', '''					if (tls_missing_blocks.size() > 0) {
						static thread_local StdUnorderedSet<Vector3i> tls_dropped_requests;
						tls_dropped_requests.clear();
						{
							MutexLock mlock(lod.loading_blocks_mutex);
							for (const Vector3i bpos : tls_missing_blocks) {
								unreference_data_block_from_loading_lists(lod.loading_blocks, tls_dropped_requests, bpos);
							}
						}
						// We don't touch the cancellation tokens since tasks haven't been spawned yet for these
						if (tls_dropped_requests.size() > 0) {
							for (size_t i = 0; i < data_blocks_to_load.size();) {
								const VoxelLodTerrainUpdateData::BlockLocation &loc = data_blocks_to_load[i].loc;
								if (loc.lod == lod_index && tls_dropped_requests.count(loc.position) != 0) {
									data_blocks_to_load[i] = data_blocks_to_load.back();
									data_blocks_to_load.pop_back();
								} else {
									++i;
								}
							}
						}
					}''')
