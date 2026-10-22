# --- per-frame mesh upload budget (ADR 0020 / ADR 0016 open item) ---
# Part of voxel_patch.py (kept in its own file: run it after voxel_patch.py).
# Mesh updates are applied on the main thread through VoxelEngine's time-spread
# runner, whose budget only counts main-thread time; the GPU upload of the new
# meshes lands at the frame's sync point. A byte cap per frame defers the rest to
# the next frame; a newer update for the same block replaces a waiting one, so a
# flowing liquid steps slower instead of stalling the frame.
import sys
root = sys.argv[1]

p = root + '/terrain/variable_lod/voxel_lod_terrain.h'
s = open(p, encoding='utf-8').read()
old = '\tvoid set_lod_fade_duration(float seconds);\n\tfloat get_lod_fade_duration() const;\n'
assert s.count(old) == 1
s = s.replace(old, old + '''
	// TechnoApe P11: cap on mesh data applied per frame in MB (0 = unlimited)
	void set_mesh_upload_budget_mb(float mb);
	float get_mesh_upload_budget_mb() const;
''', 1)
old = '\tFixedArray<StdUnorderedMap<Vector3i, RefCount>, constants::MAX_LOD> _queued_main_thread_mesh_updates;\n'
assert s.count(old) == 1
s = s.replace(old, old + '''
	// TechnoApe P11: mesh upload budget
	uint64_t _mesh_upload_budget_bytes = 0;
	uint64_t _mesh_upload_bytes_this_frame = 0;
	FixedArray<StdUnorderedMap<Vector3i, VoxelEngine::BlockMeshOutput>, constants::MAX_LOD> _deferred_mesh_updates_per_lod;
	void enqueue_mesh_update(VoxelEngine::BlockMeshOutput &ob);
	void push_deferred_mesh_updates();
''', 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)

p = root + '/terrain/variable_lod/voxel_lod_terrain.cpp'
s = open(p, encoding='utf-8').read()
old = '''void VoxelLodTerrain::ApplyMeshUpdateTask::run(TimeSpreadTaskContext &ctx) {'''
assert s.count(old) == 1
s = s.replace(old, '''namespace {
// TechnoApe P11: rough size of the mesh data this update carries (vertex ~32 B, index 4 B)
uint64_t estimate_mesh_bytes(const VoxelEngine::BlockMeshOutput &ob) {
	uint64_t bytes = 0;
	Ref<ArrayMesh> am = ob.mesh;
	if (am.is_valid()) {
		for (int i = 0; i < am->get_surface_count(); ++i) {
			bytes += uint64_t(am->surface_get_array_len(i)) * 32 + uint64_t(am->surface_get_array_index_len(i)) * 4;
		}
	} else {
		for (const VoxelMesher::Output::Surface &surface : ob.surfaces.surfaces) {
			if (surface.arrays.size() > Mesh::ARRAY_VERTEX) {
				PackedVector3Array v = surface.arrays[Mesh::ARRAY_VERTEX];
				bytes += uint64_t(v.size()) * 32;
			}
		}
	}
	return bytes;
}
} // namespace

''' + old, 1)
old = '''		queued_tasks_in_lod.erase(it);
	}

	self->apply_mesh_update(data);
}'''
assert s.count(old) == 1
s = s.replace(old, '''		queued_tasks_in_lod.erase(it);
	}

	// TechnoApe P11: over this frame's upload budget -> wait a frame (newest update per block wins)
	if (self->_mesh_upload_budget_bytes > 0 && self->_mesh_upload_bytes_this_frame >= self->_mesh_upload_budget_bytes) {
		self->_deferred_mesh_updates_per_lod[data.lod][data.position] = std::move(data);
		return;
	}
	self->_mesh_upload_bytes_this_frame += estimate_mesh_bytes(data);
	self->_deferred_mesh_updates_per_lod[data.lod].erase(data.position); // an older waiting update is stale now

	self->apply_mesh_update(data);
}

void VoxelLodTerrain::enqueue_mesh_update(VoxelEngine::BlockMeshOutput &ob) {
	const Vector3i position = ob.position;
	const uint8_t lod = ob.lod;
	ApplyMeshUpdateTask *task = ZN_NEW(ApplyMeshUpdateTask);
	task->volume_id = get_volume_id();
	task->self = this;
	task->data = std::move(ob);
	VoxelEngine::get_singleton().push_main_thread_time_spread_task(task);

	// If two tasks are queued for the same mesh, cancel the old ones.
	StdUnorderedMap<Vector3i, RefCount> &queued_tasks_in_lod = _queued_main_thread_mesh_updates[lod];
	auto p = queued_tasks_in_lod.insert({ position, RefCount(1) });
	if (!p.second) {
		p.first->second.add();
	}
}

void VoxelLodTerrain::push_deferred_mesh_updates() {
	_mesh_upload_bytes_this_frame = 0;
	for (unsigned int lod_index = 0; lod_index < _deferred_mesh_updates_per_lod.size(); ++lod_index) {
		StdUnorderedMap<Vector3i, VoxelEngine::BlockMeshOutput> &deferred = _deferred_mesh_updates_per_lod[lod_index];
		if (deferred.empty()) {
			continue;
		}
		for (auto &kv : deferred) {
			enqueue_mesh_update(kv.second);
		}
		deferred.clear();
	}
}

void VoxelLodTerrain::set_mesh_upload_budget_mb(float mb) {
	_mesh_upload_budget_bytes = uint64_t(math::max(mb, 0.f) * 1048576.f);
}

float VoxelLodTerrain::get_mesh_upload_budget_mb() const {
	return float(_mesh_upload_budget_bytes) / 1048576.f;
}''', 1)
old = '''	callbacks.mesh_output_callback = [](void *cb_data, VoxelEngine::BlockMeshOutput &ob) {
		VoxelLodTerrain *self = reinterpret_cast<VoxelLodTerrain *>(cb_data);
		ApplyMeshUpdateTask *task = ZN_NEW(ApplyMeshUpdateTask);
		task->volume_id = self->get_volume_id();
		task->self = self;
		task->data = std::move(ob);
		VoxelEngine::get_singleton().push_main_thread_time_spread_task(task);

		// If two tasks are queued for the same mesh, cancel the old ones.
		// This is for cases where creating the mesh is slower than the speed at which it is generated,
		// which can cause a buildup that never seems to stop.
		// This is at the expense of holes appearing until all tasks are done.
		StdUnorderedMap<Vector3i, RefCount> &queued_tasks_in_lod = self->_queued_main_thread_mesh_updates[ob.lod];
		auto p = queued_tasks_in_lod.insert({ ob.position, RefCount(1) });
		if (!p.second) {
			p.first->second.add();
		}
	};'''
assert s.count(old) == 1
s = s.replace(old, '''	callbacks.mesh_output_callback = [](void *cb_data, VoxelEngine::BlockMeshOutput &ob) {
		VoxelLodTerrain *self = reinterpret_cast<VoxelLodTerrain *>(cb_data);
		self->enqueue_mesh_update(ob); // TechnoApe P11: shared with deferred updates
	};''', 1)
old = '''	// TODO This could go into time spread tasks too
	process_deferred_collision_updates(VoxelEngine::get_singleton().get_main_thread_time_budget_usec());'''
assert s.count(old) == 1
s = s.replace(old, '''	push_deferred_mesh_updates(); // TechnoApe P11: new frame, new upload budget

''' + old, 1)
old = '''	ClassDB::bind_method(D_METHOD("set_lod_fade_duration", "seconds"), &Self::set_lod_fade_duration);'''
assert s.count(old) == 1
s = s.replace(old, old + '''
	ClassDB::bind_method(D_METHOD("get_mesh_upload_budget_mb"), &Self::get_mesh_upload_budget_mb);
	ClassDB::bind_method(D_METHOD("set_mesh_upload_budget_mb", "mb"), &Self::set_mesh_upload_budget_mb);''', 1)
old = '''	ADD_PROPERTY(PropertyInfo(Variant::FLOAT, "lod_fade_duration"), "set_lod_fade_duration", "get_lod_fade_duration");'''
assert s.count(old) == 1
s = s.replace(old, old + '''
	ADD_PROPERTY(PropertyInfo(Variant::FLOAT, "mesh_upload_budget_mb"), "set_mesh_upload_budget_mb", "get_mesh_upload_budget_mb");''', 1)
if '#include "../../util/godot/classes/array_mesh.h"' not in s:
    s = s.replace('#include "voxel_lod_terrain.h"', '#include "voxel_lod_terrain.h"\n#include "../../util/godot/classes/array_mesh.h"', 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)
print('patched upload-budget')
