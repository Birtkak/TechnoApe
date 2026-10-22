#[compute]
#version 450

// Fills the near-field volume (an R8 2D-array texture, one layer per z slice)
// with a procedural scene: rolling ground with grass/dirt/stone layers, caves,
// water in the low parts, sand at the waterline and a few glowing ore cells.
// Material ids: 0 air, 1 stone, 2 dirt, 3 grass, 4 sand, 5 water, 6 glow.

layout(local_size_x = 8, local_size_y = 8, local_size_z = 8) in;
layout(r8, set = 0, binding = 0) uniform restrict writeonly image2DArray vol;
layout(push_constant, std430) uniform Params {
	int n;
	float seed;
	int pad0;
	int pad1;
} p;

float hash(vec3 q) {
	return fract(sin(dot(q, vec3(127.1, 311.7, 74.7))) * 43758.5453);
}

float vnoise(vec3 x) {
	vec3 i = floor(x);
	vec3 f = fract(x);
	f = f * f * (3.0 - 2.0 * f);
	return mix(
		mix(mix(hash(i), hash(i + vec3(1, 0, 0)), f.x), mix(hash(i + vec3(0, 1, 0)), hash(i + vec3(1, 1, 0)), f.x), f.y),
		mix(mix(hash(i + vec3(0, 0, 1)), hash(i + vec3(1, 0, 1)), f.x), mix(hash(i + vec3(0, 1, 1)), hash(i + vec3(1, 1, 1)), f.x), f.y),
		f.z);
}

void main() {
	ivec3 c = ivec3(gl_GlobalInvocationID);
	if (any(greaterThanEqual(c, ivec3(p.n)))) return;
	vec3 q = vec3(c) / float(p.n); // 0..1 inside the volume
	float ground = 0.45 + 0.14 * vnoise(vec3(q.x, 0.0, q.z) * 5.0 + p.seed) + 0.04 * vnoise(vec3(q.x, 7.0, q.z) * 19.0);
	int m = 0;
	if (q.y < ground) {
		float depth = ground - q.y;
		m = depth < 0.012 ? 3 : (depth < 0.05 ? 2 : 1);
		if (vnoise(q * 9.0 + 3.0) > 0.63 && depth > 0.05) m = 0;          // caves
		if (m == 1 && vnoise(q * 31.0 + 9.0) > 0.80) m = 6;                // glowing ore
	} else if (q.y < 0.40) {
		m = 5;                                                               // water fills the low parts
	}
	if (m == 3 && q.y < 0.425) m = 4;                                        // sand at the waterline
	imageStore(vol, c, vec4(float(m) / 255.0, 0.0, 0.0, 0.0));
}
