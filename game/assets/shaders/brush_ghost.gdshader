shader_type spatial;
render_mode unshaded, cull_back, depth_draw_never, blend_mix;

// The build ghost (scripts/player/brush_tool.gd): the brush's shell in the held material's own voxel
// colours (voxel_sample, the same pattern the placed blocks get), washed blue and see-through.

#include "res://assets/shaders/terrain_common.gdshaderinc"

uniform vec3 tiles = vec3(0.0);   // the material's atlas tiles: top, side, bottom
uniform vec4 tint : source_color = vec4(0.25, 0.55, 1.0, 1.0);
uniform float tint_mix = 0.55;
uniform float opacity = 0.6;
uniform float rim = 0.0;   // a brighter, denser edge where the surface turns away (the shape ghost)

void vertex() {
	v_world = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	v_normal_world = normalize((MODEL_MATRIX * vec4(NORMAL, 0.0)).xyz);
}

void fragment() {
	float t = v_normal_world.y > 0.5 ? tiles.x : (v_normal_world.y < -0.5 ? tiles.z : tiles.y);
	vec2 uv = (vec2(mod(t, atlas_tiles.x), floor(t / atlas_tiles.x)) + 0.5) / atlas_tiles;
	vec4 c;
	voxel_sample(uv, v_world, v_normal_world, CAMERA_POSITION_WORLD, c);
	float edge = pow(1.0 - clamp(dot(v_normal_world, normalize(CAMERA_POSITION_WORLD - v_world)), 0.0, 1.0), 3.0);
	ALBEDO = mix(mix(c.rgb, tint.rgb, tint_mix) * face_tint(v_normal_world), tint.rgb * 1.4, rim * edge);
	ALPHA = clamp(opacity + rim * edge, 0.0, 1.0);
}
