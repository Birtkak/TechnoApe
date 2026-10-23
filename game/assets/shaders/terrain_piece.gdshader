shader_type spatial;
render_mode cull_back, depth_draw_opaque;

// Rigid voxel pieces (P12, scripts/physics/falling_pieces.gd): a felled tree or a cut-off limb in
// flight. Same flat colours as the terrain (voxel_sample), but hashed at the position each voxel had
// when it broke off (mesh origin = that voxel corner, piece_origin = its world voxel), so a falling
// trunk keeps its colours instead of shimmering as it moves; lighting and face tint use the world normal.

#include "res://assets/shaders/terrain_common.gdshaderinc"

instance uniform vec3 piece_origin = vec3(0.0);

varying vec3 v_hash;
varying vec3 v_nlocal;

void vertex() {
	vcol = COLOR;
	v_hash = VERTEX + piece_origin;
	v_nlocal = NORMAL;
	v_world = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	v_normal_world = normalize((MODEL_MATRIX * vec4(NORMAL, 0.0)).xyz);
}

void fragment() {
	vec4 c;
	float idx = voxel_sample(UV, v_hash, v_nlocal, CAMERA_POSITION_WORLD + v_hash - v_world, c);
	c.rgb = season_color(c.rgb, idx, v_hash, v_nlocal);
	ALBEDO = c.rgb * vcol.rgb * face_tint(v_normal_world);
	ROUGHNESS = 1.0;
	METALLIC = 0.0;
	SPECULAR = 0.0;
	EMISSION = tile_emission(idx, c.rgb);
}
