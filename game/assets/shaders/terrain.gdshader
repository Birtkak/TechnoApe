shader_type spatial;
render_mode cull_back, depth_draw_opaque;

// Terrain atlas material (world.gd) for every opaque voxel, the micro
// models, item cubes and the viewmodel. Helpers and uniforms live in
// terrain_common.gdshaderinc, shared with terrain_liquid.gdshader.

#include "res://assets/shaders/terrain_common.gdshaderinc"

void vertex() {
	vcol = COLOR;
	v_world = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	v_normal_world = normalize((MODEL_MATRIX * vec4(NORMAL, 0.0)).xyz);
}

void fragment() {
	vec4 c;
	float idx = terrain_sample(UV, c);
	ALBEDO = c.rgb * vcol.rgb * face_tint(v_normal_world);
	ROUGHNESS = 1.0;
	METALLIC = 0.0;
	SPECULAR = 0.0;
	EMISSION = tile_emission(idx, c.rgb);
}
