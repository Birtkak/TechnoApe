# Patches for the custom godot_voxel build (P11 spike). Run with the godot_voxel root as argv[1].
import sys
root = sys.argv[1]

p = root + '/constants/voxel_constants.h'
s = open(p, encoding='utf-8').read()
old = 'static const float MAXIMUM_LOD_DISTANCE = 128.f;'
new = 'static const float MAXIMUM_LOD_DISTANCE = 1024.f; // TechnoApe P11: was 128 (64 m at 0.5 m voxels)'
assert old in s; s = s.replace(old, new, 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)

p = root + '/terrain/variable_lod/voxel_lod_terrain.h'
s = open(p, encoding='utf-8').read()
old = '\tbool _material_uses_lod_info = false;'
assert s.count(old) == 1, s.count(old)
s = s.replace(old, old + '\n\t// TechnoApe P11: transition-mask changes only need a fading copy when the shader stitches (transvoxel)\n\tbool _material_uses_transition_mask = false;', 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)

p = root + '/terrain/variable_lod/voxel_lod_terrain.cpp'
s = open(p, encoding='utf-8').read()
old = '\t\t_material_uses_lod_info = contains(uniforms, VoxelStringNames::get_singleton().u_voxel_lod_info);'
assert s.count(old) == 1, s.count(old)
s = s.replace(old, old + '\n\t\t_material_uses_transition_mask = contains(uniforms, VoxelStringNames::get_singleton().u_transition_mask);', 1)
old = 'if (_lod_fade_duration > 0.f && shader_material.is_valid() &&'
assert s.count(old) == 1, s.count(old)
s = s.replace(old, 'if (_lod_fade_duration > 0.f && _material_uses_transition_mask && shader_material.is_valid() &&', 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)
print('patched')

# --- fade through a per-instance shader parameter when the terrain has no ShaderMaterial ---
# VoxelLodTerrain only fades blocks that carry a per-block ShaderMaterial copy of
# `material`, and that copy overrides every surface of the block mesh, so a blocky
# library with several materials (terrain + liquids) loses all but one shader.
# With no terrain material the block keeps its surface materials and the fade value
# goes to RenderingServer.instance_geometry_set_shader_parameter (`instance uniform`).
p = root + '/terrain/variable_lod/voxel_mesh_block_vlt.h'
s = open(p, encoding='utf-8').read()
old = '\tvoid set_shader_material(Ref<ShaderMaterial> material);'
assert s.count(old) == 1
s = s.replace(old, old + '\n\t// TechnoApe P11: u_lod_fade on the block material, or as an instance parameter without one\n\tvoid set_lod_fade_param(Vector2 p);', 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)

p = root + '/terrain/variable_lod/voxel_mesh_block_vlt.cpp'
s = open(p, encoding='utf-8').read()
old = '''	if (_shader_material.is_valid()) {
		_shader_material->set_shader_parameter(VoxelStringNames::get_singleton().u_lod_fade, p);'''
assert s.count(old) == 1, s.count(old)
s = s.replace(old, '''	if (_shader_material.is_valid()) {
		_shader_material->set_shader_parameter(VoxelStringNames::get_singleton().u_lod_fade, p);
	} else if (_mesh_instance.is_valid()) {
		_mesh_instance.set_shader_instance_parameter(VoxelStringNames::get_singleton().u_lod_fade, p);''', 1)
old = 'void VoxelMeshBlockVLT::set_shader_material(Ref<ShaderMaterial> material) {'
assert s.count(old) == 1
s = s.replace(old, '''void VoxelMeshBlockVLT::set_lod_fade_param(Vector2 p) {
	if (_shader_material.is_valid()) {
		_shader_material->set_shader_parameter(VoxelStringNames::get_singleton().u_lod_fade, p);
	} else if (_mesh_instance.is_valid()) {
		_mesh_instance.set_shader_instance_parameter(VoxelStringNames::get_singleton().u_lod_fade, p);
	}
}

''' + old, 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)

p = root + '/terrain/variable_lod/voxel_lod_terrain.cpp'
s = open(p, encoding='utf-8').read()
old = '''			Ref<ShaderMaterial> mat = block.get_shader_material();
			if (mat.is_valid()) {
				mat->set_shader_parameter(VoxelStringNames::get_singleton().u_lod_fade, Vector2(0.0, 0.0));
			}'''
assert s.count(old) == 2, s.count(old)
s = s.replace(old, '\t\t\tblock.set_lod_fade_param(Vector2(0.0, 0.0));')
open(p, 'w', encoding='utf-8', newline='\n').write(s)
print('patched fade-instance')
