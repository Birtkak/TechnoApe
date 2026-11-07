shader_type spatial;
render_mode cull_back, depth_draw_never, specular_disabled;

// Gases (steam, smoke): a soft, thin cloud instead of see-through cubes (bug report 2026-10-25:
// "gas looks ugly"). Faces between two gas voxels are culled by the mesher (shared transparency
// index), so only the cloud's shell draws; its opacity breathes with a rising 3D noise, thins at
// grazing angles and fades where it meets the ground (depth behind), so the voxel edges blur.

#include "res://assets/shaders/terrain_common.gdshaderinc"

uniform sampler2D depth_tex : hint_depth_texture, filter_nearest;
uniform float gas_alpha : hint_range(0.0, 1.0) = 0.5;
uniform vec3 gas_tint = vec3(1.0);     // per gas (data/gas_looks.json): multiplies the tile's colour
uniform float rise_speed = 0.8;        // voxels per second the noise climbs
uniform float soft_voxels = 1.5;       // fade over this much depth to whatever is behind
uniform float flame = 0.0;             // above 0: it glows this strongly in fire's colours instead of taking the light

void vertex() {
	vcol = COLOR;
	v_world = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	v_normal_world = normalize((MODEL_MATRIX * vec4(NORMAL, 0.0)).xyz);
}

void fragment() {
	float hz = horizon_mix(v_world, CAMERA_POSITION_WORLD);
	if (hz >= 1.0) {
		discard;
	}
	vec4 c;
	float idx = terrain_sample(UV, v_world, v_normal_world, CAMERA_POSITION_WORLD, c);
	// the tile's own colour, not its pixels: a cloud has no texture
	vec2 tile = vec2(mod(idx, atlas_tiles.x), floor(idx / atlas_tiles.x));
	// the mip where one texel is one tile (16 px -> 4); a deeper one averages the whole atlas (steam drew brown)
	float tile_mip = log2(float(textureSize(atlas, 0).x) / atlas_tiles.x);
	vec3 base = textureLod(atlas, (tile + 0.5) / atlas_tiles, tile_mip).rgb;
	// about a voxel across: one cell's faces get holes, so a lone cell reads as a wisp and not as a box
	vec3 q = v_world * 0.85 + vec3(0.0, -TIME * rise_speed * 0.85, TIME * 0.05);
	float puff = lava_noise(q) * 0.65 + lava_noise(q * 2.3 + 7.0) * 0.35;
	float zb = texture(depth_tex, SCREEN_UV).r;
	float behind = 1e3;
	if (zb > 0.0) {
		vec4 vb = INV_PROJECTION_MATRIX * vec4(SCREEN_UV * 2.0 - 1.0, zb, 1.0);
		behind = max(length(vb.xyz / vb.w) - length(VERTEX), 0.0);
	}
	float facing = abs(dot(normalize(VIEW), NORMAL));
	if (flame > 0.0) {
		vec3 hot = mix(vec3(0.75, 0.08, 0.01), vec3(1.0, 0.42, 0.04), smoothstep(0.3, 0.5, puff));
		ALBEDO = vec3(0.0);
		EMISSION = mix(hot, vec3(1.0, 0.82, 0.3), smoothstep(0.5, 0.72, puff)) * flame;
	} else {
		ALBEDO = base * gas_tint * mix(0.85, 1.1, puff);
	}
	ROUGHNESS = 1.0;
	ALPHA = gas_alpha * smoothstep(0.25, 0.6, puff) * (0.55 + 0.45 * facing) * clamp(behind / soft_voxels, 0.0, 1.0) * (1.0 - hz);
}
