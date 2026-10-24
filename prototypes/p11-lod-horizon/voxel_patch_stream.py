# --- blocks the save never held skip the serial I/O queue (fast flight with a save) ---
# Part of the TechnoApe godot_voxel patches: run after voxel_patch_vertical.py.
# With a stream, VoxelLodTerrain asks it for every block of every LOD before
# generating, as one serial I/O task per block. Our VoxelStreamSQLite has the key
# cache on, so a miss is cheap, but the tasks run one at a time and every worker
# looking for work walks past the queued ones: a 20 m/s flight over a saved
# world queued 87k of them, near detail was missing 10 % of the time and p99
# frames were 15 ms (2026-10-25, "load way too slow"). VoxelStream gets
# may_have_block (default true); VoxelStreamSQLite answers it from the key cache
# once that is loaded, and request_block_load generates a block the save never
# held at once, as it does without a stream.
import sys
root = sys.argv[1]

p = root + '/streams/voxel_stream.h'
s = open(p, encoding='utf-8').read()
old = '\tvirtual int get_lod_count() const;\n'
assert s.count(old) == 1
s = s.replace(old, old + '''
	// TechnoApe P11: false when the block is known not to be stored (no need to ask), true when it may be
	virtual bool may_have_block(Vector3i block_pos, unsigned int lod_index) const {
		return true;
	}
''', 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)

p = root + '/streams/sqlite/voxel_stream_sqlite.h'
s = open(p, encoding='utf-8').read()
old = '#include "block_key_cache.h"\n'
assert s.count(old) == 1
s = s.replace(old, old + '#include <atomic>\n', 1)
old = '\tvoid set_key_cache_enabled(bool enable);\n'
assert s.count(old) == 1
s = s.replace(old, old + '\t// TechnoApe P11: answered from the key cache once it is loaded\n\tbool may_have_block(Vector3i block_pos, unsigned int lod_index) const override;\n', 1)
old = '\tbool _block_keys_cache_enabled = false;\n'
assert s.count(old) == 1
s = s.replace(old, old + '\tstd::atomic<bool> _block_keys_cache_loaded{ false };   // TechnoApe P11\n', 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)

p = root + '/streams/sqlite/voxel_stream_sqlite.cpp'
s = open(p, encoding='utf-8').read()
old = '''	_block_keys_cache.clear();
	_connection_pool.clear();
'''
assert s.count(old) == 1
s = s.replace(old, '''	_block_keys_cache_loaded = false;
	_block_keys_cache.clear();
	_connection_pool.clear();
''', 1)
old = '''			cache->add_no_lock(loc.position, loc.lod);
		});
	}
	return { con, ConnectionResult::SUCCESS };
}'''
assert s.count(old) == 1
s = s.replace(old, '''			cache->add_no_lock(loc.position, loc.lod);
		});
		_block_keys_cache_loaded = true;
	}
	return { con, ConnectionResult::SUCCESS };
}

bool VoxelStreamSQLite::may_have_block(Vector3i block_pos, unsigned int lod_index) const {
	if (!_block_keys_cache_enabled || !_block_keys_cache_loaded) {
		return true;
	}
	return _block_keys_cache.contains(block_pos, lod_index);
}''', 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)

p = root + '/terrain/variable_lod/voxel_lod_terrain_update_task.cpp'
s = open(p, encoding='utf-8').read()
old = '''	if (stream_dependency->stream.is_valid()) {
		PriorityDependency priority_dependency;'''
assert s.count(old) == 1
s = s.replace(old, '''	// TechnoApe P11: a block the save never held is generated at once, not queued for a serial I/O miss
	const bool known_absent = stream_dependency->stream.is_valid() && settings.cache_generated_blocks &&
			!stream_dependency->stream->may_have_block(block_pos, lod_index);

	if (stream_dependency->stream.is_valid() && !known_absent) {
		PriorityDependency priority_dependency;''', 1)
old = '''	} else if (settings.cache_generated_blocks) {
		// Directly generate the block without checking the stream.'''
assert s.count(old) == 1
s = s.replace(old, '''	} else if (settings.cache_generated_blocks) {
		// Directly generate the block without checking the stream (none, or it never held this block).''', 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)
print('patched stream-key-skip')
