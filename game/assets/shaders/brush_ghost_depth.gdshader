shader_type spatial;
render_mode unshaded, cull_back, depth_draw_always, blend_mix;

// The first pass of the build ghost: writes only depth, so brush_ghost (its next_pass) colours just the
// surface nearest the eye. Without it a faint ghost shows every shell behind the front one as noise.

void fragment() {
	ALBEDO = vec3(0.0);
	ALPHA = 0.0;
}
