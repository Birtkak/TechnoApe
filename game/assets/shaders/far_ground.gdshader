shader_type spatial;
render_mode cull_back, depth_draw_opaque, world_vertex_coords;

// The far layer's ground skin (ADR 0074, scripts/visuals/far_layer.gd, native FarMesh). Vertex
// colours are the columns' top materials (tile averages, linear); UV.x holds the height and CUSTOM0
// the colour the vertex has at the parent level, UV2 the parent normal's xz: over the outer part of
// its ring a tile morphs into its parent (CDLOD), so a ring change never pops. Pixels nearer than
// near_cut belong to the near box, whose horizon blend fades into this layer's texture instead.

uniform float morph_start = 0.0;    // world voxels from the camera
uniform float morph_end = 1.0;
uniform float near_cut = 0.0;
uniform float face_tint_side = 0.86;   // terrain_common's face tint, on the smooth normal
uniform float speckle = 0.35;          // per-voxel colour noise, as the near voxels have (terrain_common's voxel_sample)
uniform float speckle_fade_end = 1600.0;   // world voxels: the speckle is gone by here (sub-pixel anyway)

varying vec3 v_world;
varying vec3 v_col;
varying vec3 v_nrm;

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
	if (distance(v_world, CAMERA_POSITION_WORLD) < near_cut) {
		discard;
	}
	vec3 n = normalize(v_nrm);
	float up = clamp(n.y, 0.0, 1.0);
	vec3 col = v_col;
	if (speckle > 0.0) {
		// one shade per 25 cm cell, a little lighter or darker, like the palette noise of the voxels; it
		// fades in from the near box's edge (whose far_fade has flattened its colours by then) and out
		// where a voxel is far below a pixel
		float d = distance(v_world, CAMERA_POSITION_WORLD);
		ivec3 cell = ivec3(floor(v_world * 0.5));
		uint h = uint(cell.x) * 73856093u ^ uint(cell.y) * 19349663u ^ uint(cell.z) * 83492791u;
		h ^= h >> 13u;
		h *= 0x5bd1e995u;
		h ^= h >> 15u;
		float s = (float(h & 1023u) / 1023.0 - 0.5) * 2.0;
		float w = speckle * smoothstep(near_cut, near_cut * 2.0, d) * (1.0 - smoothstep(speckle_fade_end * 0.5, speckle_fade_end, d));
		col *= 1.0 + 0.22 * s * w;
	}
	ALBEDO = col * (up + (1.0 - up) * face_tint_side);
	ROUGHNESS = 1.0;
	METALLIC = 0.0;
	SPECULAR = 0.0;
}
