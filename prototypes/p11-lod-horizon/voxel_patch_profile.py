# --- main-thread timers: where VoxelLodTerrain / VoxelEngine spend _process ---
# Part of the TechnoApe godot_voxel patches: run after the others.
# A 100 m box stalled the main thread ~230 ms while the update task itself took
# 5-36 ms (2026-10-28). These timers keep the worst value of each main-thread
# step since the last VoxelLodTerrain.get_statistics() call ("ta_*" keys, usec).
import sys
root = sys.argv[1]

def patch(rel, old, new):
    p = root + '/' + rel
    s = open(p, encoding='utf-8').read()
    if new in s:   # already applied
        return
    assert s.count(old) == 1, (rel, old)
    s = s.replace(old, new, 1)
    open(p, 'w', encoding='utf-8', newline='\n').write(s)

H = 'engine/voxel_engine.h'
patch(H, 'namespace zylann::voxel {\n', '''namespace zylann::voxel {

// TechnoApe profile: worst main-thread step times (usec) since the last read
enum TaProf { TA_DEQUEUE, TA_DEQUEUE_N, TA_TIME_SPREAD, TA_PROGRESSIVE, TA_SYNC_VIEWERS, TA_PUSH_MESHES,
	TA_COLLISION, TA_APPLY_UPDATE, TA_NEW_TASK, TA_FADING,
	TA_TASK_MAX, TA_RESP_LOADING_LOCK, TA_RESP_SET_BLOCK, TA_RESP_LOADED_LOCK, TA_TASK_APPLY, TA_TASK_DELETE, TA_GEN_STREAM, TA_GEN_CALLBACK,
	TA_RESP_ERASE_LOCK, TA_RESP_ERASE, TA_RESP_FIND, TA_GEN_RUN_SUM, TA_GEN_RUN_N, TA_COUNT };
extern std::atomic<uint64_t> g_ta_prof[TA_COUNT];
extern const char *g_ta_slowest_task;
inline void ta_prof_max(TaProf i, uint64_t v) {
	uint64_t cur = g_ta_prof[i].load();
	while (v > cur && !g_ta_prof[i].compare_exchange_weak(cur, v)) {
	}
}
inline uint64_t ta_now_us() {
	return std::chrono::duration_cast<std::chrono::microseconds>(std::chrono::steady_clock::now().time_since_epoch())
			.count();
}
''')
patch(H, '#include "../meshers/voxel_mesher.h"\n', '#include <atomic>\n#include <chrono>\n#include "../meshers/voxel_mesher.h"\n')

E = 'engine/voxel_engine.cpp'
patch(E, '''	// Receive generation and meshing results
	_general_thread_pool.dequeue_completed_tasks([](zylann::IThreadedTask *task) {
		task->apply_result();
		ZN_DELETE(task);
	});

	// Run this after dequeueing threaded tasks, because they can add some to this runner,
	// which could in turn complete right away (we avoid 1-frame delays this way).
	_time_spread_task_runner.process(_main_thread_time_budget_usec);

	_progressive_task_runner.process();

	// Update viewer dependencies
	sync_viewers_task_priority_data();
''', '''	// Receive generation and meshing results
	uint64_t ta = ta_now_us();
	uint64_t ta_n = 0;
	_general_thread_pool.dequeue_completed_tasks([&ta_n](zylann::IThreadedTask *task) {
		const uint64_t t0 = ta_now_us();
		const char *name = task->get_debug_name();
		task->apply_result();
		const uint64_t t1 = ta_now_us();
		ZN_DELETE(task);
		const uint64_t t2 = ta_now_us();
		if (t2 - t0 > g_ta_prof[TA_TASK_MAX].load()) {
			g_ta_slowest_task = name;
		}
		ta_prof_max(TA_TASK_MAX, t2 - t0);
		ta_prof_max(TA_TASK_APPLY, t1 - t0);
		ta_prof_max(TA_TASK_DELETE, t2 - t1);
		++ta_n;
	});
	uint64_t ta2 = ta_now_us();
	ta_prof_max(TA_DEQUEUE, ta2 - ta);
	ta_prof_max(TA_DEQUEUE_N, ta_n);

	// Run this after dequeueing threaded tasks, because they can add some to this runner,
	// which could in turn complete right away (we avoid 1-frame delays this way).
	_time_spread_task_runner.process(_main_thread_time_budget_usec);
	ta = ta_now_us();
	ta_prof_max(TA_TIME_SPREAD, ta - ta2);

	_progressive_task_runner.process();
	ta2 = ta_now_us();
	ta_prof_max(TA_PROGRESSIVE, ta2 - ta);

	// Update viewer dependencies
	sync_viewers_task_priority_data();
	ta_prof_max(TA_SYNC_VIEWERS, ta_now_us() - ta2);
''')
patch(E, '#include "voxel_engine.h"\n', '#include "voxel_engine.h"\n\nnamespace zylann::voxel {\nstd::atomic<uint64_t> g_ta_prof[TA_COUNT];\nconst char *g_ta_slowest_task = "";\n}\n')

L = 'terrain/variable_lod/voxel_lod_terrain.cpp'
patch(L, '''	push_deferred_mesh_updates(); // TechnoApe P11: new frame, new upload budget

	// TODO This could go into time spread tasks too
	process_deferred_collision_updates(VoxelEngine::get_singleton().get_main_thread_time_budget_usec());
''', '''	uint64_t ta = ta_now_us();
	push_deferred_mesh_updates(); // TechnoApe P11: new frame, new upload budget
	uint64_t ta2 = ta_now_us();
	ta_prof_max(TA_PUSH_MESHES, ta2 - ta);

	// TODO This could go into time spread tasks too
	process_deferred_collision_updates(VoxelEngine::get_singleton().get_main_thread_time_budget_usec());
	ta_prof_max(TA_COLLISION, ta_now_us() - ta2);
''')
patch(L, '''		ZN_PROFILE_SCOPE();

		apply_main_thread_update_tasks();
''', '''		ZN_PROFILE_SCOPE();

		ta = ta_now_us();
		apply_main_thread_update_tasks();
		ta2 = ta_now_us();
		ta_prof_max(TA_APPLY_UPDATE, ta2 - ta);
''')
patch(L, '''			VoxelEngine::get_singleton().push_async_task(task);

		} else {''', '''			VoxelEngine::get_singleton().push_async_task(task);
			ta_prof_max(TA_NEW_TASK, ta_now_us() - ta2);

		} else {''')
patch(L, '''	// Do it after we change mesh block states so materials are updated
	process_fading_blocks(delta);
''', '''	// Do it after we change mesh block states so materials are updated
	ta = ta_now_us();
	process_fading_blocks(delta);
	ta_prof_max(TA_FADING, ta_now_us() - ta);
''')
patch(L, '''	d["blocked_lods"] = _stats.blocked_lods;
''', '''	d["blocked_lods"] = _stats.blocked_lods;
	static const char *ta_names[TA_COUNT] = { "ta_dequeue", "ta_dequeue_n", "ta_time_spread", "ta_progressive",
		"ta_sync_viewers", "ta_push_meshes", "ta_collision", "ta_apply_update", "ta_new_task", "ta_fading",
		"ta_task_max", "ta_resp_loading_lock", "ta_resp_set_block", "ta_resp_loaded_lock", "ta_task_apply", "ta_task_delete", "ta_gen_stream", "ta_gen_callback",
		"ta_resp_erase_lock", "ta_resp_erase", "ta_resp_find", "ta_gen_run_sum", "ta_gen_run_n" };
	d["ta_slowest_task"] = String(g_ta_slowest_task);
	for (int i = 0; i < TA_COUNT; ++i) {
		d[ta_names[i]] = int64_t(i >= TA_GEN_RUN_SUM ? g_ta_prof[i].load() : g_ta_prof[i].exchange(0));   // the sums run on
	}
''')

patch(L, """		bool was_loading = false;
		{
			MutexLock lock(lod.loading_blocks_mutex);""", """		bool was_loading = false;
		{
			const uint64_t t0 = ta_now_us();
			MutexLock lock(lod.loading_blocks_mutex);
			ta_prof_max(TA_RESP_LOADING_LOCK, ta_now_us() - t0);""")
patch(L, """	const bool inserted = _data->try_set_block(ob.position, block);""", """	uint64_t ta = ta_now_us();
	const bool inserted = _data->try_set_block(ob.position, block);
	ta_prof_max(TA_RESP_SET_BLOCK, ta_now_us() - ta);""")
patch(L, """		VoxelLodTerrainUpdateData::ClipboxStreamingState &cs = _update_data->state.clipbox_streaming;
		MutexLock mlock(cs.loaded_data_blocks_mutex);
		cs.loaded_data_blocks.push_back(""", """		VoxelLodTerrainUpdateData::ClipboxStreamingState &cs = _update_data->state.clipbox_streaming;
		ta = ta_now_us();
		MutexLock mlock(cs.loaded_data_blocks_mutex);
		ta_prof_max(TA_RESP_LOADED_LOCK, ta_now_us() - ta);
		cs.loaded_data_blocks.push_back(""")

G = 'generators/generate_block_task.cpp'
patch(G, """			Ref<VoxelStream> stream = _stream_dependency->stream;

			VoxelEngine::BlockDataOutput o;""", """			uint64_t ta = ta_now_us();
			Ref<VoxelStream> stream = _stream_dependency->stream;

			VoxelEngine::BlockDataOutput o;""")
patch(G, """			VoxelEngine::VolumeCallbacks callbacks = VoxelEngine::get_singleton().get_volume_callbacks(_volume_id);
			ERR_FAIL_COND(callbacks.data_output_callback == nullptr);
			callbacks.data_output_callback(callbacks.data, o);
""", """			uint64_t ta2 = ta_now_us();
			ta_prof_max(TA_GEN_STREAM, ta2 - ta);
			VoxelEngine::VolumeCallbacks callbacks = VoxelEngine::get_singleton().get_volume_callbacks(_volume_id);
			ERR_FAIL_COND(callbacks.data_output_callback == nullptr);
			callbacks.data_output_callback(callbacks.data, o);
			ta_prof_max(TA_GEN_CALLBACK, ta_now_us() - ta2);
""")
patch(L, """		MutexLock lock(lod.loading_blocks_mutex);
		lod.loading_blocks.erase(ob.position);
	}""", """		uint64_t ta3 = ta_now_us();
		MutexLock lock(lod.loading_blocks_mutex);
		uint64_t ta4 = ta_now_us();
		lod.loading_blocks.erase(ob.position);
		ta_prof_max(TA_RESP_ERASE_LOCK, ta4 - ta3);
		ta_prof_max(TA_RESP_ERASE, ta_now_us() - ta4);
	}""")
patch(L, """			auto it = lod.loading_blocks.find(ob.position);
			if (it != lod.loading_blocks.end()) {
				was_loading = true;
				viewers = it->second.viewers;
			}""", """			const uint64_t t1 = ta_now_us();
			auto it = lod.loading_blocks.find(ob.position);
			if (it != lod.loading_blocks.end()) {
				was_loading = true;
				viewers = it->second.viewers;
			}
			ta_prof_max(TA_RESP_FIND, ta_now_us() - t1);""")
patch(G, """	{
		run_cpu_generation();""", """	{
		const uint64_t ta = ta_now_us();
		run_cpu_generation();
		g_ta_prof[TA_GEN_RUN_SUM] += ta_now_us() - ta;   // sums, not maxima: worker time over all generation tasks
		g_ta_prof[TA_GEN_RUN_N] += 1;""")
