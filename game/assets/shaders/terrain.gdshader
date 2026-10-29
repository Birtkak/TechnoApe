shader_type spatial;
render_mode cull_back, depth_draw_opaque;

// Terrain atlas material (world.gd) for every opaque voxel and the micro
// models; world.prop_material, a copy with stains off, draws item cubes and
// the viewmodel. Helpers and uniforms live in
// terrain_common.gdshaderinc, shared with terrain_liquid.gdshader.

#include "res://assets/shaders/terrain_common.gdshaderinc"

// Corner occlusion of merged faces (ADR 0107): the mesher merges every cube face (UV2.x = 1 marks them) and this
// shader shades them as godot_voxel baked it (0fps corner occlusion: two side neighbours = 3 levels, else sides +
// corner), interpolated across the face. Each mesh block has a layer of ao_pages: one bit per voxel of the block and
// one around it (texel (x, z * ao_col_bytes + y / 8), bit y % 8). Other faces carry their occlusion in COLOR.
uniform bool ao_pages_on = false;
uniform sampler2DArray ao_pages : filter_nearest, repeat_disable;     // layers 0 .. ao_layers_per_page - 1
uniform sampler2DArray ao_pages_b : filter_nearest, repeat_disable;   // the layers after those (GPUs allow 2048 per array)
uniform int ao_layers_per_page = 2048;
uniform int ao_col_bytes = 9;
uniform float ao_darkness = 0.2667;
instance uniform int ao_layer = 0;
varying flat int v_ao_layer;
varying flat vec3 v_block_origin;

float ao_occ(ivec3 p) {
	p += ivec3(1);   // the volume starts one voxel before the block
	ivec3 t = ivec3(p.x, p.z * ao_col_bytes + (p.y >> 3), v_ao_layer);
	float b = v_ao_layer < ao_layers_per_page ? texelFetch(ao_pages, t, 0).r
			: texelFetch(ao_pages_b, ivec3(t.xy, v_ao_layer - ao_layers_per_page), 0).r;
	return float((int(b * 255.0 + 0.5) >> (p.y & 7)) & 1);
}

float ao_corner(float s1, float s2, float c) {
	return s1 * s2 > 0.5 ? 3.0 : s1 + s2 + c;
}

// Light left at this point of a face with normal n (world units = voxels).
float voxel_ao(vec3 wpos, vec3 n) {
	vec3 lp = wpos - v_block_origin;
	ivec3 nn = ivec3(round(n));
	ivec3 a = ivec3(floor(lp - n * 0.02)) + nn;   // the air voxel the face looks into
	vec3 an = abs(n);
	ivec3 du = an.x > 0.5 ? ivec3(0, 1, 0) : ivec3(1, 0, 0);
	ivec3 dv = an.z > 0.5 ? ivec3(0, 1, 0) : ivec3(0, 0, 1);
	float fu = fract(dot(lp, vec3(du)));
	float fv = fract(dot(lp, vec3(dv)));
	float su0 = ao_occ(a - du);
	float su1 = ao_occ(a + du);
	float sv0 = ao_occ(a - dv);
	float sv1 = ao_occ(a + dv);
	float o00 = ao_corner(su0, sv0, ao_occ(a - du - dv));
	float o10 = ao_corner(su1, sv0, ao_occ(a + du - dv));
	float o01 = ao_corner(su0, sv1, ao_occ(a - du + dv));
	float o11 = ao_corner(su1, sv1, ao_occ(a + du + dv));
	return 1.0 - ao_darkness * mix(mix(o00, o10, fu), mix(o01, o11, fu), fv);
}

void vertex() {
	vcol = COLOR;
	v_world = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	v_normal_world = normalize((MODEL_MATRIX * vec4(NORMAL, 0.0)).xyz);
	v_ao_layer = ao_layer;
	v_block_origin = MODEL_MATRIX[3].xyz;
}

void fragment() {
	float hz = horizon_mix(v_world, CAMERA_POSITION_WORLD);
	if (hz >= 1.0) {
		discard;
	}
	vec4 c;
	float idx = voxel_sample(UV, v_world, v_normal_world, CAMERA_POSITION_WORLD, c);
	c.rgb = season_color(c.rgb, idx, v_world, v_normal_world);
	vec3 shade = vcol.rgb;
	if (ao_pages_on && UV2.x > 0.5 && v_ao_layer > 0) {
		shade *= voxel_ao(v_world, v_normal_world);
	}
	ALBEDO = c.rgb * shade * face_tint(v_normal_world);
	ROUGHNESS = 1.0;
	METALLIC = 0.0;
	SPECULAR = 0.0;
	EMISSION = tile_emission(idx, c.rgb);
	if (stains_on) {
		vec4 st = surface_stain(v_world, v_normal_world);
		// keep a little of the texture's light/dark pattern under the stain
		float detail = 0.7 + 0.6 * dot(c.rgb, vec3(0.333));
		ALBEDO = mix(ALBEDO, st.rgb * detail * shade * face_tint(v_normal_world), st.a);
		EMISSION *= 1.0 - st.a;
	}
	if (hz > 0.0) {
		// the horizon: lit colour out, the sky behind in (ADR 0073)
		ALBEDO *= 1.0 - hz;
		EMISSION = mix(EMISSION, horizon_sky(v_world, CAMERA_POSITION_WORLD, SCREEN_UV), hz);
	}
}
