# --- the loading-blocks map is sized for the box up front, and the update thread's holds of its lock are timed ---
# TechnoApe ADR 0136. In flight the main thread's apply_data_block_response waited 10-71 ms on
# lod.loading_blocks_mutex (fluid-lag's proc_probe): the update thread held it to add or drop 256 loading blocks, and
# lod.loading_blocks (an unordered_map of 50-340 k entries while flying) was never reserved, so an insert past a new
# size peak rehashed all of it under the lock. Same cure as voxel_patch_reserve.py for the data map: reserve_data_map()
# also reserves loading_blocks for the whole box. get_statistics() gains the run's longest update-thread hold
# (ta_loading_hold_max), holds over 1 ms (ta_loading_hold_slow_n) and holds that rehashed (ta_loading_rehash_n); they
# run on, never reset on read. Runs last: after voxel_patch_reserve.py, _profile.py, _throughput.py, _visible_first.py.
# The data map's own locks: view_area / unview_area held them for a whole box (~420 k blocks when the box moves
# far, 95-160 ms) while SimBubble's column reads and try_set_block waited. The clipbox pass now hands them x slabs of
# about 4096 blocks and yields between slabs.
# python voxel_patch_lock_holds.py <godot_voxel> [timer|reserve]   (A/B: the timer only / timer + reserve, no slabs)
import sys
root = sys.argv[1]
mode = sys.argv[2] if len(sys.argv) > 2 else 'all'

def patch(rel, old, new):
    p = root + '/' + rel
    s = open(p, encoding='utf-8').read()
    if s.count(new) == 1 and (new.count(old) or s.count(old) == 0):
        return  # already done
    assert s.count(old) == 1, (rel, old)
    s = s.replace(old, new, 1)
    open(p, 'w', encoding='utf-8', newline='\n').write(s)

S = 'terrain/variable_lod/voxel_lod_terrain_update_clipbox_streaming.cpp'
patch(S, '''#include <atomic> // TechnoApe ADR 0136
''', '''#include <atomic> // TechnoApe ADR 0136
#include <chrono>
''')
patch(S, '''namespace zylann::voxel {
''', '''namespace zylann::voxel {

// TechnoApe ADR 0136: the update thread's holds of lod.loading_blocks_mutex (run maxima, read by get_statistics)
std::atomic<uint64_t> g_ta_loading_hold_max{ 0 };
std::atomic<uint64_t> g_ta_loading_hold_slow_n{ 0 };
std::atomic<uint64_t> g_ta_loading_rehash_n{ 0 };

struct TaLoadingHold {
	const StdUnorderedMap<Vector3i, VoxelLodTerrainUpdateData::LoadingDataBlock> &map;
	const size_t buckets = map.bucket_count();
	const std::chrono::steady_clock::time_point t0 = std::chrono::steady_clock::now();
	~TaLoadingHold() {
		const uint64_t us = uint64_t(
				std::chrono::duration_cast<std::chrono::microseconds>(std::chrono::steady_clock::now() - t0).count()
		);
		uint64_t cur = g_ta_loading_hold_max.load();
		while (us > cur && !g_ta_loading_hold_max.compare_exchange_weak(cur, us)) {
		}
		if (us >= 1000) {
			g_ta_loading_hold_slow_n.fetch_add(1);
		}
		if (map.bucket_count() != buckets) {
			g_ta_loading_rehash_n.fetch_add(1);
		}
	}
};
''')
# declared after the lock, so it is destroyed (and stops timing) before the unlock
patch(S, '''							MutexLock mlock(lod.loading_blocks_mutex);
							for (size_t i = i0; i < tls_missing_blocks.size() && i < i0 + LOCK_CHUNK; ++i) {''', '''							MutexLock mlock(lod.loading_blocks_mutex);
							TaLoadingHold ta_hold{ lod.loading_blocks };
							for (size_t i = i0; i < tls_missing_blocks.size() && i < i0 + LOCK_CHUNK; ++i) {''')
patch(S, '''						MutexLock mlock(lod.loading_blocks_mutex);
						for (size_t i = i0; i < tls_found_blocks_positions.size() && i < i0 + 256; ++i) {''', '''						MutexLock mlock(lod.loading_blocks_mutex);
						TaLoadingHold ta_hold{ lod.loading_blocks };
						for (size_t i = i0; i < tls_found_blocks_positions.size() && i < i0 + 256; ++i) {''')
patch(S, '''							MutexLock mlock(lod.loading_blocks_mutex);
							for (size_t i = i0; i < tls_missing_blocks.size() && i < i0 + 256; ++i) {''', '''							MutexLock mlock(lod.loading_blocks_mutex);
							TaLoadingHold ta_hold{ lod.loading_blocks };
							for (size_t i = i0; i < tls_missing_blocks.size() && i < i0 + 256; ++i) {''')

C = 'terrain/variable_lod/voxel_lod_terrain.cpp'
patch(C, '''extern std::atomic<uint64_t> g_ta_mesh_phase[12]; // TechnoApe ADR 0135, mesh_block_task.cpp
''', '''extern std::atomic<uint64_t> g_ta_mesh_phase[12]; // TechnoApe ADR 0135, mesh_block_task.cpp
// TechnoApe ADR 0136, voxel_lod_terrain_update_clipbox_streaming.cpp
extern std::atomic<uint64_t> g_ta_loading_hold_max, g_ta_loading_hold_slow_n, g_ta_loading_rehash_n;
''')
patch(C, '''	d["ta_slowest_task"] = String(g_ta_slowest_task);
''', '''	d["ta_slowest_task"] = String(g_ta_slowest_task);
	d["ta_loading_hold_max"] = int64_t(g_ta_loading_hold_max.load()); // run maxima, not reset on read
	d["ta_loading_hold_slow_n"] = int64_t(g_ta_loading_hold_slow_n.load());
	d["ta_loading_rehash_n"] = int64_t(g_ta_loading_rehash_n.load());
''')
if mode != 'timer':
    patch(C, '''	_data->reserve_blocks(0, size_t(math::min(across * across * across, uint64_t(4) << 20)));
}''', '''	_data->reserve_blocks(0, size_t(math::min(across * across * across, uint64_t(4) << 20)));
	// TechnoApe ADR 0136: and the loading blocks, which rehashed under loading_blocks_mutex while the main thread waited
	VoxelLodTerrainUpdateData::Lod &lod = _update_data->state.lods[0];
	MutexLock lock(lod.loading_blocks_mutex);
	lod.loading_blocks.reserve(size_t(math::min(across * across * across, uint64_t(4) << 20)));
}''')

if mode == 'all':
    patch(S, '''void add_loading_block(
''', '''// TechnoApe ADR 0136: f over x slabs of `box` of about 4096 blocks, yielding between them, so view_area and
// unview_area do not hold the data map's locks for a whole box while the main thread waits for them
template <typename F>
void for_each_lock_slab(const Box3i &box, F f) {
	const int w = math::max(4096 / math::max(box.size.y * box.size.z, 1), 1);
	for (int x = 0; x < box.size.x; x += w) {
		if (x > 0) {
			std::this_thread::yield();
		}
		f(Box3i(box.position + Vector3i(x, 0, 0), Vector3i(math::min(w, box.size.x - x), box.size.y, box.size.z)));
	}
}

void add_loading_block(
''')
    patch(S, '''					auto view = [&data, lod_index](Box3i box_to_load) {
						data.view_area(box_to_load, lod_index, &tls_missing_blocks, nullptr, nullptr);
					};''', '''					auto view = [&data, lod_index](Box3i box_to_load) {
						for_each_lock_slab(box_to_load, [&data, lod_index](Box3i b) { // TechnoApe ADR 0136
							data.view_area(b, lod_index, &tls_missing_blocks, nullptr, nullptr);
						});
					};''')
    patch(S, '''					auto unview = [&data, blocks_to_save, lod_index](Box3i box_to_remove) {
						data.unview_area(
								box_to_remove,
								lod_index,
								&tls_found_blocks_positions,
								&tls_missing_blocks,
								blocks_to_save
						);''', '''					auto unview = [&data, blocks_to_save, lod_index](Box3i box_to_remove) {
						for_each_lock_slab(box_to_remove, [&data, blocks_to_save, lod_index](Box3i b) { // TechnoApe ADR 0136
						data.unview_area(
								b,
								lod_index,
								&tls_found_blocks_positions,
								&tls_missing_blocks,
								blocks_to_save
						);
						});''')
