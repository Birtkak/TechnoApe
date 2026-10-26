// The sky's gradient, shared by sky.gdshader (which draws it) and the terrain
// materials (which blend into it at the horizon, ADR 0073: the voxel world is
// one box of full-detail voxels and ends where the box does; from horizon_start
// its pixels take the colour the sky has behind them, so the edge is the sky,
// not a wall or a dither). scripts/visuals/atmosphere.gd sets these uniforms on
// the sky material and on every terrain material (World.horizon_materials).

uniform vec3 zenith_color : source_color = vec3(0.23, 0.45, 0.85);
uniform vec3 horizon_color : source_color = vec3(0.72, 0.82, 0.95);
uniform vec3 ground_color : source_color = vec3(0.32, 0.36, 0.42);
uniform vec3 sunset_color : source_color = vec3(1.0, 0.55, 0.25);
uniform vec3 night_zenith : source_color = vec3(0.006, 0.010, 0.026);
uniform vec3 night_horizon : source_color = vec3(0.035, 0.05, 0.09);
uniform vec3 sun_dir = vec3(0.3, 0.7, 0.4);    // true sun direction (the scene light becomes the moon at night)
uniform float eclipse_dark = 0.0;         // 0..1 how much of the sun is covered (darkens the sky)
uniform float exposure = 1.0;

struct SkyLight {
	float day;
	float dusk;
	vec3 zen;
	vec3 hor;
};

SkyLight sky_light() {
	SkyLight l;
	float sun_up = clamp(normalize(sun_dir).y, -1.0, 1.0);
	// daylight: 1 well above the horizon, 0 well below; dusk is the band between.
	// A total solar eclipse brings dusk at noon.
	l.day = smoothstep(-0.12, 0.18, sun_up) * (1.0 - 0.93 * eclipse_dark);
	l.dusk = (1.0 - smoothstep(0.0, 0.35, abs(sun_up))) * smoothstep(-0.25, 0.0, sun_up);
	l.zen = mix(night_zenith, zenith_color, l.day);
	l.hor = mix(night_horizon, horizon_color, l.day);
	l.hor = mix(l.hor, sunset_color, l.dusk * 0.6);
	return l;
}

// The sky's colour in direction `dir` (unit, world space), before clouds, sun, moon and stars.
vec3 gradient(vec3 dir, SkyLight l) {
	vec3 sun = normalize(sun_dir);
	float h = dir.y;
	vec3 hor = mix(l.hor, sunset_color * 0.5, eclipse_dark * 0.5 * smoothstep(0.1, 0.0, h)); // a sunset all round the horizon
	float horizon_t = pow(1.0 - clamp(h, 0.0, 1.0), 3.5);
	vec3 col = mix(l.zen, hor, horizon_t);
	if (h < 0.0) {
		vec3 gnd = mix(ground_color * 0.25, ground_color, l.day);
		// a wide band: the terrain's horizon blend (ADR 0073) meets this from a few metres up, so it is the haze colour
		col = mix(hor, gnd, smoothstep(0.0, 0.35, -h));
	}
	// the sun side of the sky is warmer and brighter near the horizon
	float toward = pow(clamp(dot(dir, sun), 0.0, 1.0), 4.0);
	col += mix(vec3(0.0), sunset_color, l.dusk) * toward * 0.9 * (1.0 - clamp(h, 0.0, 1.0));
	col += horizon_color * toward * 0.08 * l.day;
	return col;
}
