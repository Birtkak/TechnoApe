class_name MonolithBeacon
extends MeshInstance3D

## The Gelvin monolith's sky beam (ADR 0098): a 2 km magenta pillar of light over the seam, seen from
## anywhere, so the spawn can always be found. The camera draws only ~60 m (ADR 0073), so past NEAR the
## beam is scaled toward the camera by NEAR / distance, around the camera: the picture is the same, the
## mesh stays inside the far plane. Its width stops shrinking at MIN_WIDTH_M as seen from NEAR, so a
## far beam is still a visible line.

const HEIGHT_M := 2000.0
const RADIUS_M := 1.2
const NEAR_M := 40.0
const MIN_WIDTH_M := 0.35

const SHADER := """
shader_type spatial;
render_mode unshaded, blend_add, cull_disabled, depth_draw_never, fog_disabled, shadows_disabled;
uniform vec3 colour = vec3(1.0, 0.22, 0.85);
uniform float height = 1.0;
varying float v_up;
void vertex() {
	v_up = VERTEX.y / height + 0.5;   // 0 at the base, 1 at the top
}
void fragment() {
	float core = pow(abs(dot(normalize(NORMAL), normalize(VIEW))), 3.0);   // bright down the middle, soft edges
	float fade = smoothstep(0.0, 0.004, v_up) * pow(1.0 - v_up, 2.0);
	float bands = 0.8 + 0.2 * sin(v_up * 900.0 - TIME * 2.5);   // light rising up the beam
	ALBEDO = colour * 1.6 * core * fade * bands;
}
"""

var base := Vector3.ZERO   # world position of the beam's foot
var camera: Camera3D


func _init(p_base: Vector3, p_camera: Camera3D) -> void:
	base = p_base
	camera = p_camera
	var h := Settings.m2v(HEIGHT_M)
	var m := CylinderMesh.new()
	m.top_radius = Settings.m2v(RADIUS_M)
	m.bottom_radius = Settings.m2v(RADIUS_M)
	m.height = h
	m.radial_segments = 12
	m.rings = 1
	var sm := ShaderMaterial.new()
	sm.shader = Shader.new()
	sm.shader.code = SHADER
	sm.set_shader_parameter("height", h)
	m.material = sm
	mesh = m
	cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	extra_cull_margin = h


func _process(_delta: float) -> void:
	if camera == null:
		return
	var cam := camera.global_position
	var foot := base + Vector3(0, Settings.m2v(HEIGHT_M) * 0.5, 0)   # the cylinder's centre
	var d := Vector2(base.x - cam.x, base.z - cam.z).length()
	var near := Settings.m2v(NEAR_M)
	var th := 1.0 if d <= near else near / d   # the width as if only the horizontal distance were pulled in
	var sxz := maxf(th, MIN_WIDTH_M / (RADIUS_M * 2.0))
	# pull the whole beam (top included) inside NEAR: the far plane clips by view depth, so a beam only
	# pulled in sideways lost its upper part when looked at straight on and shrank to a dot
	var top := base + Vector3(0, Settings.m2v(HEIGHT_M), 0)
	var t := minf(1.0, near / maxf(cam.distance_to(base), cam.distance_to(top)))
	position = cam + (foot - cam) * t
	scale = Vector3(sxz * t / th, t, sxz * t / th)
