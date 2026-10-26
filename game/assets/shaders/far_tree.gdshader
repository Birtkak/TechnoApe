shader_type spatial;
render_mode cull_back, depth_draw_opaque, world_vertex_coords;

// Far trees (ADR 0074): crown blobs and trunks as MultiMesh instances, coloured per instance with the
// leaves' tile average. On the last level that draws instances they shrink into the ground over the
// ring's morph band, where the next level bakes the canopy into the skin instead.

uniform float morph_start = 1e9;
uniform float morph_end = 2e9;
uniform float near_cut = 0.0;
uniform float face_tint_side = 0.86;

varying vec3 v_world;
varying vec3 v_col;
varying vec3 v_nrm;

void vertex() {
	vec3 origin = MODEL_MATRIX[3].xyz;
	float d = distance(origin, CAMERA_POSITION_WORLD);
	float m = smoothstep(morph_start, morph_end, d);
	VERTEX = origin + (VERTEX - origin) * (1.0 - m);
	v_world = VERTEX;
	v_nrm = NORMAL;
	v_col = COLOR.rgb;
}

void fragment() {
	if (distance(v_world, CAMERA_POSITION_WORLD) < near_cut) {
		discard;
	}
	vec3 n = normalize(v_nrm);
	float up = clamp(n.y, 0.0, 1.0);
	ALBEDO = v_col * (up + (1.0 - up) * face_tint_side);
	ROUGHNESS = 1.0;
	METALLIC = 0.0;
	SPECULAR = 0.0;
}
