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

# The same loops held loading_blocks_mutex for ~15 ms per slab (10k blocks), and the main thread waited for
# it on every finished block in that time: they now let go of it every LOCK_CHUNK blocks. Each entry is
# consistent on its own under the lock, so the main thread may run between chunks.
patch(C, '''						ZN_PROFILE_SCOPE_NAMED("Add loading blocks");
						MutexLock mlock(lod.loading_blocks_mutex);
						for (const Vector3i bpos : tls_missing_blocks) {
							add_loading_block(lod, bpos, lod_index, data_blocks_to_load);
						}''', '''						ZN_PROFILE_SCOPE_NAMED("Add loading blocks");
						constexpr size_t LOCK_CHUNK = 256; // TechnoApe ADR 0105
						for (size_t i0 = 0; i0 < tls_missing_blocks.size(); i0 += LOCK_CHUNK) {
							MutexLock mlock(lod.loading_blocks_mutex);
							for (size_t i = i0; i < tls_missing_blocks.size() && i < i0 + LOCK_CHUNK; ++i) {
								add_loading_block(lod, tls_missing_blocks[i], lod_index, data_blocks_to_load);
							}
						}''')
patch(C, '''					if (tls_found_blocks_positions.size() > 0) {
						MutexLock mlock(lod.loading_blocks_mutex);
						for (const Vector3i bpos : tls_found_blocks_positions) {
							// emit_data_block_unloaded(bpos);

							// TODO If they were loaded, why would they be in loading blocks?
							// Maybe to make sure they are not in here regardless
							lod.loading_blocks.erase(bpos);
						}
					}''', '''					for (size_t i0 = 0; i0 < tls_found_blocks_positions.size(); i0 += 256) {
						// TechnoApe ADR 0105: in chunks, see "Add loading blocks"
						MutexLock mlock(lod.loading_blocks_mutex);
						for (size_t i = i0; i < tls_found_blocks_positions.size() && i < i0 + 256; ++i) {
							// TODO If they were loaded, why would they be in loading blocks?
							// Maybe to make sure they are not in here regardless
							lod.loading_blocks.erase(tls_found_blocks_positions[i]);
						}
					}''')
patch(C, '''						{
							MutexLock mlock(lod.loading_blocks_mutex);
							for (const Vector3i bpos : tls_missing_blocks) {
								unreference_data_block_from_loading_lists(lod.loading_blocks, tls_dropped_requests, bpos);
							}
						}''', '''						for (size_t i0 = 0; i0 < tls_missing_blocks.size(); i0 += 256) {
							// TechnoApe ADR 0105: in chunks, see "Add loading blocks"
							MutexLock mlock(lod.loading_blocks_mutex);
							for (size_t i = i0; i < tls_missing_blocks.size() && i < i0 + 256; ++i) {
								unreference_data_block_from_loading_lists(lod.loading_blocks, tls_dropped_requests, tls_missing_blocks[i]);
							}
						}''')

# Before each chunk the worker lets the main thread through first (voxel_patch_apply_budget.py raises the flag
# while it applies finished tasks; Windows mutexes are not fair, so a bare unlock/relock starved it).
s = open(root + '/' + C, encoding='utf-8').read()
n = s.count('\t\t\t\t\t\t\tMutexLock mlock(lod.loading_blocks_mutex);\n\t\t\t\t\t\t\tfor (size_t i = i0;')
n += s.count('\t\t\t\t\t\tMutexLock mlock(lod.loading_blocks_mutex);\n\t\t\t\t\t\tfor (size_t i = i0;')
assert n == 3, n
for ind in ('\t\t\t\t\t\t\t', '\t\t\t\t\t\t'):
    s = s.replace(ind + 'MutexLock mlock(lod.loading_blocks_mutex);\n' + ind + 'for (size_t i = i0;',
                  ind + 'while (g_main_applying_results.load(std::memory_order_relaxed)) {\n' + ind + '\tstd::this_thread::yield();\n' + ind + '}\n'
                  + ind + 'MutexLock mlock(lod.loading_blocks_mutex);\n' + ind + 'for (size_t i = i0;')
s = s.replace('#include "voxel_lod_terrain_update_clipbox_streaming.h"\n', '#include "voxel_lod_terrain_update_clipbox_streaming.h"\n#include "../../util/tasks/threaded_task_runner.h"\n#include <thread>\n', 1)
open(root + '/' + C, 'w', encoding='utf-8', newline='\n').write(s)

