# --- deferred mesh uploads, nearest first (ADR 0108) ---
# Part of the TechnoApe godot_voxel patches: run after voxel_patch_budget.py.
# Over the per-frame upload budget a finished mesh waits a frame (voxel_patch_budget.py). Every frame all waiting
# meshes were queued again in hash-map order, so while a 100 m box streamed in behind the lifted loading cover the
# ground under the player waited behind thousands of far meshes. The waiting meshes are now queued nearest to the
# camera first, and only as many as the frame's budget takes; the rest keep waiting.
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

C = 'terrain/variable_lod/voxel_lod_terrain.cpp'
patch(C, '''void VoxelLodTerrain::push_deferred_mesh_updates() {
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
}''', '''void VoxelLodTerrain::push_deferred_mesh_updates() {
	_mesh_upload_bytes_this_frame = 0;
	// TechnoApe ADR 0108: nearest to the camera first, and only what this frame's budget takes
	const Vector3 cam = get_local_camera_info().position;
	static thread_local StdVector<std::pair<float, Vector3i>> order;
	for (unsigned int lod_index = 0; lod_index < _deferred_mesh_updates_per_lod.size(); ++lod_index) {
		StdUnorderedMap<Vector3i, VoxelEngine::BlockMeshOutput> &deferred = _deferred_mesh_updates_per_lod[lod_index];
		if (deferred.empty()) {
			continue;
		}
		const float bs = float(get_mesh_block_size() << lod_index);
		order.clear();
		for (const auto &kv : deferred) {
			const Vector3 centre = (Vector3(kv.first.x, kv.first.y, kv.first.z) + Vector3(0.5f, 0.5f, 0.5f)) * bs;
			order.push_back({ float(cam.distance_squared_to(centre)), kv.first });
		}
		std::sort(order.begin(), order.end(), [](const auto &a, const auto &b) { return a.first < b.first; });
		uint64_t planned = 0;
		for (const auto &o : order) {
			if (_mesh_upload_budget_bytes > 0 && planned >= _mesh_upload_budget_bytes) {
				break;
			}
			auto it = deferred.find(o.second);
			planned += estimate_mesh_bytes(it->second);
			VoxelEngine::BlockMeshOutput ob = std::move(it->second);
			deferred.erase(it);
			enqueue_mesh_update(ob);
		}
	}
}''')
patch(C, '''#include "../../util/godot/classes/rendering_server.h"
''', '''#include "../../util/godot/classes/rendering_server.h"
#include <algorithm> // TechnoApe ADR 0108
''')
