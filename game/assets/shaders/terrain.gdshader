shader_type spatial;
render_mode cull_back, depth_draw_opaque;

// Terrain atlas material (world.gd) for every opaque voxel and the micro
// models; world.prop_material, a copy with stains off, draws item cubes and
// the viewmodel. Helpers and uniforms live in
// terrain_common.gdshaderinc, shared with terrain_liquid.gdshader.

#include "res://assets/shaders/terrain_common.gdshaderinc"

void vertex() {
	vcol = COLOR;
	v_world = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	v_normal_world = normalize((MODEL_MATRIX * vec4(NORMAL, 0.0)).xyz);
}

void fragment() {
	if (fade_discard(FRAGCOORD.xy, v_world, CAMERA_POSITION_WORLD)) {
		discard;
	}
	vec4 c;
	float idx = voxel_sample(UV, v_world, v_normal_world, CAMERA_POSITION_WORLD, c);
	c.rgb = season_color(c.rgb, idx, v_world, v_normal_world);
	ALBEDO = c.rgb * vcol.rgb * face_tint(v_normal_world);
	ROUGHNESS = 1.0;
	METALLIC = 0.0;
	SPECULAR = 0.0;
	EMISSION = tile_emission(idx, c.rgb);
	if (stains_on) {
		vec4 st = surface_stain(v_world, v_normal_world);
		// keep a little of the texture's light/dark pattern under the stain
		float detail = 0.7 + 0.6 * dot(c.rgb, vec3(0.333));
		ALBEDO = mix(ALBEDO, st.rgb * detail * vcol.rgb * face_tint(v_normal_world), st.a);
		EMISSION *= 1.0 - st.a;
	}
}
