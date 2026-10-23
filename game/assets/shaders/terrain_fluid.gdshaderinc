// Minecraft-style liquid surfaces (godot_voxel VoxelBlockyFluid): the blocky
// mesher builds each liquid voxel at its level (eighths from the simulation,
// SimWorld fullness) with the top corners raised to the highest neighbour, so
// the surface slopes from full cells down to thin films and over edges. It
// passes no atlas UVs: UV.y is the flow state (0..7 = 45 degree steps from +x
// towards -z, 8 = idle). The texture comes from the world position on the
// face plane and scrolls along the flow; side faces run downwards.
// Included by terrain_fluid.gdshader (see-through) and
// terrain_fluid_opaque.gdshader (lava); each defines render_mode itself.

#include "res://assets/shaders/terrain_common.gdshaderinc"

uniform float fluid_tile = 5.0;           // the liquid's atlas tile
uniform float flow_speed = 0.35;          // texture tiles per second along the flow
uniform float liquid_alpha : hint_range(0.0, 1.0) = 0.68;
uniform float edge_alpha_boost = 0.25;

varying vec2 v_flow;

void vertex() {
	vcol = COLOR;
	v_world = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	v_normal_world = normalize((MODEL_MATRIX * vec4(NORMAL, 0.0)).xyz);
	v_flow = UV;
}

void fragment() {
	if (lod_fade_discard(FRAGCOORD.xy)) {
		discard;
	}
	vec3 n = v_normal_world;
	vec3 an = abs(n);
	bool top = an.y > 0.5;
	// the two world axes across the face; -y so the tile's top row stays at the top
	vec2 wc = top ? v_world.xz : (an.x > 0.5 ? vec2(v_world.z, -v_world.y) : vec2(v_world.x, -v_world.y));
	vec2 shift = vec2(0.0);
	float fs = floor(v_flow.y + 0.5);
	if (top) {
		if (fs < 7.5) {
			float a = fs * 0.785398;
			shift = vec2(cos(a), -sin(a)) * TIME * flow_speed;
		}
	} else {
		shift = vec2(0.0, -2.0) * TIME * flow_speed;   // falling down the side
	}
	vec2 p = wc - shift;
	vec2 tile = vec2(mod(fluid_tile, atlas_tiles.x), floor(fluid_tile / atlas_tiles.x));
	vec2 uv = (tile + fract(p)) / atlas_tiles;
	// gradients of the unwrapped coordinate: no mip seam where fract() wraps
	vec4 c = textureGrad(atlas, uv, dFdx(p) / atlas_tiles, dFdy(p) / atlas_tiles);
	ALBEDO = c.rgb * vcol.rgb * face_tint(n);
	ROUGHNESS = 1.0;
	METALLIC = 0.0;
	SPECULAR = 0.0;
	EMISSION = tile_emission(fluid_tile, c.rgb);
	if (top && abs(fluid_tile - water_tile) < 0.5) {
		NORMAL = normalize((VIEW_MATRIX * vec4(ripple_normal(v_world.xz + shift), 0.0)).xyz);
		SPECULAR = water_specular;
		ROUGHNESS = water_roughness;
	}
#ifdef FLUID_SEE_THROUGH
	float facing = abs(dot(normalize(VIEW), NORMAL));
	ALPHA = clamp(liquid_alpha + (1.0 - facing) * edge_alpha_boost, 0.0, 1.0);
#endif
}
