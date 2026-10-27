// Minecraft-style liquid surfaces (godot_voxel VoxelBlockyFluid): the blocky
// mesher builds each liquid voxel at its level (eighths from the simulation,
// SimWorld fullness) with the top corners raised to the highest neighbour, so
// the surface slopes from full cells down to thin films and over edges. It
// passes no atlas UVs: UV.y is the flow state (0..7 = 45 degree steps from +x
// towards -z, 8 = idle). The surface coordinate comes from the world position
// on the face plane and drifts along the flow; side faces run downwards.
// Included by terrain_fluid.gdshader (see-through) and
// terrain_fluid_opaque.gdshader (lava); each defines render_mode itself.
//
// Every liquid gets the water treatment (bug report 2026-10-25 "give all liquids the water look"):
// no atlas texture, a colour from the liquid's thickness along the view ray (see-through ones), ripple
// normals with a sun glint on the top, a rim where it meets the shore, lanes of foam running down a
// falling face. Each liquid's material sets its own character (data/liquid_looks.json, World): oil a
// dark glossy film with a rainbow sheen, blood deep red, toxic sludge a faintly glowing green with
// bubbles, lava (opaque) crust plates drifting on glowing cracks under a slow heavy swell.

#include "res://assets/shaders/terrain_common.gdshaderinc"

uniform float fluid_tile = 5.0;           // the liquid's atlas tile (lava picks its own path by it)
uniform float flow_speed = 0.35;
uniform float steep_alpha = 0.7;          // opacity floor of a steep top (a cascade sheet, ADR 0069 A3)          // surface tiles per second along the flow
uniform float liquid_alpha : hint_range(0.0, 1.0) = 0.68;
uniform float edge_alpha_boost = 0.25;

// the look (defaults = water)
uniform vec3 liquid_shallow : source_color = vec3(0.22, 0.82, 0.70);
uniform vec3 liquid_deep : source_color = vec3(0.02, 0.34, 0.40);
uniform float depth_m = 3.0;          // metres of liquid for ~63 % of the deep colour
uniform float clear_m = 1.1;          // metres of liquid for ~63 % of the opacity
uniform float alpha_min = 0.22;
uniform float alpha_max = 0.94;
uniform vec3 rim_color : source_color = vec3(0.86, 0.96, 0.95);
uniform float rim_m = 0.1;            // the pale shore rim fades out over this much liquid
uniform float rim = 0.35;
uniform vec3 foam_color : source_color = vec3(0.9, 0.97, 1.0);
uniform float foam = 0.7;             // how strongly the streaks on a falling face show
uniform float fall_speed = 40.0;      // voxels per second the streaks run down a falling face (5 m/s)
uniform float sheet_m = 1.5;          // a falling face colours as water this deep at most (a sheet, not an abyss)
uniform float ripple_scale = 1.0;     // ripple frequency (< 1 = broad swell)
uniform float ripple_speed = 1.0;
uniform float ripple_amount = 1.0;
uniform float surface_specular = 0.33;
uniform float surface_roughness = 0.14;
uniform float glow = 0.0;             // emission = colour * glow (toxic sludge)
uniform float mottle = 0.0;           // slow drifting patches in the colour (0 = water's even colour)
uniform float sheen = 0.0;            // oil: a thin-film rainbow on the top
uniform float bubbles = 0.0;          // toxic sludge: bubbles swell and pop on the top
uniform float metres_per_unit = 0.125;

#ifdef FLUID_SEE_THROUGH
uniform sampler2D depth_tex : hint_depth_texture, filter_nearest;
#endif

varying vec2 v_flow;

void vertex() {
	vcol = COLOR;
	v_world = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	v_normal_world = normalize((MODEL_MATRIX * vec4(NORMAL, 0.0)).xyz);
	v_flow = UV;
}

// ripple_normal (terrain_common) with this liquid's scale, speed and strength
vec3 liquid_ripple(vec2 p) {
	vec2 q = p * ripple_scale;
	float t = TIME * 1.1 * ripple_speed;
	float dx = sin(q.x * 2.3 + t * 1.3) * 0.5 + sin((q.x + q.y) * 1.1 - t * 0.9) * 0.5;
	float dz = cos(q.y * 2.1 - t * 1.1) * 0.5 + cos((q.x - q.y) * 1.3 + t * 0.7) * 0.5;
	float k = ripple_strength * 0.15 * ripple_amount;
	return normalize(vec3(dx * k, 1.0, dz * k));
}

// foam running down a falling face: lanes two voxels wide, each at its own speed, carrying long soft streaks
// (several metres, fading in and out) that are brightest down the lane's middle; a lane without a streak this
// moment stays plain water. Voxel-wide lanes with short bands read as a knitted chevron pattern on a still sheet
// (bug report 2026-10-25 21-01-01 "broken waterfall").
float fall_streak(vec2 wc) {
	float lx = wc.x * 0.5;
	float lane = floor(lx);
	float across = 1.0 - abs(fract(lx) * 2.0 - 1.0);   // 0 at the lane's edges, 1 in the middle
	float speed = fall_speed * (0.7 + 0.6 * lava_hash(vec3(lane, 1.0, 3.0)));
	float along = (wc.y - TIME * speed) * 0.03125 + lava_hash(vec3(lane, 7.0, 2.0));   // one period = 32 voxels (4 m)
	float v = fract(along);
	float streak = smoothstep(0.2, 0.5, v) * (1.0 - smoothstep(0.75, 1.0, v));   // a long tail, a soft head
	float present = step(0.35, lava_hash(vec3(lane, floor(along), 4.0)));           // some periods carry no foam
	float wisp = 0.6 + 0.4 * sin(wc.y * 1.7 + lane * 3.1);                           // texture along the streak
	return streak * present * wisp * smoothstep(0.0, 0.7, across) * (0.6 + 0.4 * lava_hash(vec3(lane, 4.0, 9.0)));
}

// bubbles on a 3-voxel grid: one in some cells, swelling to a ring and popping, each on its own clock
float bubble_ring(vec2 p) {
	vec2 cell = floor(p / 3.0);
	float h = lava_hash(vec3(cell, 5.0));
	float phase = fract(TIME * (0.25 + 0.2 * h) + h * 7.0);
	vec2 c = (cell + 0.25 + 0.5 * vec2(h, lava_hash(vec3(cell, 9.0)))) * 3.0;
	float r = mix(0.15, 1.1, phase);
	float d = length(p - c);
	float ring = 1.0 - smoothstep(0.0, 0.22, abs(d - r));
	float dome = (1.0 - smoothstep(0.0, r, d)) * 0.35;
	return (ring + dome) * step(0.45, h) * (1.0 - smoothstep(0.8, 1.0, phase));
}

// lava_color (terrain_common) sampled smoothly: crust plates a couple of metres across drifting and
// churning, thin glowing cracks between them, the crust itself warm in places. `q` in voxels.
uniform float lava_crack = 0.07;          // crack half-width in plate-noise units (smaller = thinner)
uniform vec3 lava_crust_dark : source_color = vec3(0.10, 0.035, 0.025);
uniform vec3 lava_crust_warm : source_color = vec3(0.34, 0.09, 0.035);
vec3 lava_surface(vec3 q, float t, out vec3 emission) {
	float plates = lava_noise(q * 0.24 + vec3(t * 0.04, t * 0.015, -t * 0.03));
	float plates2 = lava_noise(q * 0.55 + vec3(-t * 0.05, 0.0, t * 0.04));
	float wob = (lava_noise(q * 1.3) - 0.5) * 0.05;   // ragged crack edges
	float crack = 1.0 - smoothstep(lava_crack * 0.3, lava_crack, abs(plates - 0.5 + wob));
	crack = max(crack, (1.0 - smoothstep(lava_crack * 0.15, lava_crack * 0.5, abs(plates2 - 0.5 + wob))) * 0.45);
	float churn = lava_noise(q * 0.45 + vec3(0.0, t * 0.15, t * 0.09));
	float warm = smoothstep(0.68, 0.95, lava_noise(q * 0.3 + vec3(t * 0.02, 3.0, 0.0)));
	float heat = clamp(crack * (0.75 + 0.45 * churn) + warm * 0.08, 0.0, 1.0);
	vec3 crust = mix(lava_crust_dark, lava_crust_warm, clamp((plates2 - 0.3) * 1.2 + warm * 0.5, 0.0, 1.0));
	vec3 hot = mix(vec3(0.95, 0.26, 0.03), vec3(1.0, 0.62, 0.16), churn);
	emission = hot * heat * heat * lava_glow * 0.7 + crust * 0.25;
	return mix(crust, hot, heat);
}

void fragment() {
	float hz = horizon_mix(v_world, CAMERA_POSITION_WORLD);
	if (hz >= 1.0) {
		discard;
	}
	vec3 n = v_normal_world;
	vec3 an = abs(n);
	bool top = an.y > 0.5;
	// the two world axes across the face; -y so the surface runs downwards on a side
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
	// tops sample in world space: each cell's own flow shift left seams between cells flowing different
	// ways (the patterns drift on their own clocks); the flow still moves the ripples
	vec2 p = top ? wc : wc - shift;
	float tint = face_tint(n);
	ROUGHNESS = 1.0;
	METALLIC = 0.0;
	SPECULAR = 0.0;
	// ADR 0069 A3: a steep top (a cascade: the height field's sheet over stair-stepped ground) is a
	// falling sheet, by the geometric slope of the triangle
	float steep = 0.0;
	if (top) {
		vec3 gn = normalize(cross(dFdx(v_world), dFdy(v_world)));
		steep = smoothstep(0.85, 0.55, abs(gn.y));
		NORMAL = normalize((VIEW_MATRIX * vec4(liquid_ripple(v_world.xz + shift), 0.0)).xyz);
		// the ripple normal points up, so a cascade seen head on is a grazing mirror of the bright sky: a
		// white wall, worst far away (bug 2026-10-27_15-15-44). A falling sheet does not gloss
		SPECULAR = surface_specular * (1.0 - steep);
		ROUGHNESS = mix(surface_roughness, 1.0, steep);
	}
	float m = mottle > 0.0 ? (lava_noise(vec3(p * 0.18, TIME * 0.05)) - 0.5) * mottle : 0.0;
	if (abs(fluid_tile - lava_tile) < 0.5) {
		// lava: crust plates and glowing cracks (lava_color), now sampled smoothly and drifting with the flow
		// instead of one colour per voxel; a falling face pours bright streaks
		vec3 glow_c;
		// one field in world space: tops slicing the noise at their own height cut dug, uneven lava into a
		// mosaic of mismatched plates (bug 2026-10-27_15-45-02); every top now shares one plane
		vec3 q = top ? vec3(v_world.x, 0.0, v_world.z) : v_world;
		vec3 c = lava_surface(q, TIME, glow_c);
		if (!top) {
			float s = fall_streak(wc);
			c = mix(c, foam_color, s * foam);
			glow_c += foam_color * s * foam * lava_glow * 0.6;
		}
		float heat = clamp(dot(glow_c, vec3(0.33)) / lava_glow, 0.0, 1.0);
		ALBEDO = c * vcol.rgb * tint * (1.0 + m);
		EMISSION = glow_c;
		ROUGHNESS = top ? mix(mix(surface_roughness, 1.0, heat), 1.0, steep) : 1.0;   // the crust shines, the cracks glow
		SPECULAR = top ? surface_specular * (1.0 - heat) * (1.0 - steep) : 0.0;
	} else {
		vec3 col = liquid_shallow;
		float a = liquid_alpha;
#ifdef FLUID_SEE_THROUGH
		float facing = abs(dot(normalize(VIEW), NORMAL));
		float zb = texture(depth_tex, SCREEN_UV).r;
		float d_m = 1000.0;   // sky behind: as deep as it gets
		if (zb > 0.0) {
			vec4 vb = INV_PROJECTION_MATRIX * vec4(SCREEN_UV * 2.0 - 1.0, zb, 1.0);
			d_m = max(length(vb.xyz / vb.w) - length(VERTEX), 0.0) * metres_per_unit;
		}
		if (!top) {
			d_m = min(d_m, sheet_m);   // a falling face is a thin sheet (a waterfall, a pour): never the deep colour
		}
		// the pale rim is a shore, where a flat top thins out: a cascade or a fall hugs its rock the whole
		// way down, and the rim turned it into a white wall (bug 2026-10-27_15-15-44)
		float rim_k = (1.0 - smoothstep(0.0, rim_m, d_m)) * rim * (top ? 1.0 - steep : 0.0);
		col = mix(liquid_shallow, liquid_deep, 1.0 - exp(-d_m / depth_m)) * (1.0 + m);
		col = mix(col, rim_color, rim_k);
		a = clamp(mix(alpha_min, alpha_max, 1.0 - exp(-d_m / clear_m)) + (1.0 - facing) * edge_alpha_boost + rim_k, 0.0, 1.0);
		if (top && sheen > 0.0) {
			// a thin oil film: the hue turns with the film's thickness (drifting noise) and the view angle
			float film = lava_noise(vec3(p * 0.12, TIME * 0.05)) * 1.6 + (1.0 - facing) * 0.8;
			vec3 rainbow = 0.5 + 0.5 * cos(6.28318 * (film + vec3(0.0, 0.33, 0.67)));
			float slick = smoothstep(0.45, 0.75, lava_noise(vec3(p * 0.08 + 11.0, TIME * 0.03)));   // the film is in slicks
			col += rainbow * sheen * slick * (0.3 + 0.7 * (1.0 - facing));
		}
		if (top && bubbles > 0.0) {
			float b = bubble_ring(p);
			col = mix(col, rim_color, clamp(b * bubbles, 0.0, 1.0));
			a = max(a, b * bubbles);
		}
		if (top) {
			// a cascade's depth to the ground swings between nothing and three cells every column, and the
			// thickness alpha drew that as stripes of clear water beside opaque panels: give it the side faces'
			// floor and a trace of foam (more foam washed it white)
			col = mix(col, foam_color, steep * foam * 0.12);
			a = mix(a, clamp(max(a, steep_alpha), 0.0, 1.0), steep);
		}
		if (an.y < 0.5) {
			float s = fall_streak(wc);
			col = mix(col, foam_color, s * foam);
			a = clamp(max(a, 0.55) + s * 0.35, 0.0, 1.0);
		}
		ALPHA = a * (1.0 - hz);   // the horizon: the terrain behind blends into the sky (ADR 0073)
#endif
		ALBEDO = col * tint;
		EMISSION = col * glow;
	}
	if (hz > 0.0) {
		// the horizon: lit colour out, the sky behind in (ADR 0073)
		ALBEDO *= 1.0 - hz;
		EMISSION = mix(EMISSION, horizon_sky(v_world, CAMERA_POSITION_WORLD, SCREEN_UV), hz);
	}
}
