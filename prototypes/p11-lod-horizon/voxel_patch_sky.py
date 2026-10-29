# --- sky blocks without a task (ADR 0108) ---
# Part of the TechnoApe godot_voxel patches: run after voxel_patch_shell.py and voxel_patch_apply_budget.py.
# The ground shell keeps 64 m of air over the highest ground, water or tree of every column (builds up to 64 m
# stand, ADR 0105). At 100 m those were 70 % of all generation tasks (622 k of 888 k in a 20 s flight): each cost the
# generator only its sky check, but the queue peaked at 570 k tasks, which the workers re-sort every 200 ms under one
# lock, and near ground waited 13 s behind it. A lod 0 block at or over its column's top bound (the shell's memo of
# FineGen.ground_range, the same bound the generator's sky check uses) that the save never held is set as air in the
# update task, like apply_block_data_requests_as_empty, and never queued.
import sys
root = sys.argv[1]
REVERT = len(sys.argv) > 2 and sys.argv[2] == 'revert'

def patch(rel, old, new):
    if REVERT:
        old, new = new, old
    p = root + '/' + rel
    s = open(p, encoding='utf-8').read()
    if s.count(new) == 1 and (new.count(old) or s.count(old) == 0):
        return  # already done
    assert s.count(old) == 1, (rel, old)
    s = s.replace(old, new, 1)
    open(p, 'w', encoding='utf-8', newline='\n').write(s)

C = 'terrain/variable_lod/voxel_lod_terrain_update_task.cpp'
patch(C, '''// Used only when streaming block by block
void request_block_load(''', '''// Used only when streaming block by block
// TechnoApe ADR 0108: returns true for a sky block, which the caller sets as air instead
bool request_block_load(''')
patch(C, '''		if (voxels != nullptr) {
			lod.quick_reloading_blocks.push_back(VoxelLodTerrainUpdateData::QuickReloadingBlock{ voxels, block_pos });
			return;
		}
	}

	// TechnoApe P11: a block the save never held is generated at once, not queued for a serial I/O miss
	const bool known_absent = stream_dependency->stream.is_valid() && settings.cache_generated_blocks &&
			!stream_dependency->stream->may_have_block(block_pos, lod_index);
''', '''		if (voxels != nullptr) {
			lod.quick_reloading_blocks.push_back(VoxelLodTerrainUpdateData::QuickReloadingBlock{ voxels, block_pos });
			return false;
		}
	}

	// TechnoApe P11: a block the save never held is generated at once, not queued for a serial I/O miss
	const bool known_absent = stream_dependency->stream.is_valid() && settings.cache_generated_blocks &&
			!stream_dependency->stream->may_have_block(block_pos, lod_index);

	// TechnoApe ADR 0108: at or over the column's top bound and never saved: air, no task
	if (lod_index == 0 && settings.cache_generated_blocks && (stream_dependency->stream.is_null() || known_absent)) {
		auto g = state.shell_ground.find((uint64_t(uint32_t(block_pos.x)) << 32) | uint32_t(block_pos.z));
		if (g != state.shell_ground.end() && block_pos.y * int(data_block_size) >= g->second.second) {
			return true;
		}
	}
''')
patch(C, '''	} else {
		ZN_PRINT_WARNING("Requesting a block load when it should not have been necessary");
	}
}''', '''	} else {
		ZN_PRINT_WARNING("Requesting a block load when it should not have been necessary");
	}
	return false;
}''')
patch(C, '''	for (unsigned int i = 0; i < blocks_to_load.size(); ++i) {
		const VoxelLodTerrainUpdateData::BlockToLoad btl = blocks_to_load[i];
		request_block_load(''', '''	static thread_local StdVector<Vector3i> tls_sky; // TechnoApe ADR 0108
	tls_sky.clear();
	for (unsigned int i = 0; i < blocks_to_load.size(); ++i) {
		const VoxelLodTerrainUpdateData::BlockToLoad btl = blocks_to_load[i];
		if (request_block_load(''')
patch(C, '''				btl.cancellation_token,
				state
		);
	}
}''', '''				btl.cancellation_token,
				state
			)) {
			tls_sky.push_back(btl.loc.position);
		}
	}
	// TechnoApe ADR 0108: sky blocks are air now (lod 0), in chunks so the main thread's applies get the locks
	VoxelLodTerrainUpdateData::Lod &lod = state.lods[0];
	const VoxelFormat format = data->get_format();
	for (size_t i0 = 0; i0 < tls_sky.size(); i0 += 256) {
		while (g_main_applying_results.load(std::memory_order_relaxed)) {
			std::this_thread::yield();
		}
		const size_t i1 = math::min(tls_sky.size(), i0 + 256);
		for (size_t i = i0; i < i1; ++i) {
			RefCount viewers;
			{
				MutexLock mlock(lod.loading_blocks_mutex);
				auto it = lod.loading_blocks.find(tls_sky[i]);
				if (it == lod.loading_blocks.end()) {
					continue;
				}
				viewers = it->second.viewers;
			}
			std::shared_ptr<VoxelBuffer> air = make_shared_instance<VoxelBuffer>(VoxelBuffer::ALLOCATOR_POOL);
			air->create(Vector3iUtil::create(data_block_size), &format);
			VoxelDataBlock block(air, 0);
			block.viewers = viewers;
			// after the map holds it, as apply_data_block_response does, so it is never requested twice
			if (data->try_set_block(tls_sky[i], block)) {
				MutexLock mlock(lod.loading_blocks_mutex);
				lod.loading_blocks.erase(tls_sky[i]);
			}
		}
		MutexLock mlock(state.clipbox_streaming.loaded_data_blocks_mutex);
		for (size_t i = i0; i < i1; ++i) {
			state.clipbox_streaming.loaded_data_blocks.push_back(VoxelLodTerrainUpdateData::BlockLocation{ tls_sky[i], 0 });
		}
	}
}''')
patch(C, '''#include "../../util/tasks/async_dependency_tracker.h"
''', '''#include "../../util/tasks/async_dependency_tracker.h"
#include <thread> // TechnoApe ADR 0108
''')
