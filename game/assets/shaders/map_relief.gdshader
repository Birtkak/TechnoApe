shader_type canvas_item;

// The explored map's tiles (WorldMap): shaded relief, lit from the north-west (Xaero / JourneyMap
// style). Pixel alpha byte: 0 not explored (left clear), 255 generated-terrain colour in RGB, else the
// real surface: R material id, G liquid depth (voxels), B | A << 8 height (voxels).

uniform sampler2D palette : filter_nearest;       // 256 x 1: each material's colour
uniform vec4 water_deep : source_color = vec4(0.05, 0.12, 0.28, 1.0);
uniform float relief = 1.0;                      // shading strength
uniform float px_voxels = 4.0;                   // voxels per map pixel

float alpha_byte(vec4 d) {
	return floor(d.a * 255.0 + 0.5);
}

float height_of(vec4 d) {
	return floor(d.b * 255.0 + 0.5) + 256.0 * alpha_byte(d);
}

// a neighbour's height, or `h` where it is not real surface (unexplored, generated, off the tile)
float neighbour(sampler2D tex, ivec2 p, ivec2 size, float h) {
	if (p.x < 0 || p.y < 0 || p.x >= size.x || p.y >= size.y) {
		return h;
	}
	vec4 d = texelFetch(tex, p, 0);
	float a = alpha_byte(d);
	return (a == 0.0 || a == 255.0) ? h : height_of(d);
}

void fragment() {
	ivec2 size = textureSize(TEXTURE, 0);
	ivec2 p = clamp(ivec2(UV * vec2(size)), ivec2(0), size - 1);
	vec4 d = texelFetch(TEXTURE, p, 0);
	float a = alpha_byte(d);
	if (a == 0.0) {
		discard;
	}
	if (a == 255.0) {
		COLOR = vec4(d.rgb, 1.0);
	} else {
		float h = height_of(d);
		int mat = int(floor(d.r * 255.0 + 0.5));
		vec3 c = texelFetch(palette, ivec2(mat, 0), 0).rgb;
		float depth = floor(d.g * 255.0 + 0.5);
		float shade = 1.0;
		if (depth > 0.0) {
			c = mix(c, water_deep.rgb, clamp(depth / 48.0, 0.0, 1.0) * 0.85);
		} else {
			float hx = neighbour(TEXTURE, p + ivec2(1, 0), size, h) - neighbour(TEXTURE, p - ivec2(1, 0), size, h);
			float hz = neighbour(TEXTURE, p + ivec2(0, 1), size, h) - neighbour(TEXTURE, p - ivec2(0, 1), size, h);
			vec3 n = normalize(vec3(-hx, 2.0 * px_voxels, -hz));
			vec3 l = normalize(vec3(-1.0, 1.5, -1.0));
			shade = mix(1.0, clamp(dot(n, l) / l.y, 0.45, 1.4), relief);
		}
		COLOR = vec4(c * shade, 1.0);
	}
}
