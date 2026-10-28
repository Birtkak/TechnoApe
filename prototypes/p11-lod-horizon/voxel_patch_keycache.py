# --- the save's key cache loads up front (ADR 0105) ---
# Part of the TechnoApe godot_voxel patches: run after voxel_patch_stream.py.
# voxel_patch_stream.py lets blocks the save never held skip the serial I/O queue, using the key cache, but the
# cache only loaded with the first connection, inside the first I/O task: the first update had already queued every
# block of the box as a serial load (a new world at 100 m: ~470 k of them at ~15 k a second, the loading screen
# waited 30 s and more). Turning the key cache on with a database path set now opens a connection at once, which
# loads every key, and puts it back in the pool.
import sys
root = sys.argv[1]

def patch(rel, old, new):
    p = root + '/' + rel
    s = open(p, encoding='utf-8').read()
    assert s.count(old) == 1, (rel, old)
    s = s.replace(old, new, 1)
    open(p, 'w', encoding='utf-8', newline='\n').write(s)

patch('streams/sqlite/voxel_stream_sqlite.cpp', '''void VoxelStreamSQLite::set_key_cache_enabled(bool enable) {
	_block_keys_cache_enabled = enable;
}''', '''void VoxelStreamSQLite::set_key_cache_enabled(bool enable) {
	_block_keys_cache_enabled = enable;
	bool has_path;
	{
		MutexLock mlock(_connection_mutex);
		has_path = !_globalized_connection_path.empty();
	}
	if (enable && has_path && !_block_keys_cache_loaded) {
		// TechnoApe ADR 0105: load the keys now, before the terrain queues its first requests
		const ConnectionResult r = get_connection();
		if (r.connection != nullptr) {
			recycle_connection(r.connection);
		}
	}
}''')
