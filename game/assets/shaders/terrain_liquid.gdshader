shader_type spatial;
render_mode cull_back, depth_draw_always;

// See-through liquids and gases (water, oil, steam, smoke, fire): the same
// atlas sampling as terrain.gdshader plus alpha, a sun glint on water tops
// and depth writes, so the nearest water surface wins and nothing behind it
// sorts wrongly. Faces between two liquid voxels are culled by the blocky
// mesher (shared transparency index), only the shell is drawn.

#include "res://assets/shaders/terrain_common.gdshaderinc"

uniform float liquid_alpha : hint_range(0.0, 1.0) = 0.68;
uniform float edge_alpha_boost = 0.25;   // grazing angles look more opaque, like a real surface

void vertex() {
	vcol = COLOR;
	v_world = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	v_normal_world = normalize((MODEL_MATRIX * vec4(NORMAL, 0.0)).xyz);
}

void fragment() {
	if (lod_fade_discard(FRAGCOORD.xy)) {
		discard;
	}
	vec4 c;
	float idx = terrain_sample(UV, v_world, v_normal_world, CAMERA_POSITION_WORLD, c);
	ALBEDO = c.rgb * vcol.rgb * face_tint(v_normal_world);
	ROUGHNESS = 1.0;
	METALLIC = 0.0;
	SPECULAR = 0.0;
	EMISSION = tile_emission(idx, c.rgb);
	if (abs(idx - water_tile) < 0.5 && v_normal_world.y > 0.9) {
		NORMAL = normalize((VIEW_MATRIX * vec4(ripple_normal(v_world.xz), 0.0)).xyz);
		SPECULAR = water_specular;
		ROUGHNESS = water_roughness;
	}
	float facing = abs(dot(normalize(VIEW), NORMAL));
	ALPHA = clamp(liquid_alpha + (1.0 - facing) * edge_alpha_boost, 0.0, 1.0);
}
