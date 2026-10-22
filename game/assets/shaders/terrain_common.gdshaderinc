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

// --- surface stains (scripts/visuals/stain_map.gd) ----------------------------
// Noita-style coatings: blood, oil, soot. A stain belongs to an EMPTY cell (where
// the liquid or flame was) and colours the faces that border it: floors fully,
// walls up to a tide line that rises with the amount, ceilings faintly. The
// window around the player lives in a texture array, layer = world y, texel =
// world (x, z), all wrapped (mod size); rgb = colour * amount, a = amount.
// Per stain pixel (16 per voxel edge, the atlas resolution) the amount is
// interpolated across neighbouring cells and thresholded against noise, so
// stains are pixel splats with ragged edges, not tinted cubes.
uniform bool stains_on = false;
uniform sampler2DArray stain_tex : filter_nearest;
uniform ivec3 stain_size = ivec3(128, 64, 128);
uniform ivec3 stain_min = ivec3(0);    // valid window in world voxels (inclusive)
uniform ivec3 stain_max = ivec3(0);    // exclusive
uniform float stain_px = 16.0;

float stain_hash(vec3 p) {
	p = fract(p * 0.1031);
	p += dot(p, p.zyx + 31.32);
	return fract((p.x + p.y) * p.z);
}

vec4 stain_cell(ivec3 c) {
	if (any(lessThan(c, stain_min)) || any(greaterThanEqual(c, stain_max))) {
		return vec4(0.0);
	}
	ivec3 m = ((c % stain_size) + stain_size) % stain_size;
	return texelFetch(stain_tex, ivec3(m.x, m.z, m.y), 0);
}

// rgb = stain colour, a = how much of it covers this pixel (0 = clean).
vec4 surface_stain(vec3 wpos, vec3 n) {
	vec3 an = abs(n);
	if (max(an.x, max(an.y, an.z)) < 0.9) {
		return vec4(0.0);   // only axis-aligned faces
	}
	int ax = an.x > 0.5 ? 0 : (an.y > 0.5 ? 1 : 2);
	vec3 pq = (floor(wpos * stain_px) + 0.5) / stain_px;   // stain pixel centre
	vec3 out_p = pq + sign(n) * an * 0.5;                   // inside the cell the face looks into
	vec3 e1 = ax == 0 ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0);
	vec3 e2 = ax == 2 ? vec3(0.0, 1.0, 0.0) : vec3(0.0, 0.0, 1.0);
	vec3 b = out_p - 0.5 * (e1 + e2);
	vec3 base = floor(b);
	vec3 f3 = b - base;
	vec3 cell = floor(out_p);
	base = mix(base, cell, step(0.5, an));   // along the normal: the outside cell itself
	float fu = dot(f3, e1);
	float fv = dot(f3, e2);
	if (ax != 1) {
		base.y = cell.y;      // walls: no blending between cells above / below
		if (ax == 0) { fu = 0.0; } else { fv = 0.0; }
	}
	ivec3 c00 = ivec3(base);
	ivec3 d1 = ivec3(e1);
	ivec3 d2 = ivec3(e2);
	vec4 s = mix(mix(stain_cell(c00), stain_cell(c00 + d1), fu),
		mix(stain_cell(c00 + d2), stain_cell(c00 + d1 + d2), fu), fv);
	float a = s.a;
	if (a < 0.02) {
		return vec4(0.0);
	}
	vec3 px = floor(out_p * stain_px);
	float h = stain_hash(px);
	float blot = stain_hash(floor(out_p * 4.0) + 17.0);
	float thr = 0.1 + 0.8 * (0.55 * h + 0.45 * blot);
	if (ax != 1) {
		// walls: liquid sits low in the cell; the tide line wobbles per pixel column
		float local_h = pq.y - cell.y;
		float col_j = stain_hash(vec3(px.x + px.z, 3.0, px.x - px.z));
		a *= step(local_h, 0.12 + 0.8 * a + 0.12 * (col_j - 0.5)) * 0.85;   // < 1: walls stay speckled
	} else if (n.y < 0.0) {
		a *= 0.35;            // ceilings: splashes and soot only
	}
	float cover = step(thr, a);
	return vec4(s.rgb / max(s.a, 1e-4), cover * (0.78 + 0.2 * min(a, 1.0)));
}
