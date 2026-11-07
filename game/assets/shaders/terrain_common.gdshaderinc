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
uniform float glow_tile_b = 104.0;  // a second one (geode crystal, ADR 0052)
uniform float glow_strength = 2.2;
uniform float lava_glow = 2.4;      // > 1 so lava and fire bloom
uniform float fire_glow = 2.0;
uniform float flame_tile = 120.0;   // torch flames (ADR 0053): an 8-colour patterned flame that glows and flickers
uniform float flame_glow = 1.25;
uniform float rune_tile = 174.0;   // the Gelvin monolith (ADR 0098): its magenta texels glow and breathe
uniform float seam_tile = 175.0;
uniform float rune_glow = 2.6;
uniform float face_tint_side = 0.86; // x/z faces vs the top (bottom darker still)
uniform float face_tint_bottom = 0.62;
uniform float water_specular = 0.55;
uniform float water_roughness = 0.14;
uniform float ripple_strength = 0.35;
// far faces (ADR 0017): grass sides seen from afar show the grass top, and from
// far_fade_start (world voxels from the camera) textures fade toward tile averages
uniform float grass_top_tile = 2.0;
uniform float grass_side_tile = 3.0;
// Patterned tiles (ADR 0039, GameMaterials.pattern_tiles): bit t of pattern_tiles[t / 32] set = tile t
// is a pattern laid over the world grid, one texel per voxel (plank rows, brick courses, cobble, bark),
// instead of one texel picked by the voxel's hash.
uniform int pattern_tiles[8];
uniform float far_fade_start = 300.0;
uniform float far_fade_end = 1200.0;
// The horizon (ADR 0073): the voxel world is one box of full-detail voxels. From horizon_start
// (world voxels from the camera) a pixel blends into the colour the sky has behind it and is gone at
// horizon_end, the box's edge, so the world ends in sky, never in a wall of cubes or a dither. 0 = off.
uniform float horizon_start = 0.0;
uniform float horizon_end = 0.0;

#include "res://assets/shaders/sky_common.gdshaderinc"

varying vec4 vcol;
varying vec3 v_world;
varying vec3 v_normal_world;

// Tile index under this UV and the (animated) sample from the atlas.
// Near faces use the mesher's UVs: one tile per face, whether the face is a
// voxel, a micro cell or an item cube. On far LOD blocks a face spans 2^lod
// voxels and those UVs would stretch one tile across all of it (ADR 0017), so
// there the tile repeats per world voxel (constant texel size, a Minecraft
// hillside seen from afar), grass sides show the grass top instead of dirt
// stripes, and past far_fade_start the mip gradients widen so the sample
// blends toward the tile's average colour. Gradients are capped at one tile
// per pixel: coarser mips blend neighbouring tiles together.
float terrain_sample(vec2 uv_in, vec3 wpos, vec3 n, vec3 cam, out vec4 c) {
	vec2 scaled = uv_in * atlas_tiles;
	vec2 tile = floor(scaled);
	vec2 local = scaled - tile;
	// voxels per tile across this pixel: 1 on a voxel face, 2^lod on far LOD
	// blocks, far below 1 on micro cells and item cubes
	float vox_per_tile = (length(dFdx(wpos)) + length(dFdy(wpos))) / max(length(dFdx(scaled)) + length(dFdy(scaled)), 1e-6);
	vec2 gsrc = scaled;   // unwrapped coordinate whose derivatives pick the mip (no seams at the wrap)
	if (vox_per_tile > 1.5) {
		vec3 an = abs(n);
		// the two world axes across the face; -y so the tile's top row stays at the top
		vec2 wc = an.y > 0.5 ? wpos.xz : (an.x > 0.5 ? vec2(wpos.z, -wpos.y) : vec2(wpos.x, -wpos.y));
		local = fract(wc);
		gsrc = wc;
		if (an.y < 0.5 && abs(tile.y * atlas_tiles.x + tile.x - grass_side_tile) < 0.5) {
			tile = vec2(mod(grass_top_tile, atlas_tiles.x), floor(grass_top_tile / atlas_tiles.x));
		}
	}
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
	float widen = 1.0 + 16.0 * smoothstep(far_fade_start, far_fade_end, distance(wpos, cam));
	vec2 dx = dFdx(gsrc) / atlas_tiles * widen;
	vec2 dy = dFdy(gsrc) / atlas_tiles * widen;
	float m = max(max(abs(dx.x), abs(dy.x)) * atlas_tiles.x, max(abs(dx.y), abs(dy.y)) * atlas_tiles.y);
	if (m > 1.0) {
		dx /= m;
		dy /= m;
	}
	c = textureGrad(atlas, uv, dx, dy);
	return idx;
}

// Flat-coloured voxels (ADR 0033): a voxel is one colour, picked from its
// material's tile by a hash of the voxel's world position, so the 16x16 tile
// works as a palette whose pixel counts are the colour weights. The voxel is
// the one just inside the face (faces of far LOD blocks still get per-voxel
// colours). Past far_fade_start the colour blends to the tile's average (its
// 1x1 mip) so distant ground does not sparkle. Animated tiles (water, lava,
// fire) keep the scrolling texture.
uint voxel_hash(ivec3 p) {
	uint h = uint(p.x) * 73856093u ^ uint(p.y) * 19349663u ^ uint(p.z) * 83492791u;
	h ^= h >> 13u;
	h *= 0x5bd1e995u;
	h ^= h >> 15u;
	return h;
}

float voxel_sample(vec2 uv_in, vec3 wpos, vec3 n, vec3 cam, out vec4 c) {
	vec2 scaled = uv_in * atlas_tiles;
	vec2 tile = floor(scaled);
	float idx = tile.y * atlas_tiles.x + tile.x;
	if (abs(idx - water_tile) < 0.5 || abs(idx - lava_tile) < 0.5 || abs(idx - fire_tile) < 0.5) {
		return terrain_sample(uv_in, wpos, n, cam, c);
	}
	ivec2 tex_size = textureSize(atlas, 0);
	ivec2 tile_px = tex_size / ivec2(atlas_tiles);
	ivec3 vox = ivec3(floor(wpos - n * 0.02));
	int ti = int(idx + 0.5);
	ivec2 cell;
	if (ti >= 0 && ti < 256 && (pattern_tiles[ti >> 5] & (1 << (ti & 31))) != 0) {
		// the tile across the face's plane: walls show it upright, floors from above
		vec3 an = abs(n);
		ivec2 q = an.y > 0.5 ? vox.xz : (an.x > an.z ? ivec2(vox.z, -vox.y - 1) : ivec2(vox.x, -vox.y - 1));
		cell = ivec2(((q.x % tile_px.x) + tile_px.x) % tile_px.x, ((q.y % tile_px.y) + tile_px.y) % tile_px.y);
	} else {
		uint h = voxel_hash(vox);
		cell = ivec2(int(h % uint(tile_px.x)), int((h >> 8u) % uint(tile_px.y)));
	}
	ivec2 texel = ivec2(tile) * tile_px + cell;
	c = texelFetch(atlas, texel, 0);
	float far = smoothstep(far_fade_start, far_fade_end, distance(wpos, cam));
	if (far > 0.0) {
		vec4 avg = textureLod(atlas, (tile + 0.5) / atlas_tiles, log2(float(tile_px.x)));
		c = mix(c, avg, far);
	}
	return idx;
}

// How far into the horizon blend this pixel is, 0..1 (1 = past the edge: discard it).
float horizon_mix(vec3 wpos, vec3 cam) {
	if (horizon_end <= 0.0) {
		return 0.0;
	}
	return smoothstep(horizon_start, horizon_end, distance(wpos, cam));
}

// What is behind this pixel: the sky's own colour (sky_common: the gradient sky.gdshader draws).
// What the terrain blends into; fog is applied after, as on the sky itself.
vec3 horizon_sky(vec3 wpos, vec3 cam, vec2 screen_uv) {
	return gradient(normalize(wpos - cam), sky_light()) * exposure;
}

float face_tint(vec3 n) {
	float up = clamp(n.y, 0.0, 1.0);
	float down = clamp(-n.y, 0.0, 1.0);
	float side = 1.0 - up - down;
	return up + side * face_tint_side + down * face_tint_bottom;
}

vec3 tile_emission(float idx, vec3 c) {
	if (abs(idx - glow_tile) < 0.5 || abs(idx - glow_tile_b) < 0.5) {
		return c * glow_strength;
	} else if (abs(idx - lava_tile) < 0.5) {
		return c * lava_glow;
	} else if (abs(idx - fire_tile) < 0.5) {
		return c * fire_glow;
	} else if (abs(idx - flame_tile) < 0.5) {
		// each colour of the flame breathes at its own phase: the flame flickers without a remesh
		return c * flame_glow * (0.8 + 0.2 * sin(TIME * 9.0 + c.g * 23.0) + 0.08 * sin(TIME * 23.0 + c.r * 11.0));
	} else if (idx > 203.5 && idx < 207.5) {
		// cave dressing (glow berries, magma crust, fungus caps): only their bright texels glow
		return c * glow_strength * step(0.7, max(c.r, max(c.g, c.b)));
	} else if (abs(idx - 216.0) < 0.5) {
		// hot meteoric iron (ADR 0157): its molten veins glow white-orange and breathe
		return c * 2.6 * step(0.55, c.r) * (0.85 + 0.15 * sin(TIME * 2.3 + c.g * 17.0));
	} else if (abs(idx - rune_tile) < 0.5 || abs(idx - seam_tile) < 0.5) {
		// only the magenta texels (runes, seam) glow, not the stone around them
		return c * rune_glow * step(0.25, min(c.r, c.b) - c.g) * (0.8 + 0.2 * sin(TIME * 1.3));
	}
	return vec3(0.0);
}

// Lava, one colour per voxel (bug report 2026-10-25: it was a flat glowing yellow): dark crust plates
// a few voxels across that drift and churn, with glowing cracks between them. `cell` is the voxel
// (world, floored). Returns the albedo; `emission` gets the glow, strong only in the cracks so the
// bloom outlines them instead of washing the pool out.
float lava_hash(vec3 p) {
	return fract(sin(dot(p, vec3(127.1, 311.7, 74.7))) * 43758.5453);
}

float lava_noise(vec3 p) {
	vec3 i = floor(p);
	vec3 f = fract(p);
	f = f * f * (3.0 - 2.0 * f);
	float a = mix(mix(lava_hash(i), lava_hash(i + vec3(1, 0, 0)), f.x), mix(lava_hash(i + vec3(0, 1, 0)), lava_hash(i + vec3(1, 1, 0)), f.x), f.y);
	float b = mix(mix(lava_hash(i + vec3(0, 0, 1)), lava_hash(i + vec3(1, 0, 1)), f.x), mix(lava_hash(i + vec3(0, 1, 1)), lava_hash(i + vec3(1, 1, 1)), f.x), f.y);
	return mix(a, b, f.z);
}

vec3 lava_color(vec3 cell, float t, out vec3 emission) {
	vec3 c = cell + 0.5;
	float plates = lava_noise(c * 0.21 + vec3(t * 0.06, t * 0.025, -t * 0.04));
	float crack = 1.0 - smoothstep(0.02, 0.16, abs(plates - 0.5));
	float churn = lava_noise(c * 0.55 + vec3(0.0, t * 0.18, t * 0.11));
	float heat = clamp(crack * (0.65 + 0.55 * churn) + 0.12 * churn * churn, 0.0, 1.0);
	float speck = lava_hash(cell);
	vec3 crust = mix(vec3(0.13, 0.045, 0.03), vec3(0.30, 0.09, 0.04), speck * (0.6 + 0.4 * churn));
	vec3 hot = mix(vec3(0.95, 0.28, 0.03), vec3(1.0, 0.66, 0.18), churn);
	emission = hot * heat * lava_glow * 0.75 + crust * 0.35;
	return mix(crust, hot, heat);
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
	float thr = 0.05 + 0.55 * (0.55 * h + 0.45 * blot);   // Noita coats: solid once soaked, ragged only at the edge
	if (ax != 1) {
		// walls: liquid sits low in the cell; the tide line wobbles per pixel column. Liquid that ran down from
		// the cell above wets the face top to bottom (a stream down a wall coats it, no stripes per voxel row).
		float local_h = pq.y - cell.y;
		float col_j = stain_hash(vec3(px.x + px.z, 3.0, px.x - px.z));
		float above = stain_cell(ivec3(cell) + ivec3(0, 1, 0)).a;
		a *= step(local_h, above > 0.02 ? 1.1 : 0.12 + 0.8 * a + 0.12 * (col_j - 0.5));
	} else if (n.y < 0.0) {
		a *= 0.35;            // ceilings: splashes and soot only
	}
	float cover = step(thr, a);
	return vec4(s.rgb / max(s.a, 1e-4), cover * (0.78 + 0.2 * min(a, 1.0)));
}


// Seasons (ADR 0025): atmosphere.gd sets these from the calendar. The climate map
// holds the temperature (-1..1, lapse included) around the player at 16 m, so
// winter snow settles only where the climate is cold or temperate and autumn
// colours stay out of the hot biomes. Deciduous leaves = the oak leaf tile.
uniform float season_autumn = 0.0;      // 0..1 how far the leaves have turned
uniform float season_winter = 0.0;      // 0..1 straw-coloured grass, brown oak leaves
uniform float season_snow = 0.0;        // 0..1 snow cover on top faces
uniform float snow_t_max = 0.2;         // warmest temperature where winter snow settles
uniform sampler2D climate_map : filter_linear, repeat_disable;
uniform vec4 climate_rect = vec4(0.0);  // x0, z0, size x, size z (world voxels); size 0 = no map yet
uniform float leaf_tile = 20.0;
uniform float tall_grass_tile = 23.0;
uniform float fern_tile = 72.0;

float climate_temperature(vec3 wpos) {
	if (climate_rect.z <= 0.0) {
		return 1.0;
	}
	vec2 uv = (wpos.xz - climate_rect.xy) / climate_rect.zw;
	if (any(lessThan(uv, vec2(0.0))) || any(greaterThan(uv, vec2(1.0)))) {
		return 1.0;
	}
	return texture(climate_map, uv).r * 2.0 - 1.0;
}

vec3 season_color(vec3 col, float idx, vec3 wpos, vec3 n) {
	if (season_autumn + season_winter + season_snow <= 0.0) {
		return col;
	}
	float t = climate_temperature(wpos);
	float seasonal = 1.0 - smoothstep(snow_t_max + 0.1, snow_t_max + 0.35, t);   // hot biomes stay green
	float lum = dot(col, vec3(0.30, 0.59, 0.11));
	float vh = stain_hash(floor(wpos) + 3.0);   // per voxel
	if (abs(idx - leaf_tile) < 0.5) {
		vec3 turn = mix(vec3(0.95, 0.74, 0.20), vec3(0.93, 0.40, 0.10), smoothstep(0.3, 0.7, vh));
		turn = mix(turn, vec3(0.72, 0.16, 0.08), step(0.82, vh));
		float a = clamp(season_autumn * 1.5 - vh * 0.5, 0.0, 1.0) * seasonal;
		col = mix(col, turn * lum * 2.4, a);
		col = mix(col, vec3(0.45, 0.31, 0.18) * lum * 2.2, season_winter * 0.85 * seasonal);
	} else if (abs(idx - grass_top_tile) < 0.5 || abs(idx - grass_side_tile) < 0.5 || abs(idx - tall_grass_tile) < 0.5
			|| abs(idx - fern_tile) < 0.5) {
		float green = step(col.r * 1.08, col.g) * step(col.b, col.g);   // the grass pixels, not the soil under them
		col = mix(col, vec3(0.70, 0.63, 0.40) * lum * 2.0, season_winter * 0.7 * green * seasonal);
		if (abs(idx - fern_tile) < 0.5) {
			col = mix(col, vec3(0.62, 0.42, 0.20) * lum * 2.2, season_autumn * 0.6 * seasonal);
		}
	}
	if (season_snow > 0.0 && n.y > 0.6 && abs(idx - lava_tile) > 0.5 && abs(idx - fire_tile) > 0.5) {
		float cold = 1.0 - smoothstep(snow_t_max - 0.06, snow_t_max + 0.04, t);
		float s = season_snow * cold;
		// ragged edge while it thins: pixels drop out by a per-pixel hash
		s *= step(stain_hash(floor(wpos * 16.0)) * 0.7, s * 1.2);
		col = mix(col, vec3(0.92, 0.94, 0.98), s);
	}
	return col;
}
