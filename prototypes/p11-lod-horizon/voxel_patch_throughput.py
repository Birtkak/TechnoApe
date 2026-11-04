# --- ADR 0135: worker time by task kind, so a flight can tell which stage limits the box ---
# Part of the TechnoApe godot_voxel patches: run after voxel_patch_profile.py and voxel_patch_mesh_speed.py.
# Sums (usec, since start) of worker time in GenerateBlock / MeshBlock / LoadBlockData / SaveBlockData /
# VoxelLodTerrainUpdate / other tasks, in the task pickup (queue lock, heap) and idle (semaphore wait, park,
# serial-retry sleeps); VoxelLodTerrain.get_statistics() reports them as ta_run_<kind>_us / _n.
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

H = 'util/tasks/threaded_task_runner.h'
patch(H, '''inline std::atomic<bool> g_main_applying_results{ false };
''', '''inline std::atomic<bool> g_main_applying_results{ false };

// TechnoApe ADR 0135: worker time (usec sums) and counts by task kind, plus pickup and idle time
enum TaRunKind { TA_RUN_GEN, TA_RUN_MESH, TA_RUN_LOAD, TA_RUN_SAVE, TA_RUN_UPDATE, TA_RUN_OTHER, TA_RUN_PICK, TA_RUN_IDLE,
	TA_RUN_COUNT };
inline std::atomic<uint64_t> g_ta_run_us[TA_RUN_COUNT];
inline std::atomic<uint64_t> g_ta_run_n[TA_RUN_COUNT];
inline uint64_t ta_run_now_us() {
	return std::chrono::duration_cast<std::chrono::microseconds>(std::chrono::steady_clock::now().time_since_epoch())
			.count();
}
inline TaRunKind ta_run_kind(const char *name) {
	if (name == nullptr) {
		return TA_RUN_OTHER;
	}
	switch (name[0]) {
		case 'G':
			return TA_RUN_GEN; // GenerateBlock
		case 'M':
			return TA_RUN_MESH; // MeshBlock
		case 'L':
			return TA_RUN_LOAD; // LoadBlockData
		case 'S':
			return TA_RUN_SAVE; // SaveBlockData
		case 'V':
			return TA_RUN_UPDATE; // VoxelLodTerrainUpdate
		default:
			return TA_RUN_OTHER;
	}
}
''')

C = 'util/tasks/threaded_task_runner.cpp'
# idle: parked workers
patch(C, '''			data.waiting = true;
			Thread::sleep_usec(2000);
			continue;
		}
		data.waiting = false;
		bool is_running_serial_task = false;
		bool task_queue_was_empty = false;
		{''', '''			data.waiting = true;
			Thread::sleep_usec(2000);
			continue;
		}
		data.waiting = false;
		bool is_running_serial_task = false;
		bool task_queue_was_empty = false;
		const uint64_t ta_pick0 = ta_run_now_us(); // TechnoApe ADR 0135
		{''')
patch(C, '''		if (cancelled_tasks.size() > 0) {
			MutexLock lock(_completed_tasks_mutex);''', '''		g_ta_run_us[TA_RUN_PICK] += ta_run_now_us() - ta_pick0; // TechnoApe ADR 0135
		g_ta_run_n[TA_RUN_PICK] += 1;
		if (cancelled_tasks.size() > 0) {
			MutexLock lock(_completed_tasks_mutex);''')
patch(C, '''				data.waiting = true;
				_tasks_semaphore.wait();''', '''				data.waiting = true;
				const uint64_t ta_idle0 = ta_run_now_us(); // TechnoApe ADR 0135
				_tasks_semaphore.wait();
				g_ta_run_us[TA_RUN_IDLE] += ta_run_now_us() - ta_idle0;
				g_ta_run_n[TA_RUN_IDLE] += 1;''')
patch(C, '''				Thread::sleep_usec(1000);
			}''', '''				Thread::sleep_usec(1000);
				g_ta_run_us[TA_RUN_IDLE] += 1000; // TechnoApe ADR 0135: serial retry
			}''')
patch(C, '''					data.debug_running_task_name = item.task->get_debug_name();
					item.task->run(ctx);''', '''					data.debug_running_task_name = item.task->get_debug_name();
					const uint64_t ta_run0 = ta_run_now_us(); // TechnoApe ADR 0135
					item.task->run(ctx);
					const TaRunKind ta_kind = ta_run_kind(data.debug_running_task_name);
					g_ta_run_us[ta_kind] += ta_run_now_us() - ta_run0;
					g_ta_run_n[ta_kind] += 1;''')

T = 'terrain/variable_lod/voxel_lod_terrain.cpp'
patch(T, '''	// Process
	d["dropped_block_loads"] = _stats.dropped_block_loads;''', '''	// TechnoApe ADR 0135: worker time by task kind (usec sums since start)
	static const char *ta_run_names[TA_RUN_COUNT] = { "gen", "mesh", "load", "save", "update", "other", "pick", "idle" };
	for (int i = 0; i < TA_RUN_COUNT; ++i) {
		d[String("ta_run_") + ta_run_names[i] + "_us"] = int64_t(g_ta_run_us[i].load());
		d[String("ta_run_") + ta_run_names[i] + "_n"] = int64_t(g_ta_run_n[i].load());
	}
	int64_t upload_waiting = 0; // meshes held back by mesh_upload_budget_mb
	for (const auto &m : _deferred_mesh_updates_per_lod) {
		upload_waiting += m.size();
	}
	d["upload_waiting"] = upload_waiting;

	// Process
	d["dropped_block_loads"] = _stats.dropped_block_loads;''')
# mesh requests by cause: 0 first mesh of a block, 1 the stream asks again (state not NEVER_UPDATED), 2 an edit
U = 'terrain/variable_lod/voxel_lod_terrain_update_task.h'
patch(U, '''namespace zylann::voxel {
''', '''namespace zylann::voxel {

// TechnoApe ADR 0135: mesh requests by cause (first, stream again, edit), sums since start
inline std::atomic<uint64_t> g_ta_mesh_sched[3];
''')
patch('terrain/variable_lod/voxel_lod_terrain_update_clipbox_streaming.cpp', '''		TaskCancellationToken cancellation_token = TaskCancellationToken::create();
		mesh_block.update_list_index = update_list.size();''', '''		TaskCancellationToken cancellation_token = TaskCancellationToken::create();
		g_ta_mesh_sched[mesh_block.state == VoxelLodTerrainUpdateData::MESH_NEVER_UPDATED ? 0 : 1] += 1; // ADR 0135
		mesh_block.update_list_index = update_list.size();''')
patch('terrain/variable_lod/voxel_lod_terrain_update_task.h', '''				block.state = VoxelLodTerrainUpdateData::MESH_UPDATE_NOT_SENT;
				block.update_list_index = blocks_pending_update.size();''', '''				block.state = VoxelLodTerrainUpdateData::MESH_UPDATE_NOT_SENT;
				g_ta_mesh_sched[2] += 1; // TechnoApe ADR 0135
				block.update_list_index = blocks_pending_update.size();''')
patch(T, '''	d["upload_waiting"] = upload_waiting;''', '''	d["upload_waiting"] = upload_waiting;
	d["mesh_first"] = int64_t(g_ta_mesh_sched[0].load());
	d["mesh_again"] = int64_t(g_ta_mesh_sched[1].load());
	d["mesh_edit"] = int64_t(g_ta_mesh_sched[2].load());''')
# mesh task phases: 0 gather us, 1 build us, 2 tasks whose gathered buffer is uniform (no faces), 3 their us
M = 'meshers/mesh_block_task.cpp'
patch(M, '''void MeshBlockTask::run(zylann::ThreadedTaskContext &ctx) {''', '''// TechnoApe ADR 0135: mesh task phases (gather us, build us, uniform tasks, their us), sums since start
std::atomic<uint64_t> g_ta_mesh_phase[12];
static inline uint64_t ta_mesh_now_us() {
	return std::chrono::duration_cast<std::chrono::microseconds>(std::chrono::steady_clock::now().time_since_epoch())
			.count();
}

void MeshBlockTask::run(zylann::ThreadedTaskContext &ctx) {''')
patch(M, '''	{
		gather_voxels_cpu();
		build_mesh();
	}
}''', '''	{
		const uint64_t ta0 = ta_mesh_now_us();
		gather_voxels_cpu();
		const uint64_t ta1 = ta_mesh_now_us();
		const bool ta_uniform = _voxels.get_channel_compression(VoxelBuffer::CHANNEL_TYPE) == VoxelBuffer::COMPRESSION_UNIFORM;
		build_mesh();
		const uint64_t ta2 = ta_mesh_now_us();
		g_ta_mesh_phase[0] += ta1 - ta0;
		g_ta_mesh_phase[1] += ta2 - ta1;
		if (ta_uniform) {
			g_ta_mesh_phase[2] += 1;
			g_ta_mesh_phase[3] += ta2 - ta0;
		}
	}
}''')
patch(M, '''#include "mesh_block_task.h"
''', '''#include "mesh_block_task.h"
#include <atomic>
#include <chrono>
''')
patch(T, '''	d["mesh_first"] = int64_t(g_ta_mesh_sched[0].load());''', '''	d["mesh_first"] = int64_t(g_ta_mesh_sched[0].load());
	static const char *ta_mp_names[12] = { "mesh_gather_us", "mesh_build_us", "mesh_uniform_n", "mesh_uniform_us",
		"mesh_faces_us", "mesh_ao_us", "mesh_arrays_us", "mesh_occluders_us", "mesh_loop_us", "mesh_aovol_us",
		"mesh_merge_us", "mesh_faces_n" };
	for (int i = 0; i < 12; ++i) {
		d[ta_mp_names[i]] = int64_t(g_ta_mesh_phase[i].load());
	}''')
patch(T, '''namespace zylann::voxel {
''', '''namespace zylann::voxel {
extern std::atomic<uint64_t> g_ta_mesh_phase[12]; // TechnoApe ADR 0135, mesh_block_task.cpp
''')
# blocky mesher phases: 4 faces (generate_mesh), 5 occlusion image, 6 Godot arrays, 7 shadow occluders
B = 'meshers/blocky/voxel_mesher_blocky.cpp'
patch(B, '''		switch (channel_depth) {
			case VoxelBuffer::DEPTH_8_BIT:
				blocky::generate_mesh(''', '''		uint64_t ta_t = ta_blocky_now_us(); // TechnoApe ADR 0135
		switch (channel_depth) {
			case VoxelBuffer::DEPTH_8_BIT:
				blocky::generate_mesh(''')
patch(B, '''		if (ao_volume != nullptr && ao_volume->size() > 0 && voxels.get_size().x == voxels.get_size().y &&''', '''		g_ta_mesh_phase[4] += ta_blocky_now_us() - ta_t; // TechnoApe ADR 0135
		ta_t = ta_blocky_now_us();
		if (ao_volume != nullptr && ao_volume->size() > 0 && voxels.get_size().x == voxels.get_size().y &&''')
patch(B, '''			output.atlas_image = Image::create_from_data(edge, int(ao_volume->size()) / edge, false, Image::FORMAT_R8, bytes);
		}
	}
''', '''			output.atlas_image = Image::create_from_data(edge, int(ao_volume->size()) / edge, false, Image::FORMAT_R8, bytes);
		}
		g_ta_mesh_phase[5] += ta_blocky_now_us() - ta_t; // TechnoApe ADR 0135
	}
	const uint64_t ta_arrays0 = ta_blocky_now_us();
''')
patch(B, '''	if (params.shadow_occluders_mask != 0 && !blocky::is_empty(arrays_per_material)) {''', '''	const uint64_t ta_occ0 = ta_blocky_now_us(); // TechnoApe ADR 0135
	g_ta_mesh_phase[6] += ta_occ0 - ta_arrays0;
	struct TaOcc {
		uint64_t t0;
		~TaOcc() { g_ta_mesh_phase[7] += ta_blocky_now_us() - t0; }
	} ta_occ{ ta_occ0 };
	if (params.shadow_occluders_mask != 0 && !blocky::is_empty(arrays_per_material)) {''')
patch(B, '''#include "voxel_mesher_blocky.h"
''', '''#include "voxel_mesher_blocky.h"
#include <atomic>
#include <chrono>
''')
# inside generate_mesh: 8 voxel loop, 9 occlusion bit volume, 10 merge pass, 11 marked faces
patch(B, '''	// TechnoApe ADR 0135: the merge pass zeroes every mark it uses''', '''	const uint64_t ta_g0 = ta_blocky_now_us(); // TechnoApe ADR 0135
	// TechnoApe ADR 0135: the merge pass zeroes every mark it uses''')
patch(B, '''								merge_marked[side].push_back(voxel_index); // TechnoApe ADR 0135
''', '''								merge_marked[side].push_back(voxel_index); // TechnoApe ADR 0135
								g_ta_mesh_phase[11] += 1;
''')
patch(B, '''	if (paging) {
		// TechnoApe ADR 0107: one bit per voxel of the block and one voxel around it, set where it darkens corners''', '''	const uint64_t ta_g1 = ta_blocky_now_us(); // TechnoApe ADR 0135
	g_ta_mesh_phase[8] += ta_g1 - ta_g0;
	if (paging) {
		// TechnoApe ADR 0107: one bit per voxel of the block and one voxel around it, set where it darkens corners''')
patch(B, '''	if (!merging) {
		return;
	}
	// TechnoApe ADR 0106: per side and slice''', '''	const uint64_t ta_g2 = ta_blocky_now_us(); // TechnoApe ADR 0135
	g_ta_mesh_phase[9] += ta_g2 - ta_g1;
	struct TaMerge {
		uint64_t t0;
		~TaMerge() { g_ta_mesh_phase[10] += ta_blocky_now_us() - t0; }
	} ta_merge{ ta_g2 };
	if (!merging) {
		return;
	}
	// TechnoApe ADR 0106: per side and slice''')
# the extern must precede generate_mesh (namespace blocky): move it to the top of the file
patch(B, '''namespace blocky {

inline bool contributes_to_ao(''', '''extern std::atomic<uint64_t> g_ta_mesh_phase[12]; // TechnoApe ADR 0135, mesh_block_task.cpp
static inline uint64_t ta_blocky_now_us() {
	return std::chrono::duration_cast<std::chrono::microseconds>(std::chrono::steady_clock::now().time_since_epoch())
			.count();
}

namespace blocky {

inline bool contributes_to_ao(''')
print('throughput counters patched')
