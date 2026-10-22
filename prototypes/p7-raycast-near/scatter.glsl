#[compute]
#version 450

// Scatters dirty 16^3 simulation chunks (SimWorld.get_chunk_bytes layout:
// y fastest, then x, then z) into the ray-cast volume texture. One workgroup
// per chunk, 64 threads, 64 cells each. The CPU only concatenates the raw
// chunk bytes; the transposition into z slices happens here.

layout(local_size_x = 64) in;
layout(r8, set = 0, binding = 0) uniform restrict writeonly image2DArray vol;
layout(std430, set = 0, binding = 1) restrict readonly buffer Bytes { uint words[]; } bytes;
layout(std430, set = 0, binding = 2) restrict readonly buffer Chunks { ivec4 c[]; } chunks;
layout(push_constant, std430) uniform Params {
	int n;
	int count;
	int pad0;
	int pad1;
} p;

void main() {
	uint chunk = gl_WorkGroupID.x;
	if (chunk >= uint(p.count)) return;
	ivec3 base = (chunks.c[chunk].xyz * 16) & (p.n - 1); // toroidal: world chunk coords wrap into the texture
	for (uint i = gl_LocalInvocationID.x; i < 4096u; i += 64u) {
		uint bi = chunk * 4096u + i;
		uint b = (bytes.words[bi >> 2] >> ((bi & 3u) * 8u)) & 255u;
		int y = int(i & 15u);
		int x = int((i >> 4) & 15u);
		int z = int(i >> 8);
		imageStore(vol, base + ivec3(x, y, z), vec4(float(b) / 255.0, 0.0, 0.0, 0.0));
	}
}
