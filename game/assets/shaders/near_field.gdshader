shader_type spatial;
render_mode cull_front, depth_draw_always;

// Near field (ADR 0012 step 3): the cube around the player ray-cast per pixel
// from the brick world at 6.25 cm cells. Two-level DDA (8^3 brick occupancy,
// then cells) over a toroidal window texture (texel = world cell & (n - 1)),
// atlas-textured faces (each cell shows its 1/8 of the material's tile, like
// MicroMesher), see-through water. Only the VALID box is marched: the part of
// the window whose texels hold the current world (NearFieldFeed); rays leaving
// it see air, and the meshed terrain draws there (terrain_common:
// near_field_discard hides the meshes inside the same box).
//
// Lighting is Godot's: the shader writes ALBEDO, NORMAL, EMISSION, DEPTH and
// LIGHT_VERTEX (the hit position), so the sun, its shadow maps (cast by the
// terrain meshes, which still render in the shadow passes), the sky ambient,
// SSAO and fog are the same on both sides of the seam.

#include "res://assets/shaders/terrain_common.gdshaderinc"

uniform sampler2DArray vol : filter_nearest, repeat_disable;
uniform sampler2DArray occ : filter_nearest, repeat_disable;
uniform int n = 512;                 // window cells per edge (power of two)
uniform ivec3 win_origin = ivec3(0); // world cell of the window's min corner (multiple of 16)
uniform ivec3 valid_min = ivec3(0);  // world cells holding current data, inclusive
uniform ivec3 valid_max = ivec3(0);  // exclusive
uniform float cell = 0.125;          // world units per cell (1 unit = 1 voxel = 8 cells)
uniform int max_bricks = 192;        // brick steps per ray (the cube's diagonal in bricks)
uniform int tile_top[64];
uniform int tile_side[64];
uniform int tile_bottom[64];
uniform int water_id = 5;
uniform int water_static_id = 23;
uniform int paint_base = 128;        // cells >= this are painted micro cells: atlas tile = id - paint_base on every face
uniform float water_mix = 0.45;      // how much the water colour tints what lies under it
uniform float debug_tint = 0.0;      // > 0: ray-cast pixels blend toward magenta (near_debug=1), to see where the near field draws

// window-relative cell bounds of the valid box (set per frame with the uniforms above)
ivec3 lo_cells() { return valid_min - win_origin; }
ivec3 hi_cells() { return valid_max - win_origin; }

void vertex() {
	v_world = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

float fetch(ivec3 c, ivec3 lo, ivec3 hi) {
	if (any(lessThan(c, lo)) || any(greaterThanEqual(c, hi))) return 0.0;
	return texelFetch(vol, (c + win_origin) & ivec3(n - 1), 0).r * 255.0;
}

float brick(ivec3 b, ivec3 blo, ivec3 bhi) {
	if (any(lessThan(b, blo)) || any(greaterThan(b, bhi))) return 0.0;
	return texelFetch(occ, (b + (win_origin >> 3)) & ivec3((n >> 3) - 1), 0).r;
}

bool is_water(float m) {
	int id = int(m + 0.5);
	return id == water_id || id == water_static_id;
}

// p: start in window cells; d: direction; nrm0: normal of the face the ray entered through (zero inside);
// tmax: cells of ray allowed. Returns the material hit (0 = none), its cell, face normal and distance in cells.
float march(vec3 p, vec3 d, vec3 nrm0, float tmax, bool skip_water, out ivec3 hit, out vec3 nrm, out float t) {
	ivec3 lo = lo_cells();
	ivec3 hi = hi_cells();
	ivec3 blo = lo >> 3;
	ivec3 bhi = (hi - 1) >> 3;
	ivec3 s = ivec3(sign(d));
	vec3 inv = 1.0 / max(abs(d), vec3(1e-6));
	ivec3 cb = ivec3(floor(p / 8.0));
	vec3 tmaxb = (vec3(cb) * 8.0 + max(vec3(s), vec3(0.0)) * 8.0 - p) * inv * vec3(s);
	vec3 tdeltab = inv * 8.0;
	float tb = 0.0;
	vec3 nrmb = nrm0;
	for (int i = 0; i < max_bricks; i++) {
		if (tb > tmax) break;
		if (any(lessThan(cb, blo)) || any(greaterThan(cb, bhi))) {
			if (i > 0) break;
		} else if (brick(cb, blo, bhi) > 0.5) {
			vec3 pf = p + d * (tb + 1e-3);
			ivec3 c = clamp(ivec3(floor(pf)), cb * 8, cb * 8 + 7);
			vec3 tmax3 = (vec3(c) + max(vec3(s), vec3(0.0)) - pf) * inv * vec3(s);
			vec3 nf = nrmb;
			float tf = 0.0;
			for (int j = 0; j < 26; j++) {
				if (any(lessThan(c, cb * 8)) || any(greaterThan(c, cb * 8 + 7))) break;
				float m = fetch(c, lo, hi);
				if (m > 0.5 && !(skip_water && is_water(m))) { hit = c; nrm = nf; t = tb + 1e-3 + tf; return m; }
				if (tmax3.x < tmax3.y && tmax3.x < tmax3.z) { tf = tmax3.x; tmax3.x += inv.x; c.x += s.x; nf = vec3(-float(s.x), 0.0, 0.0); }
				else if (tmax3.y < tmax3.z) { tf = tmax3.y; tmax3.y += inv.y; c.y += s.y; nf = vec3(0.0, -float(s.y), 0.0); }
				else { tf = tmax3.z; tmax3.z += inv.z; c.z += s.z; nf = vec3(0.0, 0.0, -float(s.z)); }
			}
		}
		if (tmaxb.x < tmaxb.y && tmaxb.x < tmaxb.z) { tb = tmaxb.x; tmaxb.x += tdeltab.x; cb.x += s.x; nrmb = vec3(-float(s.x), 0.0, 0.0); }
		else if (tmaxb.y < tmaxb.z) { tb = tmaxb.y; tmaxb.y += tdeltab.y; cb.y += s.y; nrmb = vec3(0.0, -float(s.y), 0.0); }
		else { tb = tmaxb.z; tmaxb.z += tdeltab.z; cb.z += s.z; nrmb = vec3(0.0, 0.0, -float(s.z)); }
	}
	return 0.0;
}

// Atlas colour of a cell face: the material's tile for that face, 1/8 of it per cell (wc = world cell).
vec3 face_color(int id, ivec3 wc, vec3 nrm, out int tile) {
	if (id >= paint_base) {
		tile = id - paint_base;
	} else {
		tile = tile_side[clamp(id, 0, 63)];
		if (nrm.y > 0.5) tile = tile_top[clamp(id, 0, 63)];
		else if (nrm.y < -0.5) tile = tile_bottom[clamp(id, 0, 63)];
	}
	ivec3 l = wc & ivec3(7);
	vec2 f;
	if (abs(nrm.y) > 0.5) f = vec2(float(l.x), float(l.z));
	else if (abs(nrm.x) > 0.5) f = vec2(float(l.z), 7.0 - float(l.y));
	else f = vec2(float(l.x), 7.0 - float(l.y));
	vec2 tile_uv = (vec2(float(tile % int(atlas_tiles.x)), float(tile / int(atlas_tiles.x))) + (f + 0.5) / 8.0) / atlas_tiles;
	return textureLod(atlas, tile_uv, 0.0).rgb;
}

void fragment() {
	if (any(greaterThanEqual(valid_min, valid_max))) {
		discard;
	}
	vec3 ro = CAMERA_POSITION_WORLD;
	vec3 rd = normalize(v_world - ro);
	vec3 bmin = vec3(valid_min) * cell;
	vec3 bmax = vec3(valid_max) * cell;
	vec3 inv = 1.0 / rd;
	vec3 t0s = (bmin - ro) * inv;
	vec3 t1s = (bmax - ro) * inv;
	vec3 tmin3 = min(t0s, t1s);
	vec3 tmax3 = max(t0s, t1s);
	float tenter = max(max(tmin3.x, tmin3.y), tmin3.z);
	float texit = min(min(tmax3.x, tmax3.y), tmax3.z);
	if (texit <= max(tenter, 0.0)) {
		discard;
	}
	float tstart = max(tenter, 0.0) + 1e-4;
	// the face the ray enters through, when the camera is outside the valid box
	vec3 nrm0 = vec3(0.0);
	if (tenter > 0.0) {
		if (tmin3.x >= tmin3.y && tmin3.x >= tmin3.z) nrm0 = vec3(-sign(rd.x), 0.0, 0.0);
		else if (tmin3.y >= tmin3.z) nrm0 = vec3(0.0, -sign(rd.y), 0.0);
		else nrm0 = vec3(0.0, 0.0, -sign(rd.z));
	}
	vec3 p = (ro + rd * tstart) / cell - vec3(win_origin);
	float tmax_cells = (texit - tstart) / cell;
	ivec3 hit;
	vec3 nrm;
	float t;
	float m = march(p, rd, nrm0, tmax_cells, false, hit, nrm, t);
	if (m < 0.5) {
		discard;
	}
	int id = int(m + 0.5);
	float water_t = -1.0;
	vec3 water_nrm = vec3(0.0);
	ivec3 water_hit = hit;
	if (is_water(m)) {
		water_t = t;
		water_nrm = nrm;
		ivec3 h2; vec3 n2; float t2;
		float m2 = march(vec3(hit) + 0.5 + rd * 0.01, rd, vec3(0.0), tmax_cells - t, true, h2, n2, t2);
		if (m2 > 0.5) { id = int(m2 + 0.5); hit = h2; nrm = n2; t = t + t2 + 0.5; }
		else { id = 0; }
	}
	int tile = 0;
	vec3 col;
	vec3 shade_nrm = nrm;
	if (id == 0) {
		// water with nothing solid behind it inside the box: the water itself
		col = face_color(water_id, water_hit + win_origin, water_nrm, tile);
		shade_nrm = water_nrm;
	} else {
		col = face_color(id, hit + win_origin, nrm, tile);
	}
	vec3 hitw = ro + rd * (tstart + t * cell);
	float rough = 1.0;
	float spec = 0.0;
	if (water_t >= 0.0) {
		float depth_in_water = clamp((t - water_t) * cell * 0.5 / 3.0, 0.0, 1.0);
		int wtile;
		vec3 water_col = face_color(water_id, water_hit + win_origin, water_nrm, wtile);
		col = mix(col, water_col, water_mix + 0.5 * depth_in_water);
		shade_nrm = water_nrm;
		tile = wtile;
		t = water_t;
		hitw = ro + rd * (tstart + t * cell);
		if (water_nrm.y > 0.9) {
			shade_nrm = ripple_normal(hitw.xz);
			spec = water_specular;
			rough = water_roughness;
		}
	}
	ALBEDO = col * face_tint(shade_nrm.y > 0.9 && water_t >= 0.0 ? water_nrm : shade_nrm);
	ALBEDO = mix(ALBEDO, vec3(1.0, 0.0, 1.0), debug_tint);
	NORMAL = normalize((VIEW_MATRIX * vec4(shade_nrm, 0.0)).xyz);
	ROUGHNESS = rough;
	METALLIC = 0.0;
	SPECULAR = spec;
	EMISSION = tile_emission(float(tile), col);
	vec4 vp = VIEW_MATRIX * vec4(hitw, 1.0);
	LIGHT_VERTEX = vp.xyz;
	vec4 cp = PROJECTION_MATRIX * vp;
	DEPTH = cp.z / cp.w;
}
