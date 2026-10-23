#[compute]
#version 450

// Near field (ADR 0012): scatters 16^3-cell chunks (BrickStore.get_chunk_bytes layout:
// y fastest, then x, then z) into the toroidal volume texture, one workgroup per
// chunk, 64 threads with 64 cells each, and writes the occupancy of the chunk's
// eight 8^3 bricks (1 = any non-air cell) so the ray caster can stride over
// empty bricks. The CPU (NearFieldFeed) only concatenates raw chunk bytes; the
// transposition into z slices happens here. Chunk coords are world chunks: the
// texel is chunk * 16 & (n - 1), exactly what the caster computes.

layout(local_size_x = 64) in;
layout(r8, set = 0, binding = 0) uniform restrict writeonly image2DArray vol;
layout(r8, set = 0, binding = 1) uniform restrict writeonly image2DArray occ;
layout(std430, set = 0, binding = 2) restrict readonly buffer Bytes { uint words[]; } bytes;
layout(std430, set = 0, binding = 3) restrict readonly buffer Chunks { ivec4 c[]; } chunks;
layout(push_constant, std430) uniform Params {
	int n;
	int count;
	int pad0;
	int pad1;
} p;

shared uint s_occ[8];

void main() {
	uint chunk = gl_WorkGroupID.x;
	if (chunk >= uint(p.count)) return;
	uint lid = gl_LocalInvocationID.x;
	if (lid < 8u) s_occ[lid] = 0u;
	barrier();
	ivec3 base = (chunks.c[chunk].xyz * 16) & (p.n - 1); // toroidal: world chunk coords wrap into the texture
	uint any_lo = 0u;   // bricks of this thread's cells: y in 0..7 and 8..15 (other axes fixed per thread)
	uint any_hi = 0u;
	int bx = 0;
	int bz = 0;
	for (uint i = lid * 64u; i < lid * 64u + 64u; i++) {
		uint bi = chunk * 4096u + i;
		uint b = (bytes.words[bi >> 2] >> ((bi & 3u) * 8u)) & 255u;
		int y = int(i & 15u);
		int x = int((i >> 4) & 15u);
		int z = int(i >> 8);
		bx = x >> 3;
		bz = z >> 3;
		imageStore(vol, base + ivec3(x, y, z), vec4(float(b) / 255.0, 0.0, 0.0, 0.0));
		if (b != 0u) {
			if (y < 8) any_lo = 1u; else any_hi = 1u;
		}
	}
	if (any_lo != 0u) atomicOr(s_occ[bx + 2 * (0 + 2 * bz)], 1u);
	if (any_hi != 0u) atomicOr(s_occ[bx + 2 * (1 + 2 * bz)], 1u);
	barrier();
	if (lid < 8u) {
		ivec3 bb = (chunks.c[chunk].xyz * 2 + ivec3(int(lid & 1u), int((lid >> 1u) & 1u), int(lid >> 2u))) & ((p.n >> 3) - 1);
		imageStore(occ, bb, vec4(s_occ[lid] != 0u ? 1.0 : 0.0, 0.0, 0.0, 0.0));
	}
}
