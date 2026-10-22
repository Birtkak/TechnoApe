// Shared body of the terrain materials (terrain.gdshader = opaque voxels,
// micro models, item cubes, viewmodel; terrain_liquid.gdshader = the
// see-through liquids/gases). Nearest-filtered atlas, vertex colour as baked
// AO, animated tiles (water, lava, fire scroll inside their tile; UVs wrap
// within the tile so the atlas never bleeds), emissive glow/lava/fire tiles
// that feed the bloom pass, a Minecraft-style per-face tint so blocks stay
// readable in shadow, and a sun glint with a gentle ripple normal on water.
// Godot shader functions cannot touch built-ins (UV, ALBEDO, ...), so these
// helpers take and return plain values; each material's fragment() wires them.

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
uniform float lava_glow = 2.4;      // > 1 so lava and fire bloom
uniform float fire_glow = 2.0;
uniform float face_tint_side = 0.86; // x/z faces vs the top (bottom darker still)
uniform float face_tint_bottom = 0.62;
uniform float water_specular = 0.55;
uniform float water_roughness = 0.14;
uniform float ripple_strength = 0.35;

varying vec4 vcol;
varying vec3 v_world;
varying vec3 v_normal_world;

// Tile index under this UV and the (animated) sample from the atlas.
float terrain_sample(vec2 uv_in, out vec4 c) {
	vec2 scaled = uv_in * atlas_tiles;
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
	c = textureGrad(atlas, uv, dFdx(uv_in), dFdy(uv_in));
	return idx;
}

float face_tint(vec3 n) {
	float up = clamp(n.y, 0.0, 1.0);
	float down = clamp(-n.y, 0.0, 1.0);
	float side = 1.0 - up - down;
	return up + side * face_tint_side + down * face_tint_bottom;
}

vec3 tile_emission(float idx, vec3 c) {
	if (abs(idx - glow_tile) < 0.5) {
		return c * glow_strength;
	} else if (abs(idx - lava_tile) < 0.5) {
		return c * lava_glow;
	} else if (abs(idx - fire_tile) < 0.5) {
		return c * fire_glow;
	}
	return vec3(0.0);
}

// Ripple normal (world space) for a horizontal water surface at world xz p:
// two travelling sine fields, cheap, tileable, no texture.
vec3 ripple_normal(vec2 p) {
	float t = TIME * 1.1;
	float dx = sin(p.x * 2.3 + t * 1.3) * 0.5 + sin((p.x + p.y) * 1.1 - t * 0.9) * 0.5;
	float dz = cos(p.y * 2.1 - t * 1.1) * 0.5 + cos((p.x - p.y) * 1.3 + t * 0.7) * 0.5;
	return normalize(vec3(dx * ripple_strength * 0.15, 1.0, dz * ripple_strength * 0.15));
}
