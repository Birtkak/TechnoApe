shader_type spatial;
render_mode unshaded, blend_mix, cull_back, depth_draw_never, shadows_disabled;

// Minecraft-style breaking cracks over the voxel being mined (Player). The
// 16x16 crack texture holds, per pixel, the break progress at which that
// pixel cracks (1 = never), so the cracks grow from the middle outwards.
// Each face shows the texture projected along its normal.

uniform sampler2D crack_map : filter_nearest, repeat_disable;
uniform float progress = 0.0;
varying vec3 local_pos;

void vertex() {
	local_pos = VERTEX;
}

void fragment() {
	vec3 n = abs((INV_VIEW_MATRIX * vec4(NORMAL, 0.0)).xyz);
	vec3 p = local_pos + 0.5;
	vec2 uv = n.x > 0.5 ? p.zy : (n.y > 0.5 ? p.xz : p.xy);
	float at = texture(crack_map, clamp(uv, 0.0, 0.999)).r;
	if (at > progress) {
		discard;
	}
	ALBEDO = vec3(0.06, 0.05, 0.04);
	ALPHA = 0.75;
}
