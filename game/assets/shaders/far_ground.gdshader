shader_type spatial;
render_mode cull_back, depth_draw_opaque, world_vertex_coords;

// The far layer's ground skin (ADR 0074, scripts/visuals/far_layer.gd, native FarMesh). Vertex
// colours are the columns' top materials (tile averages, linear); UV.x holds the height and CUSTOM0
// the colour the vertex has at the parent level, UV2 the parent normal's xz: over the outer part of
// its ring a tile morphs into its parent (CDLOD), so a ring change never pops. Pixels nearer than
// near_cut belong to the near box, whose horizon blend fades into this layer's texture instead.
// The skin carries what the near voxels show from afar: a per-voxel speckle, small relief (a noise
// tilt of the normal and darker hollows), strata and darker faces on steep slopes, a slight hue drift
// per patch of land. All of it fades in from the box's edge, where far_fade has flattened the voxels
// to their tile average, so the join stays invisible.

uniform float morph_start = 0.0;    // world voxels from the camera
uniform float morph_end = 1.0;
uniform float near_cut = 0.0;
uniform float face_tint_side = 0.86;   // terrain_common's face tint, on the smooth normal
uniform float speckle = 0.35;          // per-voxel colour noise, as the near voxels have (terrain_common's voxel_sample)
uniform float speckle_fade_end = 1600.0;   // world voxels: the speckle is gone by here (sub-pixel anyway)
uniform float relief = 1.0;            // strength of the noise relief (0 = the smooth skin)
uniform float relief_fade_end = 6400.0;    // world voxels: the relief is calm past here (the horizon stays quiet)

varying vec3 v_world;
varying vec3 v_col;
varying vec3 v_nrm;

uint hash3(ivec3 c) {
	uint h = uint(c.x) * 73856093u ^ uint(c.y) * 19349663u ^ uint(c.z) * 83492791u;
	h ^= h >> 13u;
	h *= 0x5bd1e995u;
	h ^= h >> 15u;
	return h;
}

float unit3(ivec3 c) {
	return float(hash3(c) & 1023u) / 1023.0;
}

// value noise over the ground plane, 0..1, cells of `size` voxels
float vnoise(vec2 p, float size, int salt) {
	vec2 q = p / size;
	ivec2 i = ivec2(floor(q));
	vec2 f = fract(q);
	f = f * f * (3.0 - 2.0 * f);
	float a = unit3(ivec3(i.x, salt, i.y));
	float b = unit3(ivec3(i.x + 1, salt, i.y));
	float c = unit3(ivec3(i.x, salt, i.y + 1));
	float d = unit3(ivec3(i.x + 1, salt, i.y + 1));
	return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

void vertex() {
	float d = distance(VERTEX, CAMERA_POSITION_WORLD);
	float m = smoothstep(morph_start, morph_end, d);
	VERTEX.y = mix(VERTEX.y, UV.x, m);
	vec3 pn = vec3(UV2.x, 0.0, UV2.y);
	pn.y = sqrt(max(0.0, 1.0 - dot(pn, pn)));
	NORMAL = normalize(mix(NORMAL, pn, m));
	v_nrm = NORMAL;
	v_col = mix(COLOR.rgb, CUSTOM0.rgb, m);
	v_world = VERTEX;
}

void fragment() {
	float d = distance(v_world, CAMERA_POSITION_WORLD);
	if (d < near_cut) {
		discard;
	}
	vec3 n = normalize(v_nrm);
	vec3 col = v_col;
	// everything past the tile average fades in over the box's edge (far_fade has flattened the voxels there)
	float w_in = smoothstep(near_cut, near_cut * 2.0, d);
	vec2 pxz = v_world.xz;
	if (relief > 0.0) {
		// relief: a 2 m and an 8 m octave of value noise tilt the normal (lit by the sun it reads as
		// hummocks and roughness) and darken the hollows a little, calm towards the horizon
		float wr = relief * w_in * (1.0 - smoothstep(relief_fade_end * 0.4, relief_fade_end, d));
		if (wr > 0.0) {
			float e = 2.0;
			float h0 = vnoise(pxz, 16.0, 1) * 0.6 + vnoise(pxz, 64.0, 2);
			float hx = vnoise(pxz + vec2(e, 0.0), 16.0, 1) * 0.6 + vnoise(pxz + vec2(e, 0.0), 64.0, 2);
			float hz = vnoise(pxz + vec2(0.0, e), 16.0, 1) * 0.6 + vnoise(pxz + vec2(0.0, e), 64.0, 2);
			vec3 tilt = vec3(-(hx - h0), 0.0, -(hz - h0)) * (3.0 * wr);
			n = normalize(n + tilt);
			col *= 1.0 - 0.16 * wr * (1.0 - h0 / 1.6);
		}
	}
	float up = clamp(n.y, 0.0, 1.0);
	// a slight hue drift per 16 m patch so a plain is not one painted colour
	float t = (vnoise(pxz, 128.0, 3) - 0.5) * 2.0 * w_in;
	col *= vec3(1.0 + 0.05 * t, 1.0 + 0.02 * t, 1.0 - 0.05 * t);
	// steep ground: strata of 1.5 m with their own shade, and the face darker the steeper it is
	float steep = smoothstep(0.85, 0.55, up) * w_in;
	if (steep > 0.0) {
		float band = unit3(ivec3(int(floor(v_world.y / 12.0)), 7, 0));
		col *= mix(1.0, 0.8 + 0.3 * band, steep);
	}
	if (speckle > 0.0) {
		// one shade per 25 cm cell, a little lighter or darker, like the palette noise of the voxels, and
		// a second octave per metre; out where a voxel is far below a pixel
		float s = (unit3(ivec3(floor(v_world * 0.5))) - 0.5) * 2.0;
		float s2 = (unit3(ivec3(floor(v_world * 0.125)) + ivec3(0, 101, 0)) - 0.5) * 2.0;
		float w = speckle * w_in * (1.0 - smoothstep(speckle_fade_end * 0.5, speckle_fade_end, d));
		col *= 1.0 + 0.22 * s * w + 0.1 * s2 * w;
	}
	ALBEDO = col * mix(face_tint_side * 0.8, 1.0, up);
	NORMAL = (VIEW_MATRIX * vec4(n, 0.0)).xyz;
	ROUGHNESS = 1.0;
	METALLIC = 0.0;
	SPECULAR = 0.0;
}
