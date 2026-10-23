shader_type particles;
render_mode disable_velocity, disable_force;

// Minecraft-style particles (ADR 0026, scripts/visuals/particle_fx.gd). Nothing is
// emitted automatically: every particle comes from GPUParticles3D.emit_particle
// with its whole state, TRANSFORM (position, size as scale), VELOCITY, COLOR and
// CUSTOM = (sprite code, age, lifetime + 100 * motion, floor y). The motions are
// data (data/particles.json), in voxels and seconds here.

uniform vec4 motion_a[16];   // gravity (down positive), drag (1/s), floor (0 none, 1 rest, 2 die), sway

void process() {
	int m = int(CUSTOM.z / 100.0);
	float life = mod(CUSTOM.z, 100.0);
	CUSTOM.y += DELTA;
	if (CUSTOM.y >= life) {
		ACTIVE = false;
	}
	vec4 a = motion_a[m];
	VELOCITY.y -= a.x * DELTA;
	VELOCITY *= exp(-a.y * DELTA);
	vec3 p = TRANSFORM[3].xyz + VELOCITY * DELTA;
	if (a.w > 0.0) {
		// leaves and spores drift sideways on a slow per-particle wave
		float ph = float(INDEX) * 1.7 + CUSTOM.y * 2.3;
		p.x += sin(ph) * a.w * DELTA;
		p.z += cos(ph * 0.8) * a.w * DELTA;
	}
	if (a.z > 0.5 && p.y < CUSTOM.w) {
		if (a.z > 1.5) {
			ACTIVE = false;
		}
		p.y = CUSTOM.w;
		VELOCITY.y = 0.0;
		VELOCITY.xz *= exp(-12.0 * DELTA);   // ground friction: chips skid and stop
	}
	TRANSFORM[3].xyz = p;
}
