shader_type spatial;
render_mode cull_disabled, depth_draw_opaque, specular_disabled, vertex_lighting;

// Draw pass of the Minecraft-style particles (ADR 0026): a camera-facing quad
// with nearest-filtered pixel art cut out at alpha 0.5, so there is nothing to
// sort. chips: a 4x4 pixel square of a terrain atlas tile (CUSTOM.x = tile +
// square / 16). sprites: a row of the particle sheet (CUSTOM.x = row * 8 +
// frames - 1), its frames played over the particle's life. Lit as a voxel top
// face, so particles darken at night and in shade; glowing motions add emission.
// Lit per vertex: a waterfall's mist covers the screen many times over, and
// per-pixel lighting of that overdraw cost milliseconds (ADR 0026).

uniform sampler2D tex : source_color, filter_nearest;
uniform vec2 grid = vec2(8.0, 10.0);   // tiles (chips) or 8 px cells (sprites) across and down
uniform bool chips = true;
uniform vec4 motion_b[16];             // glow, size at the end of life (x start)

varying float v_glow;

void vertex() {
	int m = int(INSTANCE_CUSTOM.z / 100.0);
	float life = max(mod(INSTANCE_CUSTOM.z, 100.0), 0.001);
	float t = clamp(INSTANCE_CUSTOM.y / life, 0.0, 1.0);
	vec4 b = motion_b[m];
	float s = length(MODEL_MATRIX[0].xyz) * mix(1.0, b.y, t);
	mat4 world = mat4(vec4(normalize(INV_VIEW_MATRIX[0].xyz) * s, 0.0), vec4(normalize(INV_VIEW_MATRIX[1].xyz) * s, 0.0),
			vec4(normalize(INV_VIEW_MATRIX[2].xyz) * s, 0.0), MODEL_MATRIX[3]);
	MODELVIEW_MATRIX = VIEW_MATRIX * world;
	MODELVIEW_NORMAL_MATRIX = mat3(MODELVIEW_MATRIX);
	v_glow = b.x;
	// mostly world up (a voxel top face), a little toward the camera so low sun still reaches them
	NORMAL = normalize((INV_VIEW_MATRIX * vec4(0.0, 0.0, 1.0, 0.0)).xyz * 0.25 + vec3(0.0, 0.75, 0.0));
	NORMAL = (inverse(world) * vec4(NORMAL, 0.0)).xyz;
	vec2 cell = 1.0 / grid;
	float code = INSTANCE_CUSTOM.x;
	vec2 uv = UV * 0.96 + 0.02;   // stay off the neighbouring texel at the quad's edge
	if (chips) {
		float tile = floor(code);
		float sq = floor(fract(code) * 16.0 + 0.5);
		vec2 origin = vec2(mod(tile, grid.x), floor(tile / grid.x)) * cell;
		UV = origin + (vec2(mod(sq, 4.0), floor(sq / 4.0)) + uv) * 0.25 * cell;
	} else {
		float row = floor(code / 8.0);
		float frames = mod(code, 8.0) + 1.0;
		float f = min(floor(t * frames), frames - 1.0);
		UV = (vec2(f, row) + uv) * cell;
	}
}

void fragment() {
	vec4 c = texture(tex, UV);
	if (c.a < 0.5) {
		discard;
	}
	vec3 col = c.rgb * COLOR.rgb;
	ALBEDO = col;
	EMISSION = col * v_glow;
}
