shader_type spatial;
render_mode unshaded, blend_mix, cull_disabled, depth_draw_never, shadows_disabled;

// ADR 0142: a creaking roof (MineSupport) darkens and cracks open as its fall nears. One quad per 0.5 m block
// under the roof, UV 0..1 across it; crack_map is Player.crack_image() (per pixel the progress at which it
// cracks), so the cracks grow from each block's middle like mining cracks.

uniform sampler2D crack_map : filter_nearest, repeat_disable;
uniform float progress = 0.0;

void fragment() {
	float crack = texture(crack_map, clamp(UV, 0.0, 0.999)).r <= progress ? 1.0 : 0.0;
	ALBEDO = vec3(0.05, 0.04, 0.03);
	ALPHA = max(crack * 0.85, progress * 0.4);
}
