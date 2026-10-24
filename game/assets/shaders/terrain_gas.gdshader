shader_type spatial;
render_mode cull_back, depth_draw_never, specular_disabled;

// Gases (steam, smoke): a soft, thin cloud instead of see-through cubes (bug report 2026-10-25:
// "gas looks ugly"). Faces between two gas voxels are culled by the mesher (shared transparency
// index), so only the cloud's shell draws; its opacity breathes with a rising 3D noise, thins at
// grazing angles and fades where it meets the ground (depth behind), so the voxel edges blur.

#include "res://assets/shaders/terrain_common.gdshaderinc"

uniform sampler2D depth_tex : hint_depth_texture, filter_nearest;
uniform float gas_alpha : hint_range(0.0, 1.0) = 0.5;
uniform float rise_speed = 0.8;        // voxels per second the noise climbs
uniform float soft_voxels = 1.5;       // fade over this much depth to whatever is behind

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
	// the tile's own colour, not its pixels: a cloud has no texture
	vec2 tile = vec2(mod(idx, atlas_tiles.x), floor(idx / atlas_tiles.x));
	vec3 base = textureLod(atlas, (tile + 0.5) / atlas_tiles, 8.0).rgb;
	vec3 q = v_world * 0.32 + vec3(0.0, -TIME * rise_speed * 0.32, TIME * 0.05);
	float puff = lava_noise(q) * 0.65 + lava_noise(q * 2.3 + 7.0) * 0.35;
	float zb = texture(depth_tex, SCREEN_UV).r;
	float behind = 1e3;
	if (zb > 0.0) {
		vec4 vb = INV_PROJECTION_MATRIX * vec4(SCREEN_UV * 2.0 - 1.0, zb, 1.0);
		behind = max(length(vb.xyz / vb.w) - length(VERTEX), 0.0);
	}
	float facing = abs(dot(normalize(VIEW), NORMAL));
	ALBEDO = base * mix(0.85, 1.1, puff);
	ROUGHNESS = 1.0;
	ALPHA = gas_alpha * smoothstep(0.2, 0.75, puff) * (0.55 + 0.45 * facing) * clamp(behind / soft_voxels, 0.0, 1.0);
}
