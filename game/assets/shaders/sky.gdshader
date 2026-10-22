shader_type sky;
render_mode use_half_res_pass;

// Procedural sky for the game (scripts/visuals/atmosphere.gd drives the
// uniforms): a three-stop gradient (zenith, horizon, ground), a sun disc with
// a soft halo that warms and flattens as the sun sets, a scrolling layer of
// value-noise clouds lit from the sun side, and stars plus a moon once the
// sun is below the horizon. Everything is a function of the sun direction, so
// a day/night cycle only needs the light to move. Half-resolution pass:
// clouds and stars are cheap but there is no reason to shade them per pixel.

uniform vec3 zenith_color : source_color = vec3(0.23, 0.45, 0.85);
uniform vec3 horizon_color : source_color = vec3(0.72, 0.82, 0.95);
uniform vec3 ground_color : source_color = vec3(0.32, 0.36, 0.42);
uniform vec3 sunset_color : source_color = vec3(1.0, 0.55, 0.25);
uniform vec3 night_zenith : source_color = vec3(0.012, 0.018, 0.045);
uniform vec3 night_horizon : source_color = vec3(0.05, 0.07, 0.12);
uniform float sun_size = 0.015;          // angular radius of the disc (radians-ish, in dot space)
uniform float sun_halo = 0.45;
uniform float cloud_coverage : hint_range(0.0, 1.0) = 0.42;
uniform float cloud_softness : hint_range(0.01, 0.5) = 0.18;
uniform float cloud_scale = 2.2;
uniform float cloud_height = 0.28;       // apparent altitude: smaller = clouds flatter towards the horizon
uniform vec2 cloud_wind = vec2(0.006, 0.002);
uniform float cloud_alpha = 0.92;
uniform float star_density = 0.996;
uniform float exposure = 1.0;
uniform vec3 sun_dir = vec3(0.3, 0.7, 0.4);   // true sun direction (the scene light becomes the moon at night)

// --- noise -------------------------------------------------------------------

float hash12(vec2 p) {
	vec3 p3 = fract(vec3(p.xyx) * 0.1031);
	p3 += dot(p3, p3.yzx + 33.33);
	return fract((p3.x + p3.y) * p3.z);
}

float vnoise(vec2 p) {
	vec2 i = floor(p);
	vec2 f = fract(p);
	f = f * f * (3.0 - 2.0 * f);
	float a = hash12(i);
	float b = hash12(i + vec2(1.0, 0.0));
	float c = hash12(i + vec2(0.0, 1.0));
	float d = hash12(i + vec2(1.0, 1.0));
	return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

float fbm(vec2 p) {
	float v = 0.0;
	float a = 0.5;
	mat2 rot = mat2(vec2(0.8, 0.6), vec2(-0.6, 0.8));
	for (int i = 0; i < 5; i++) {
		v += a * vnoise(p);
		p = rot * p * 2.03 + vec2(17.0, 9.0);
		a *= 0.5;
	}
	return v;
}

// --- sky ----------------------------------------------------------------------

void sky() {
	vec3 dir = EYEDIR;
	vec3 sun = normalize(sun_dir);
	float sun_up = clamp(sun.y, -1.0, 1.0);
	// daylight: 1 well above the horizon, 0 well below; dusk is the band between
	float day = smoothstep(-0.12, 0.18, sun_up);
	float dusk = (1.0 - smoothstep(0.0, 0.35, abs(sun_up))) * smoothstep(-0.25, 0.0, sun_up);

	// gradient
	float h = dir.y;
	vec3 zen = mix(night_zenith, zenith_color, day);
	vec3 hor = mix(night_horizon, horizon_color, day);
	hor = mix(hor, sunset_color, dusk * 0.6);
	float horizon_t = pow(1.0 - clamp(h, 0.0, 1.0), 3.5);
	vec3 col = mix(zen, hor, horizon_t);
	if (h < 0.0) {
		vec3 gnd = mix(ground_color * 0.25, ground_color, day);
		col = mix(hor, gnd, smoothstep(0.0, 0.12, -h));
	}
	// the sun side of the sky is warmer and brighter near the horizon
	float cos_sun = dot(dir, sun);
	float toward = pow(clamp(cos_sun, 0.0, 1.0), 4.0);
	col += mix(vec3(0.0), sunset_color, dusk) * toward * 0.9 * (1.0 - clamp(h, 0.0, 1.0));
	col += horizon_color * toward * 0.08 * day;

	// sun disc + halo (only above the ground plane)
	if (h > -0.05) {
		float disc = smoothstep(1.0 - sun_size, 1.0 - sun_size * 0.35, cos_sun);
		float halo = pow(clamp(cos_sun, 0.0, 1.0), 48.0) * sun_halo;
		vec3 sun_col = mix(sunset_color, vec3(1.0, 0.97, 0.9), smoothstep(0.0, 0.3, sun_up));
		float sun_vis = smoothstep(-0.06, 0.02, sun_up);
		col += sun_col * (disc * 6.0 + halo) * sun_vis;
		// moon: opposite the sun, dim and cool
		vec3 moon = -sun;
		float cos_moon = dot(dir, moon);
		float mdisc = smoothstep(1.0 - sun_size * 0.8, 1.0 - sun_size * 0.3, cos_moon);
		col += vec3(0.75, 0.8, 0.95) * mdisc * 1.6 * (1.0 - day) * smoothstep(-0.06, 0.02, moon.y);
	}

	// stars
	if (h > 0.0 && day < 0.95) {
		vec2 sp = dir.xz / (dir.y + 0.15) * 90.0;
		float s = hash12(floor(sp));
		float star = step(star_density, s) * (0.4 + 0.6 * hash12(floor(sp) + 7.0));
		float twinkle = 0.7 + 0.3 * sin(TIME * 2.0 + s * 60.0);
		col += vec3(star * twinkle) * (1.0 - day) * smoothstep(0.0, 0.25, h);
	}

	// clouds on a plane above the camera; fade at the horizon
	if (h > 0.01) {
		vec2 cp = dir.xz / (dir.y + cloud_height) * cloud_scale + TIME * cloud_wind;
		float n = fbm(cp);
		float cover = smoothstep(1.0 - cloud_coverage - cloud_softness, 1.0 - cloud_coverage + cloud_softness, n);
		float horizon_fade = smoothstep(0.01, 0.16, h);
		float a = cover * cloud_alpha * horizon_fade;
		// thicker where the noise is higher: the core is darker, the rim lit
		float thick = smoothstep(1.0 - cloud_coverage, 1.0, n);
		vec3 lit = mix(vec3(1.0), sunset_color, dusk * 0.8) * (0.35 + 0.65 * day);
		vec3 shade = mix(hor, zen, 0.5) * (0.55 + 0.45 * day);
		vec3 ccol = mix(lit, shade * 0.9, thick * 0.75);
		// rim towards the sun
		ccol += lit * pow(clamp(cos_sun, 0.0, 1.0), 10.0) * 0.6 * (1.0 - thick);
		ccol = mix(ccol * 0.12 + vec3(0.02, 0.025, 0.04), ccol, day);
		col = mix(col, ccol, a);
	}

	COLOR = col * exposure;
}
