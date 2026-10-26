shader_type sky;
render_mode use_half_res_pass;

// Procedural sky for the game (scripts/visuals/atmosphere.gd drives the
// uniforms from scripts/world/calendar.gd, ADR 0025): a three-stop gradient
// (zenith, horizon, ground), the sun's disc and halo, a moon with its real
// phase (lit from the sun's direction, so crescents point the right way) and
// maria, the real stars and Milky Way turning with sidereal time, optional
// constellation lines, eclipses, and a scrolling layer of value-noise clouds.
// Half-resolution pass: nothing here needs per-pixel shading.

uniform float sun_radius = 0.042;        // radians (about 8x the real 0.27 deg: a game sun)
uniform float sun_halo = 0.45;
uniform float moon_radius = 0.038;       // radians at mean distance; scaled by moon_size
uniform float moon_size = 1.0;           // perigee cycle, harvest / hunter's moon (Calendar.moon_size)
uniform vec3 moon_tint : source_color = vec3(1.0, 0.98, 0.94);
uniform float cloud_coverage : hint_range(0.0, 1.0) = 0.42;
uniform float cloud_softness : hint_range(0.01, 0.5) = 0.18;
uniform float cloud_scale = 2.2;
uniform float cloud_height = 0.28;       // apparent altitude: smaller = clouds flatter towards the horizon
uniform vec2 cloud_wind = vec2(0.006, 0.002);
uniform float cloud_alpha = 0.92;
uniform vec3 moon_dir = vec3(-0.3, 0.5, -0.4);  // where the moon is drawn (eclipses move it onto the sun)
uniform vec3 pole_dir = vec3(0.0, 0.707, -0.707); // north celestial pole (Polaris), for the moon's orientation
uniform mat3 world_to_sky = mat3(1.0);           // world direction -> equatorial (x RA 0, y RA 90, z pole)
uniform sampler2D star_map : hint_default_black, filter_linear, repeat_enable;
uniform sampler2D milky_way : hint_default_black, filter_linear, repeat_enable;
uniform float star_brightness = 2.2;
uniform float milky_way_brightness = 0.10;
uniform float constellation_lines = 0.0;  // 0 hidden .. 1 shown (/constellations)
// The far layer (ADR 0074): the far view's texture, transparent where there is no far terrain, laid
// over the sky at the pixel (far_layer.gd sets it on the sky and on the terrain materials).
uniform sampler2D far_tex : hint_default_transparent, filter_linear;
uniform bool far_on = false;
uniform float solar_eclipse = 0.0;        // 1 while an eclipse runs: the moon's dark disc covers the sun
uniform vec2 lunar_shadow = vec2(9.0, 0.0); // umbra centre in moon radii, in the moon's disc frame
uniform float lunar_umbra = 2.7;            // umbra radius in moon radii

#include "res://assets/shaders/sky_common.gdshaderinc"

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

// --- the moon's face -------------------------------------------------------------

// Albedo at a point of the disc (x west, y north, as a northern observer sees it):
// the large maria roughly where they are, plus Tycho's bright rays.
float moon_albedo(vec2 p) {
	float a = 1.0;
	a -= 0.30 * smoothstep(0.16, 0.0, length(p - vec2(0.66, 0.30)));   // Crisium
	a -= 0.28 * smoothstep(0.24, 0.0, length(p - vec2(0.34, 0.05)));   // Tranquillitatis
	a -= 0.28 * smoothstep(0.17, 0.0, length(p - vec2(0.27, 0.36)));   // Serenitatis
	a -= 0.26 * smoothstep(0.30, 0.0, length(p - vec2(-0.22, 0.46)));  // Imbrium
	a -= 0.24 * smoothstep(0.40, 0.0, length(p - vec2(-0.58, 0.08)));  // Procellarum
	a -= 0.22 * smoothstep(0.20, 0.0, length(p - vec2(-0.16, -0.33))); // Nubium
	a -= 0.22 * smoothstep(0.16, 0.0, length(p - vec2(0.58, -0.16)));  // Fecunditatis
	a -= 0.20 * smoothstep(0.11, 0.0, length(p - vec2(0.42, -0.30)));  // Nectaris
	a -= 0.14 * smoothstep(0.08, 0.0, abs(p.y - 0.72)) * step(-0.5, p.x) * step(p.x, 0.45); // Frigoris
	vec2 t = p - vec2(-0.10, -0.74);                                        // Tycho and its rays
	a += 0.25 * smoothstep(0.05, 0.0, length(t));
	float ray = pow(abs(sin(atan(t.y, t.x) * 7.0)), 30.0) * smoothstep(0.9, 0.05, length(t));
	a += 0.10 * ray;
	a *= 0.9 + 0.2 * vnoise(p * 9.0);
	a = 1.0 - (1.0 - a) * 1.6;   // the maria read from the ground: stronger contrast
	return clamp(a, 0.3, 1.25);
}

// --- sky ----------------------------------------------------------------------
// The gradient itself (SkyLight, sky_light, gradient) is in sky_common.gdshaderinc, shared with the
// terrain materials that blend into it at the horizon (ADR 0073).
// Two passes: the half-resolution pass draws the gradient and the clouds (the
// expensive fbm) and writes the cloud cover to ALPHA; the full-resolution pass
// adds the sharp things (stars, Milky Way, sun, moon) behind those clouds:
// final = half + (sharp - gradient) * (1 - cloud cover). The radiance cubemap
// (ambient light) gets the whole sky at once.

// rgb = cloud colour, a = cover
vec4 clouds(vec3 dir, SkyLight l, float time) {
	float h = dir.y;
	if (h <= 0.01) {
		return vec4(0.0);
	}
	vec3 sun = normalize(sun_dir);
	vec2 cp = dir.xz / (dir.y + cloud_height) * cloud_scale + time * cloud_wind;
	float n = fbm(cp);
	float cover = smoothstep(1.0 - cloud_coverage - cloud_softness, 1.0 - cloud_coverage + cloud_softness, n);
	float horizon_fade = smoothstep(0.01, 0.16, h);
	float a = cover * cloud_alpha * horizon_fade;
	// thicker where the noise is higher: the core is darker, the rim lit
	float thick = smoothstep(1.0 - cloud_coverage, 1.0, n);
	vec3 lit = mix(vec3(1.0), sunset_color, l.dusk * 0.8) * (0.35 + 0.65 * l.day);
	vec3 shade = mix(l.hor, l.zen, 0.5) * (0.55 + 0.45 * l.day);
	vec3 ccol = mix(lit, shade * 0.9, thick * 0.75);
	ccol += lit * pow(clamp(dot(dir, sun), 0.0, 1.0), 10.0) * 0.6 * (1.0 - thick);   // rim towards the sun
	// by night the clouds are dark, faintly lit by the moon
	vec3 moon = normalize(moon_dir);
	float moonlit = clamp(-dot(moon, sun) * 0.5 + 0.5, 0.0, 1.0) * smoothstep(-0.02, 0.2, moon.y);
	ccol = mix(ccol * (0.06 + 0.10 * moonlit) + vec3(0.012, 0.015, 0.025), ccol, l.day);
	return vec4(ccol, a);
}

// The sharp layer over the gradient: stars, Milky Way, sun, moon.
vec3 celestial(vec3 dir, vec3 col, SkyLight l, float time) {
	vec3 sun = normalize(sun_dir);
	float sun_up = clamp(sun.y, -1.0, 1.0);
	float h = dir.y;
	float day = l.day;
	float cos_sun = dot(dir, sun);

	// the moon's disc: where is this pixel on it?
	vec3 moon = normalize(moon_dir);
	float mr = moon_radius * moon_size;
	vec3 east = normalize(cross(pole_dir, moon) + vec3(1e-5, 0.0, 0.0));
	vec3 north = cross(moon, east);
	vec3 off = dir - moon * dot(dir, moon);
	vec2 mp = vec2(dot(off, -east), dot(off, north)) / sin(mr);   // x west, y north, in moon radii
	float in_moon = (dot(dir, moon) > 0.0) ? 1.0 - smoothstep(0.96, 1.0, length(mp)) : 0.0;
	float moon_vis = smoothstep(-0.02, 0.03, moon.y);

	// night sky: Milky Way and stars, dimmed by daylight, the horizon and the moon's disc
	if (h > -0.02 && day < 0.98) {
		vec3 c = world_to_sky * dir;
		vec2 uv = vec2(fract(atan(c.y, c.x) / TAU), acos(clamp(c.z, -1.0, 1.0)) / PI);
		vec4 s = texture(star_map, uv);
		// stars come out through nautical twilight (sun 3 to 12 deg down), not at sunset
		// (and in a total eclipse's darkness, the brightest ones)
		float dark = max(smoothstep(-0.05, -0.2, sun_up), pow(eclipse_dark, 6.0) * 0.5);
		float night = (1.0 - day) * dark * smoothstep(-0.02, 0.25, h) * (1.0 - in_moon * moon_vis);
		// the moonlit sky drowns the faint stars and the Milky Way
		float moonlight = clamp(-dot(moon, sun) * 0.5 + 0.5, 0.0, 1.0) * moon_vis * (1.0 - day);
		float faint = 1.0 - 0.6 * moonlight;
		float mw = texture(milky_way, uv).r;
		col += vec3(0.80, 0.86, 1.0) * mw * mw * milky_way_brightness * night * faint;
		float twinkle = 0.75 + 0.25 * sin(time * 3.0 + hash12(floor(uv * 1024.0)) * 60.0) * (1.0 - smoothstep(0.0, 0.5, h) * 0.6);
		vec3 star = s.rgb * s.rgb;
		col += star * star_brightness * twinkle * night * mix(faint, 1.0, smoothstep(0.1, 0.4, max(star.r, max(star.g, star.b))));
		col += vec3(0.25, 0.45, 0.85) * s.a * 0.35 * constellation_lines * (1.0 - day) * smoothstep(-0.02, 0.1, h);
	}

	// sun disc + halo; during a solar eclipse the moon's dark disc covers it and the corona shows
	if (h > -0.05) {
		float sang = acos(clamp(cos_sun, -1.0, 1.0));
		float disc = 1.0 - smoothstep(sun_radius * 0.9, sun_radius, sang);
		float halo = pow(clamp(cos_sun, 0.0, 1.0), 48.0) * sun_halo;
		vec3 sun_col = mix(sunset_color, vec3(1.0, 0.97, 0.9), smoothstep(0.0, 0.3, sun_up));
		float sun_vis = smoothstep(-0.06, 0.02, sun_up);
		float covered = solar_eclipse * in_moon;
		col += sun_col * (disc * 6.0 * (1.0 - covered) + halo * (1.0 - eclipse_dark)) * sun_vis;
		// corona: a soft, streaky white ring that only shows near totality
		float rr = sang / sun_radius;
		vec3 sx = normalize(cross(sun, vec3(0.0, 1.0, 0.0)) + vec3(1e-5));
		vec3 sy = cross(sx, sun);
		float streak = 0.6 + 0.4 * vnoise(vec2(atan(dot(dir, sy), dot(dir, sx)) * 6.0, 0.0));
		float corona = smoothstep(0.9, 1.05, rr) * exp(-(rr - 1.0) * 2.2) * streak;
		col += vec3(0.95, 0.97, 1.0) * corona * 1.6 * smoothstep(0.85, 1.0, eclipse_dark) * sun_vis;
	}

	// the moon: lit where its surface faces the sun, faint earthshine on the rest
	if (in_moon > 0.0 && moon_vis > 0.0) {
		float z = sqrt(max(0.0, 1.0 - dot(mp, mp)));
		vec3 n = normalize(-east * mp.x + north * mp.y - moon * z);   // surface normal (towards us at the centre)
		float lit = dot(n, sun);
		float light = smoothstep(-0.03, 0.06, lit) * pow(max(lit, 0.0), 0.25);
		float alb = moon_albedo(mp);
		vec3 mcol = moon_tint * alb * light * 1.35;
		// lunar eclipse: the earth's umbra, deep copper-red inside
		float umbra = 1.0 - smoothstep(lunar_umbra - 0.12, lunar_umbra + 0.12, length(mp - lunar_shadow));
		mcol = mix(mcol, vec3(0.42, 0.11, 0.05) * alb * 0.35, umbra);
		float earthshine = 0.035 * (1.0 - light) * (1.0 - day);
		// low moons are warmer (the same air that reddens sunsets)
		mcol *= mix(vec3(1.0, 0.72, 0.45), vec3(1.0), smoothstep(0.0, 0.25, moon.y));
		vec3 disc_col = mcol + vec3(0.55, 0.6, 0.7) * alb * earthshine;
		// by day only the lit part shows (the dark part is the sky behind it); in a solar
		// eclipse the new moon is seen only where it blocks the sun (above)
		col = mix(col, max(col, disc_col), in_moon * moon_vis * (1.0 - solar_eclipse));
	}
	return col;
}

void sky() {
	vec3 dir = EYEDIR;
	SkyLight l = sky_light();
	if (AT_CUBEMAP_PASS) {
		vec3 g = gradient(dir, l);
		vec4 cl = clouds(dir, l, TIME);
		COLOR = mix(celestial(dir, g, l, TIME), cl.rgb, cl.a) * exposure;
	} else if (AT_HALF_RES_PASS) {
		vec4 cl = clouds(dir, l, TIME);
		COLOR = mix(gradient(dir, l), cl.rgb, cl.a) * exposure;
		ALPHA = cl.a;
	} else {
		vec3 g = gradient(dir, l);
		vec3 sharp = celestial(dir, g, l, TIME);
		COLOR = HALF_RES_COLOR.rgb + (sharp - g) * (1.0 - HALF_RES_COLOR.a) * exposure;
		if (far_on) {
			vec4 f = texture(far_tex, SCREEN_UV);
			COLOR = mix(COLOR, f.rgb, f.a);
		}
	}
}
