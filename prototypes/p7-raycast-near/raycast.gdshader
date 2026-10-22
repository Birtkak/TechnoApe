shader_type spatial;
render_mode unshaded, cull_front, depth_draw_always;

// Ray-casts a cubic micro-voxel volume (R8 material ids, one 2D-array layer per
// z slice) per pixel, Grimorium / "3D Noita" style: no mesh, every cell visible
// the frame it changes. Two-level DDA: 8^3 bricks from the occupancy texture
// are skipped in one stride, cells inside occupied bricks are walked one by
// one. Writes DEPTH so ordinary meshes composite correctly; lights with one
// lambert term and one shadow ray towards the sun. The mesh this material sits
// on is the volume's bounding box (front faces culled so it works from inside).

uniform sampler2DArray vol : filter_nearest, repeat_disable;
uniform sampler2DArray occ : filter_nearest, repeat_disable;
uniform int n = 256;                 // cells per edge
uniform vec3 vol_origin = vec3(0.0); // world position of cell (0,0,0)
uniform float cell = 0.0625;         // world units per cell
uniform vec3 sun_dir = vec3(0.4, 0.8, 0.3);
uniform vec3 palette[8];
uniform int max_bricks = 200;

varying vec3 v_world;

void vertex() {
	v_world = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

float fetch(ivec3 c) {
	if (any(lessThan(c, ivec3(0))) || any(greaterThanEqual(c, ivec3(n)))) return 0.0;
	return texelFetch(vol, c, 0).r * 255.0;
}

float brick(ivec3 b) {
	int nb = n >> 3;
	if (any(lessThan(b, ivec3(0))) || any(greaterThanEqual(b, ivec3(nb)))) return 0.0;
	return texelFetch(occ, b, 0).r;
}

// Two-level DDA from p (cell space) along d. Returns the material id or 0;
// hit cell, face normal and t (in cells along the ray) on a hit.
float march(vec3 p, vec3 d, int bricks, out ivec3 hit, out vec3 nrm, out float t) {
	ivec3 s = ivec3(sign(d));
	vec3 inv = 1.0 / max(abs(d), vec3(1e-6));
	// brick level
	ivec3 cb = ivec3(floor(p / 8.0));
	vec3 tmaxb = (vec3(cb) * 8.0 + max(vec3(s), vec3(0.0)) * 8.0 - p) * inv * vec3(s);
	vec3 tdeltab = inv * 8.0;
	float tb = 0.0;
	vec3 nrmb = vec3(0.0);
	int nb = n >> 3;
	for (int i = 0; i < bricks; i++) {
		if (any(lessThan(cb, ivec3(0))) || any(greaterThanEqual(cb, ivec3(nb)))) {
			if (i > 0) break; // left the volume
		} else if (brick(cb) > 0.5) {
			// cell level inside this brick, from the brick entry point
			vec3 pf = p + d * (tb + 1e-3);
			ivec3 c = clamp(ivec3(floor(pf)), cb * 8, cb * 8 + 7);
			vec3 tmax = (vec3(c) + max(vec3(s), vec3(0.0)) - pf) * inv * vec3(s);
			vec3 nf = nrmb;
			float tf = 0.0;
			for (int j = 0; j < 26; j++) {
				if (any(lessThan(c, cb * 8)) || any(greaterThan(c, cb * 8 + 7))) break;
				float m = fetch(c);
				if (m > 0.5) { hit = c; nrm = nf; t = tb + 1e-3 + tf; return m; }
				if (tmax.x < tmax.y && tmax.x < tmax.z) { tf = tmax.x; tmax.x += inv.x; c.x += s.x; nf = vec3(-float(s.x), 0.0, 0.0); }
				else if (tmax.y < tmax.z) { tf = tmax.y; tmax.y += inv.y; c.y += s.y; nf = vec3(0.0, -float(s.y), 0.0); }
				else { tf = tmax.z; tmax.z += inv.z; c.z += s.z; nf = vec3(0.0, 0.0, -float(s.z)); }
			}
		}
		if (tmaxb.x < tmaxb.y && tmaxb.x < tmaxb.z) { tb = tmaxb.x; tmaxb.x += tdeltab.x; cb.x += s.x; nrmb = vec3(-float(s.x), 0.0, 0.0); }
		else if (tmaxb.y < tmaxb.z) { tb = tmaxb.y; tmaxb.y += tdeltab.y; cb.y += s.y; nrmb = vec3(0.0, -float(s.y), 0.0); }
		else { tb = tmaxb.z; tmaxb.z += tdeltab.z; cb.z += s.z; nrmb = vec3(0.0, 0.0, -float(s.z)); }
	}
	return 0.0;
}

void fragment() {
	vec3 ro = CAMERA_POSITION_WORLD;
	vec3 rd = normalize(v_world - ro);
	vec3 bmin = vol_origin;
	vec3 bmax = vol_origin + vec3(float(n)) * cell;
	vec3 inv = 1.0 / rd;
	vec3 t0s = (bmin - ro) * inv;
	vec3 t1s = (bmax - ro) * inv;
	vec3 tmin3 = min(t0s, t1s);
	vec3 tmax3 = max(t0s, t1s);
	float tenter = max(max(tmin3.x, tmin3.y), tmin3.z);
	float texit = min(min(tmax3.x, tmax3.y), tmax3.z);
	if (texit <= max(tenter, 0.0)) discard;
	float tstart = max(tenter, 0.0) + 1e-4;
	vec3 p = (ro + rd * tstart - vol_origin) / cell;
	ivec3 hit;
	vec3 nrm;
	float t;
	float m = march(p, rd, max_bricks, hit, nrm, t);
	if (m < 0.5) discard;
	int id = int(m + 0.5);
	vec3 base = palette[clamp(id, 0, 7)];
	vec3 sun = normalize(sun_dir);
	float lambert = max(dot(nrm, sun), 0.0);
	float shade = 1.0;
	if (lambert > 0.0) {
		ivec3 h2; vec3 n2; float t2;
		vec3 sp = vec3(hit) + 0.5 + nrm * 0.51;
		if (march(sp, sun, 40, h2, n2, t2) > 0.5) shade = 0.0;
	}
	vec3 col = base * (0.28 + 0.72 * lambert * shade);
	if (id == 6) col = base * 1.6;
	ALBEDO = col;
	vec3 hitw = ro + rd * (tstart + t * cell);
	vec4 cp = PROJECTION_MATRIX * VIEW_MATRIX * vec4(hitw, 1.0);
	DEPTH = cp.z / cp.w;
}
