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

#ifdef FLUID_SEE_THROUGH
// Water (Lay of the Land look, ADR 0036 look pass): no texture; the colour and opacity come from the
// water's thickness along the view ray (depth buffer behind the surface), clear turquoise over the
// bed in the shallows, dark teal and nearly opaque in the deep, a pale rim where it meets the shore.
uniform sampler2D depth_tex : hint_depth_texture, filter_nearest;
uniform float metres_per_unit = 0.125;
uniform vec3 water_shallow : source_color = vec3(0.22, 0.82, 0.70);
uniform vec3 water_deep : source_color = vec3(0.02, 0.34, 0.40);
uniform float water_depth_m = 3.0;    // metres of water for ~63 % of the deep colour
uniform float water_clear_m = 1.1;    // metres of water for ~63 % of the opacity
uniform float water_alpha_min = 0.22;
uniform float water_alpha_max = 0.94;
uniform float water_rim_m = 0.1;      // the pale shore rim fades out over this much water
uniform float water_rim = 0.35;
uniform float waterfall_speed = 40.0;  // voxels per second the foam streaks run down a falling face (5 m/s)
#endif

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
	if (abs(fluid_tile - lava_tile) < 0.5) {
		vec3 glow;
		ALBEDO = lava_color(floor(v_world - n * 0.01), TIME, glow) * vcol.rgb * face_tint(n);
		EMISSION = glow;
	}
	if (top && abs(fluid_tile - water_tile) < 0.5) {
		NORMAL = normalize((VIEW_MATRIX * vec4(ripple_normal(v_world.xz + shift), 0.0)).xyz);
		SPECULAR = water_specular * 0.6;   // the sky sheen washed the colour out
		ROUGHNESS = water_roughness;
	}
#ifdef FLUID_SEE_THROUGH
	float facing = abs(dot(normalize(VIEW), NORMAL));
	if (abs(fluid_tile - water_tile) < 0.5) {
		float zb = texture(depth_tex, SCREEN_UV).r;
		float d_m = 1000.0;   // sky behind: as deep as it gets
		if (zb > 0.0) {
			vec4 vb = INV_PROJECTION_MATRIX * vec4(SCREEN_UV * 2.0 - 1.0, zb, 1.0);
			d_m = max(length(vb.xyz / vb.w) - length(VERTEX), 0.0) * metres_per_unit;
		}
		float rim = (1.0 - smoothstep(0.0, water_rim_m, d_m)) * water_rim;
		vec3 col = mix(water_shallow, water_deep, 1.0 - exp(-d_m / water_depth_m));
		ALBEDO = mix(col, vec3(0.86, 0.96, 0.95), rim) * face_tint(n);
		ALPHA = clamp(mix(water_alpha_min, water_alpha_max, 1.0 - exp(-d_m / water_clear_m)) + (1.0 - facing) * edge_alpha_boost + rim, 0.0, 1.0);
		if (an.y < 0.5) {
			// ADR 0049: water falling down a face (a waterfall sheet, a spill over a ledge) reads as foam streaks
			// running down in voxel-wide lanes, each at its own speed, so a still sheet no longer looks frozen
			float lane = floor(wc.x);
			float speed = waterfall_speed * (0.75 + 0.5 * lava_hash(vec3(lane, 1.0, 3.0)));
			float v = fract((wc.y - TIME * speed) * 0.125 + lava_hash(vec3(lane, 7.0, 2.0)));
			float streak = smoothstep(0.6, 0.95, v) * (0.55 + 0.45 * lava_hash(vec3(lane, 4.0, 9.0)));
			ALBEDO = mix(ALBEDO, vec3(0.9, 0.97, 1.0), streak * 0.7);
			ALPHA = clamp(max(ALPHA, 0.55) + streak * 0.35, 0.0, 1.0);
		}
	} else {
		ALPHA = clamp(liquid_alpha + (1.0 - facing) * edge_alpha_boost, 0.0, 1.0);
	}
#endif
}
