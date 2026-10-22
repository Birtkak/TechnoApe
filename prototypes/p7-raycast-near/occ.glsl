#[compute]
#version 450

// Brick occupancy: one byte per 8x8x8 brick of the volume (1 = any solid cell),
// so the ray caster can skip empty bricks in 8-cell strides. Dispatched over
// the whole volume at start and over the dirty brick layers after each upload.

layout(local_size_x = 8, local_size_y = 8, local_size_z = 8) in;
layout(r8, set = 0, binding = 0) uniform restrict readonly image2DArray vol;
layout(r8, set = 0, binding = 1) uniform restrict writeonly image2DArray occ;
layout(push_constant, std430) uniform Params {
	int nb;        // bricks per edge
	int z_offset;  // first brick layer of this dispatch
	int pad0;
	int pad1;
} p;

void main() {
	ivec3 b = ivec3(gl_GlobalInvocationID) + ivec3(0, 0, p.z_offset);
	if (any(greaterThanEqual(b, ivec3(p.nb)))) return;
	ivec3 base = b * 8;
	float any_solid = 0.0;
	for (int z = 0; z < 8 && any_solid == 0.0; z++)
		for (int y = 0; y < 8 && any_solid == 0.0; y++)
			for (int x = 0; x < 8; x++)
				if (imageLoad(vol, base + ivec3(x, y, z)).r > 0.001) { any_solid = 1.0; break; }
	imageStore(occ, b, vec4(any_solid, 0.0, 0.0, 0.0));
}
