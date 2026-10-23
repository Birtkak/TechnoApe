shader_type spatial;
render_mode cull_back, depth_draw_opaque;

// Props drawn with the atlas as pixel art (world.prop_material): item cubes,
// the held block, and the stations and pots that block entities draw over
// their footprint (ADR 0034). Unlike the terrain (one flat colour per 12.5 cm
// voxel, voxel_sample), these show their 16x16 tiles across the face.

#include "res://assets/shaders/terrain_common.gdshaderinc"

void vertex() {
	vcol = COLOR;
	v_world = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	v_normal_world = normalize((MODEL_MATRIX * vec4(NORMAL, 0.0)).xyz);
}

void fragment() {
	vec4 c;
	float idx = terrain_sample(UV, v_world, v_normal_world, CAMERA_POSITION_WORLD, c);
	ALBEDO = c.rgb * vcol.rgb * face_tint(v_normal_world);
	ROUGHNESS = 1.0;
	METALLIC = 0.0;
	SPECULAR = 0.0;
	EMISSION = tile_emission(idx, c.rgb);
}
