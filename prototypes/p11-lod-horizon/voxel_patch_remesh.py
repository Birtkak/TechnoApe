# --- rate limit for edit-caused remeshes (ADR 0020 / ADR 0016 open item) ---
# Part of the TechnoApe godot_voxel patches: run after voxel_patch.py and
# voxel_patch_budget.py. The mesh upload that stalls a frame happens when worker
# threads build the meshes, so the throttle sits where remeshes are scheduled:
# VoxelLodTerrain.edit_remesh_rate (per second, 0 = unlimited) is a token bucket
# over the mesh updates that edits (VoxelTool pastes, generator changes) request;
# updates over the rate stay pending and go out in later updates. Streaming
# (first-time meshes of new blocks) is not limited.
import sys
root = sys.argv[1]

p = root + '/terrain/variable_lod/voxel_lod_terrain_update_data.h'
s = open(p, encoding='utf-8').read()
old = '''	struct MeshToUpdate {
		Vector3i position;
		TaskCancellationToken cancellation_token;
		bool require_visual = false;
	};'''
assert s.count(old) == 1
s = s.replace(old, '''	struct MeshToUpdate {
		Vector3i position;
		TaskCancellationToken cancellation_token;
		bool require_visual = false;
		bool from_edit = false; // TechnoApe P11: requested by an edit (rate-limited), not by streaming
	};''', 1)
old = '\t\tbool collision_enabled = true;\n'
assert s.count(old) == 1
s = s.replace(old, old + '\t\t// TechnoApe P11: edit-caused remeshes per second, 0 = unlimited\n\t\tfloat edit_remesh_rate = 0.f;\n', 1)
old = '\t\tEditNotificationInputs edit_notifications;\n'
assert s.count(old) == 1
s = s.replace(old, old + '''
		// TechnoApe P11: token bucket for edit-caused remeshes
		struct EditRemeshBudget {
			uint64_t last_usec = 0;
			float tokens = 0.f;
		};
		EditRemeshBudget edit_remesh;
''', 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)

p = root + '/terrain/variable_lod/voxel_lod_terrain_update_task.h'
s = open(p, encoding='utf-8').read()
old = '''			StdVector<VoxelLodTerrainUpdateData::MeshToUpdate> &blocks_pending_update,
			const bool require_visual
	) {'''
assert s.count(old) == 1
s = s.replace(old, '''			StdVector<VoxelLodTerrainUpdateData::MeshToUpdate> &blocks_pending_update,
			const bool require_visual,
			const bool from_edit = false
	) {''', 1)
old = '''				blocks_pending_update.push_back(
						VoxelLodTerrainUpdateData::MeshToUpdate{ bpos, TaskCancellationToken(), require_visual }
				);'''
assert s.count(old) == 1
s = s.replace(old, '''				blocks_pending_update.push_back(VoxelLodTerrainUpdateData::MeshToUpdate{
						bpos, TaskCancellationToken(), require_visual, from_edit });''', 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)

p = root + '/terrain/variable_lod/voxel_lod_terrain_update_task.cpp'
s = open(p, encoding='utf-8').read()
# the two edit paths mark their requests
old = '''					VoxelLodTerrainUpdateTask::schedule_mesh_update(
							block_it->second,
							bpos,
							lod.mesh_blocks_pending_update,
							block_it->second.mesh_viewers.get() > 0
					);'''
assert s.count(old) == 1
s = s.replace(old, '''					VoxelLodTerrainUpdateTask::schedule_mesh_update(
							block_it->second,
							bpos,
							lod.mesh_blocks_pending_update,
							block_it->second.mesh_viewers.get() > 0,
							true // TechnoApe P11: edit-caused
					);''', 1)
old = '''					schedule_mesh_update( //
							mesh_block_it->second, //
							mesh_block_pos, //
							lod.mesh_blocks_pending_update, //
							mesh_block_it->second.mesh_viewers.get() > 0 //
					);'''
assert s.count(old) == 1
s = s.replace(old, '''					schedule_mesh_update( //
							mesh_block_it->second, //
							mesh_block_pos, //
							lod.mesh_blocks_pending_update, //
							mesh_block_it->second.mesh_viewers.get() > 0, //
							true // TechnoApe P11: edit-caused
					);''', 1)
# send_mesh_requests: token bucket, unsent edit requests stay pending
old = '''	const unsigned int lod_count = data.get_lod_count();

	for (unsigned int lod_index = 0; lod_index < lod_count; ++lod_index) {
		ZN_PROFILE_SCOPE();
		VoxelLodTerrainUpdateData::Lod &lod = state.lods[lod_index];

		for (unsigned int bi = 0; bi < lod.mesh_blocks_pending_update.size(); ++bi) {
			ZN_PROFILE_SCOPE();
			const VoxelLodTerrainUpdateData::MeshToUpdate &mesh_to_update = lod.mesh_blocks_pending_update[bi];

			auto mesh_block_it = lod.mesh_map_state.map.find(mesh_to_update.position);
			// A block must have been allocated before we ask for a mesh update
			ZN_ASSERT_CONTINUE(mesh_block_it != lod.mesh_map_state.map.end());
			VoxelLodTerrainUpdateData::MeshBlockState &mesh_block = mesh_block_it->second;
			// All blocks we get here must be in the scheduled state
			ZN_ASSERT_CONTINUE(mesh_block.state == VoxelLodTerrainUpdateData::MESH_UPDATE_NOT_SENT);
'''
assert s.count(old) == 1
s = s.replace(old, '''	const unsigned int lod_count = data.get_lod_count();

	// TechnoApe P11: edit-caused remeshes are rate-limited (token bucket); the rest stay pending
	float edit_tokens = 1e9f;
	if (settings.edit_remesh_rate > 0.f) {
		const uint64_t now = Time::get_singleton()->get_ticks_usec();
		VoxelLodTerrainUpdateData::State::EditRemeshBudget &b = state.edit_remesh;
		if (b.last_usec != 0) {
			const float burst = math::max(settings.edit_remesh_rate * 0.1f, 8.f);
			b.tokens = math::min(b.tokens + float(now - b.last_usec) * 1e-6f * settings.edit_remesh_rate, burst);
		}
		b.last_usec = now;
		edit_tokens = b.tokens;
	}

	for (unsigned int lod_index = 0; lod_index < lod_count; ++lod_index) {
		ZN_PROFILE_SCOPE();
		VoxelLodTerrainUpdateData::Lod &lod = state.lods[lod_index];
		StdVector<VoxelLodTerrainUpdateData::MeshToUpdate> kept; // over the rate: next update

		for (unsigned int bi = 0; bi < lod.mesh_blocks_pending_update.size(); ++bi) {
			ZN_PROFILE_SCOPE();
			const VoxelLodTerrainUpdateData::MeshToUpdate &mesh_to_update = lod.mesh_blocks_pending_update[bi];

			auto mesh_block_it = lod.mesh_map_state.map.find(mesh_to_update.position);
			// A block must have been allocated before we ask for a mesh update
			ZN_ASSERT_CONTINUE(mesh_block_it != lod.mesh_map_state.map.end());
			VoxelLodTerrainUpdateData::MeshBlockState &mesh_block = mesh_block_it->second;
			// All blocks we get here must be in the scheduled state
			ZN_ASSERT_CONTINUE(mesh_block.state == VoxelLodTerrainUpdateData::MESH_UPDATE_NOT_SENT);

			if (mesh_to_update.from_edit && settings.edit_remesh_rate > 0.f) {
				if (edit_tokens < 1.f) {
					kept.push_back(mesh_to_update);
					continue;
				}
				edit_tokens -= 1.f;
			}
''', 1)
old = '''			mesh_block.state = VoxelLodTerrainUpdateData::MESH_UPDATE_SENT;
			mesh_block.update_list_index = -1;
		}

		lod.mesh_blocks_pending_update.clear();
	}
}'''
assert s.count(old) == 1
s = s.replace(old, '''			mesh_block.state = VoxelLodTerrainUpdateData::MESH_UPDATE_SENT;
			mesh_block.update_list_index = -1;
		}

		// TechnoApe P11: requests over the edit rate stay pending, with their list indices fixed up
		lod.mesh_blocks_pending_update = std::move(kept);
		for (unsigned int i = 0; i < lod.mesh_blocks_pending_update.size(); ++i) {
			auto it = lod.mesh_map_state.map.find(lod.mesh_blocks_pending_update[i].position);
			if (it != lod.mesh_map_state.map.end()) {
				it->second.update_list_index = i;
			}
		}
	}
	if (settings.edit_remesh_rate > 0.f) {
		state.edit_remesh.tokens = edit_tokens;
	}
}''', 1)
if '#include "../../util/profiling_clock.h"' not in s:
    s = s.replace('#include "../../util/math/conv.h"', '#include "../../util/math/conv.h"\n#include "../../util/profiling_clock.h"', 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)

p = root + '/terrain/variable_lod/voxel_lod_terrain.h'
s = open(p, encoding='utf-8').read()
old = '\tvoid set_mesh_upload_budget_mb(float mb);\n\tfloat get_mesh_upload_budget_mb() const;\n'
assert s.count(old) == 1
s = s.replace(old, old + '''	// TechnoApe P11: edit-caused remeshes per second (0 = unlimited)
	void set_edit_remesh_rate(float rate);
	float get_edit_remesh_rate() const;
''', 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)

p = root + '/terrain/variable_lod/voxel_lod_terrain.cpp'
s = open(p, encoding='utf-8').read()
old = '''float VoxelLodTerrain::get_mesh_upload_budget_mb() const {
	return float(_mesh_upload_budget_bytes) / 1048576.f;
}'''
assert s.count(old) == 1
s = s.replace(old, old + '''

void VoxelLodTerrain::set_edit_remesh_rate(float rate) {
	_update_data->wait_for_end_of_task();
	_update_data->settings.edit_remesh_rate = math::max(rate, 0.f);
}

float VoxelLodTerrain::get_edit_remesh_rate() const {
	return _update_data->settings.edit_remesh_rate;
}''', 1)
old = '''	ClassDB::bind_method(D_METHOD("set_mesh_upload_budget_mb", "mb"), &Self::set_mesh_upload_budget_mb);'''
assert s.count(old) == 1
s = s.replace(old, old + '''
	ClassDB::bind_method(D_METHOD("get_edit_remesh_rate"), &Self::get_edit_remesh_rate);
	ClassDB::bind_method(D_METHOD("set_edit_remesh_rate", "rate"), &Self::set_edit_remesh_rate);''', 1)
old = '''	ADD_PROPERTY(PropertyInfo(Variant::FLOAT, "mesh_upload_budget_mb"), "set_mesh_upload_budget_mb", "get_mesh_upload_budget_mb");'''
assert s.count(old) == 1
s = s.replace(old, old + '''
	ADD_PROPERTY(PropertyInfo(Variant::FLOAT, "edit_remesh_rate"), "set_edit_remesh_rate", "get_edit_remesh_rate");''', 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)
print('patched edit-remesh-rate')
