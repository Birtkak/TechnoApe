# --- only nearby mesh blocks cast sun shadows (ADR 0105) ---
# Part of the TechnoApe godot_voxel patches: run after voxel_patch_remesh.py.
# The sun's shadow map draws every terrain block that may cast into the shadowed area, and Godot counts as caster
# everything towards the sun: at a 72 m box the shadow pass drew ~12 M of 18 M triangles, even with the shadows
# ending at 40 m. VoxelLodTerrain.shadow_caster_distance (voxels, 0 = all) turns shadow casting off for mesh blocks
# farther than that from the camera, a few times a second, touching only blocks whose state changes.
import sys
root = sys.argv[1]

def patch(rel, old, new):
    p = root + '/' + rel
    s = open(p, encoding='utf-8').read()
    assert s.count(old) == 1, (rel, old)
    s = s.replace(old, new, 1)
    open(p, 'w', encoding='utf-8', newline='\n').write(s)

H = 'terrain/variable_lod/voxel_lod_terrain.h'
patch(H, '''	void set_edit_remesh_rate(float rate);''', '''	void set_edit_remesh_rate(float rate);
	// TechnoApe ADR 0105: mesh blocks farther than this from the camera cast no shadow (voxels, 0 = all cast)
	void set_shadow_caster_distance(float d);
	float get_shadow_caster_distance() const;
	float _shadow_caster_distance = 0.f;
	uint64_t _shadow_caster_next_usec = 0;
	unsigned int _shadow_caster_passes = 0;
	StdUnorderedSet<const VoxelMeshBlockVLT *> _shadow_casters_off;
	void update_shadow_casters();''')
s = open(root + '/' + H, encoding='utf-8').read()
if 'std_unordered_set.h' not in s:
    s = s.replace('#include', '#include "../../util/containers/std_unordered_set.h"\n#include', 1)
    open(root + '/' + H, 'w', encoding='utf-8', newline='\n').write(s)

C = 'terrain/variable_lod/voxel_lod_terrain.cpp'
patch(C, '''float VoxelLodTerrain::get_edit_remesh_rate() const {
	return _update_data->settings.edit_remesh_rate;
}
''', '''float VoxelLodTerrain::get_edit_remesh_rate() const {
	return _update_data->settings.edit_remesh_rate;
}

void VoxelLodTerrain::set_shadow_caster_distance(float d) {
	_shadow_caster_distance = math::max(d, 0.f);
	_shadow_caster_next_usec = 0;
	if (_shadow_caster_distance == 0.f) {
		_on_shadow_casting_changed(); // everything casts again
		_shadow_casters_off.clear();
	}
}

float VoxelLodTerrain::get_shadow_caster_distance() const {
	return _shadow_caster_distance;
}

void VoxelLodTerrain::update_shadow_casters() {
	const uint64_t now = Time::get_singleton()->get_ticks_usec();
	if (now < _shadow_caster_next_usec) {
		return;
	}
	_shadow_caster_next_usec = now + 200000;
	// every 25th pass forgets what it set, so a block re-created at the same address is set again
	if (++_shadow_caster_passes % 25 == 0) {
		_shadow_casters_off.clear();
	}
	const RenderingServerEnums::ShadowCastingSetting on = RenderingServerEnums::ShadowCastingSetting(get_shadow_casting());
	const Vector3 cam = get_local_camera_info().position;
	const float size = float(get_mesh_block_size());
	const float d2 = _shadow_caster_distance * _shadow_caster_distance;
	StdUnorderedSet<const VoxelMeshBlockVLT *> off;
	_mesh_maps_per_lod[0].for_each_block([&](VoxelMeshBlockVLT &block) {
		// distance from the camera to the block's box
		const Vector3 lo = Vector3(block.position.x, block.position.y, block.position.z) * size;
		const Vector3 c = Vector3(
				math::clamp(cam.x, lo.x, lo.x + size), math::clamp(cam.y, lo.y, lo.y + size), math::clamp(cam.z, lo.z, lo.z + size)
		);
		const bool far = (c - cam).length_squared() > d2;
		const bool was_off = _shadow_casters_off.count(&block) != 0;
		if (far) {
			off.insert(&block);
			if (!was_off) {
				block.set_shadow_casting(RenderingServerEnums::SHADOW_CASTING_SETTING_OFF);
			}
		} else if (was_off || _shadow_caster_passes % 25 == 0) {
			block.set_shadow_casting(on);
		}
	});
	_shadow_casters_off.swap(off);
}
''')
patch(C, '''	ClassDB::bind_method(D_METHOD("set_edit_remesh_rate", "rate"), &Self::set_edit_remesh_rate);''', '''	ClassDB::bind_method(D_METHOD("set_edit_remesh_rate", "rate"), &Self::set_edit_remesh_rate);
	ClassDB::bind_method(D_METHOD("set_shadow_caster_distance", "voxels"), &Self::set_shadow_caster_distance);
	ClassDB::bind_method(D_METHOD("get_shadow_caster_distance"), &Self::get_shadow_caster_distance);''')
patch(C, '''	// Do it after we change mesh block states so materials are updated
''', '''	if (_shadow_caster_distance > 0.f) {
		update_shadow_casters(); // TechnoApe ADR 0105
	}

	// Do it after we change mesh block states so materials are updated
''')
