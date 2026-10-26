shader_type spatial;
render_mode cull_back, depth_draw_opaque, world_vertex_coords;

// The far layer's water (ADR 0074): one flat, opaque sheet per wet cell at its level. COLOR.rgb is the
// bed's colour, COLOR.a the depth (metres / 8): shallow water shows its bed through a tint, deep water
// is the water's own colour, as the near fluids look from a distance.

uniform float near_cut = 0.0;
uniform vec3 water_color : source_color = vec3(0.22, 0.45, 0.58);
uniform vec3 deep_color : source_color = vec3(0.05, 0.17, 0.30);
uniform float water_specular = 0.5;
uniform float water_roughness = 0.18;

varying vec3 v_world;
varying vec4 v_col;

void vertex() {
	v_world = VERTEX;
	v_col = COLOR;
}

void fragment() {
	if (distance(v_world, CAMERA_POSITION_WORLD) < near_cut) {
		discard;
	}
	float depth = clamp(v_col.a, 0.0, 1.0);
	vec3 wc = mix(water_color, deep_color, depth);
	ALBEDO = mix(v_col.rgb * 0.8, wc, 0.45 + 0.55 * depth);
	ROUGHNESS = water_roughness;
	METALLIC = 0.0;
	SPECULAR = water_specular;
}
