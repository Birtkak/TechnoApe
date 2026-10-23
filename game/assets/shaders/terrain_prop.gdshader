shader_type spatial;
render_mode cull_back, depth_draw_opaque;

// Props drawn from the atlas (world.prop_material): item cubes, the held block, and the stations
// and pots that block entities draw over their footprint (ADR 0034). Terrain shows one flat colour
// per 12.5 cm voxel (voxel_sample); props show their tile in prop_cells x prop_cells flat cells
// (6.25 cm on a 0.5 m block, the grain of the micro models), each the average of its texels, so
// they read as small voxel models rather than 16x16 pixel art (ADR 0036). Animated tiles scroll.

#include "res://assets/shaders/terrain_common.gdshaderinc"

uniform float prop_cells = 8.0;

void vertex() {
	vcol = COLOR;
	v_world = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	v_normal_world = normalize((MODEL_MATRIX * vec4(NORMAL, 0.0)).xyz);
}

void fragment() {
	vec4 c;
	vec2 scaled = UV * atlas_tiles;
	vec2 tile = floor(scaled);
	float idx = tile.y * atlas_tiles.x + tile.x;
	if (abs(idx - water_tile) < 0.5 || abs(idx - lava_tile) < 0.5 || abs(idx - fire_tile) < 0.5) {
		idx = terrain_sample(UV, v_world, v_normal_world, CAMERA_POSITION_WORLD, c);
	} else {
		vec2 cell = (floor(fract(scaled) * prop_cells) + 0.5) / prop_cells;
		float tile_px = float(textureSize(atlas, 0).x) / atlas_tiles.x;
		c = textureLod(atlas, (tile + cell) / atlas_tiles, log2(tile_px / prop_cells));
	}
	ALBEDO = c.rgb * vcol.rgb * face_tint(v_normal_world);
	ROUGHNESS = 1.0;
	METALLIC = 0.0;
	SPECULAR = 0.0;
	EMISSION = tile_emission(idx, c.rgb);
}
