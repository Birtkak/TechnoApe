# Patch for the custom godot_voxel build: an edit queued for LOD propagation can outlive its block.
# VoxelData::update_lods asserts (fatal, kills the game) when a LOD0 block that was edited is gone by
# the time the queued edits are flushed: clipbox streaming unloaded it in between. The sim bubble
# pastes water near the edge of full detail and hit this (bug report 2026-10-24: the river crash).
# Fix: drop such blocks from the list instead of carrying them up to the LOD1 pass.
# Run with the godot_voxel root as argv[1].
import sys
root = sys.argv[1]

p = root + '/storage/voxel_data.cpp'
s = open(p, encoding='utf-8').read()
old = '''		for (const Vector3i data_block_pos : blocks_pending_lodding_lod0) {
			VoxelDataBlock *data_block = data_lod0.map.get_block(data_block_pos);
			ERR_CONTINUE(data_block == nullptr);'''
new = '''		// TechnoApe: an edited block can be unloaded before its edit is flushed; drop it here, or the
		// LOD1 pass below asserts on it (fatal).
		unordered_remove_if(blocks_pending_lodding_lod0, [&data_lod0](const Vector3i bpos) {
			return data_lod0.map.get_block(bpos) == nullptr;
		});
		for (const Vector3i data_block_pos : blocks_pending_lodding_lod0) {
			VoxelDataBlock *data_block = data_lod0.map.get_block(data_block_pos);
			ERR_CONTINUE(data_block == nullptr);'''
assert s.count(old) == 1, s.count(old)
s = s.replace(old, new, 1)
old = '#include "../util/containers/std_vector.h"'
assert s.count(old) == 1
s = s.replace(old, old + '\n#include "../util/containers/container_funcs.h" // TechnoApe', 1)
old = '''			ZN_ASSERT(src_block != nullptr);
			src_block->set_needs_lodding(false);'''
new = '''			if (src_block == nullptr) {
				continue; // TechnoApe: unloaded since it was queued (was a fatal ZN_ASSERT)
			}
			src_block->set_needs_lodding(false);'''
assert s.count(old) == 1, s.count(old)
s = s.replace(old, new, 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)
print('patched')
