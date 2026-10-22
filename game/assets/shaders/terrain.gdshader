shader_type spatial;
render_mode cull_back, depth_draw_opaque, specular_disabled;

// Terrain atlas material (world.gd). Replaces the StandardMaterial3D: nearest
// filtering, vertex colour as baked AO, and animated tiles: fragments whose UV
// falls in the water, lava or fire tile scroll inside that tile over time (UVs
// wrap within the tile so the atlas never bleeds into a neighbour). Rivers,
// lakes, waterfall sheets and lava pools therefore move; everything else is
// sampled exactly as before.

uniform sampler2D atlas : source_color, filter_nearest_mipmap, repeat_disable;
uniform vec2 atlas_tiles = vec2(8.0, 6.0);
uniform float water_tile = 5.0;
uniform float lava_tile = 7.0;
uniform float fire_tile = 12.0;
uniform float water_speed = 0.45;   // tiles per second along the flow direction
uniform float lava_speed = 0.06;
uniform float fire_speed = 1.6;
uniform float glow_tile = 45.0;     // emissive tile (glow mushroom caps)
uniform float glow_strength = 2.2;

varying vec4 vcol;

void vertex() {
	vcol = COLOR;
}

void fragment() {
	vec2 scaled = UV * atlas_tiles;
	vec2 tile = floor(scaled);
	vec2 local = scaled - tile;
	float idx = tile.y * atlas_tiles.x + tile.x;
	vec2 shift = vec2(0.0);
	if (abs(idx - water_tile) < 0.5) {
		shift = vec2(0.37, 1.0) * TIME * water_speed;
	} else if (abs(idx - lava_tile) < 0.5) {
		shift = vec2(1.0, 0.23) * TIME * lava_speed;
	} else if (abs(idx - fire_tile) < 0.5) {
		shift = vec2(0.0, -1.0) * TIME * fire_speed;
	}
	local = fract(local + shift);
	vec2 uv = (tile + local) / atlas_tiles;
	// derivatives from the unshifted UV: no mip seams where the wrap happens
	vec4 c = textureGrad(atlas, uv, dFdx(UV), dFdy(UV));
	ALBEDO = c.rgb * vcol.rgb;
	ROUGHNESS = 1.0;
	METALLIC = 0.0;
	if (abs(idx - glow_tile) < 0.5) {
		EMISSION = c.rgb * glow_strength;
	} else if (abs(idx - lava_tile) < 0.5 || abs(idx - fire_tile) < 0.5) {
		EMISSION = c.rgb * 0.9;
	}
}
