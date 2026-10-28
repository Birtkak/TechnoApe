class_name DeathScreen
extends CanvasLayer

## The Gelvin monolith rebuilds the apetion (ADR 0098, docs/design/lore.md). On death the view breaks into
## green voxels and a terminal reports the specimen lost; at the black point `respawn` runs, then
## the view at the bed resolves out of big voxels again ("rebuilt voxel by voxel"). ~6 s.

const SHADER := """
shader_type canvas_item;
uniform sampler2D screen_tex : hint_screen_texture, filter_nearest;
uniform float block = 1.0;   // size in pixels of the voxels the view breaks into
uniform float fade = 0.0;    // 0 = the view, 1 = black
void fragment() {
	vec2 px = SCREEN_PIXEL_SIZE * block;
	vec3 c = texture(screen_tex, (floor(SCREEN_UV / px) + 0.5) * px).rgb;
	float g = dot(c, vec3(0.3, 0.59, 0.11));
	c = mix(c, vec3(0.15, 1.0, 0.5) * g, clamp(block / 16.0, 0.0, 1.0));
	COLOR = vec4(c * (1.0 - fade), 1.0);
}
"""
const BLOCK_PX := 48.0
const CHARS_PER_S := 45.0
const BAR := 24

var playing := false
var _mat := ShaderMaterial.new()
var _label := Label.new()


func _ready() -> void:
	layer = 19   # over the HUD, under the loading cover (20) a far respawn brings up
	process_mode = Node.PROCESS_MODE_ALWAYS
	var rect := ColorRect.new()
	rect.set_anchors_preset(Control.PRESET_FULL_RECT)
	rect.mouse_filter = Control.MOUSE_FILTER_IGNORE
	var sh := Shader.new()
	sh.code = SHADER
	_mat.shader = sh
	rect.material = _mat
	add_child(rect)
	_label.set_anchors_preset(Control.PRESET_FULL_RECT)
	_label.offset_left = 80
	_label.offset_top = 80
	var font := SystemFont.new()
	font.font_names = PackedStringArray(["Consolas", "Courier New", "monospace"])
	_label.add_theme_font_override("font", font)
	_label.add_theme_font_size_override("font_size", 26)
	_label.add_theme_color_override("font_color", Color(0.45, 1.0, 0.6))
	add_child(_label)
	visible = false


## Shown, unchanged (block 1, no fade), for a few frames under the loading cover: the shader's
## pipeline compiles then, not on the first death.
func warm() -> void:
	visible = true
	for _i in 8:
		await get_tree().process_frame
	if not playing:
		visible = false


func play(specimen: int, cause: String, respawn: Callable) -> void:
	playing = true
	visible = true
	_label.text = ""
	_shade(1.0, 0.0)
	var t := create_tween().set_trans(Tween.TRANS_QUAD).set_ease(Tween.EASE_IN)
	t.tween_method(func(b: float) -> void: _shade(b, b / BLOCK_PX * 0.8), 1.0, BLOCK_PX, 1.4)
	await t.finished
	await _type("> APETION %04d  SIGNAL LOST\n> cause: %s\n> recovering voxel record ...\n" % [specimen, cause])
	var head := _label.text
	t = create_tween()
	t.tween_method(func(f: float) -> void:
		var n := int(f * BAR)
		_label.text = head + "> RECONSTRUCTING  [%s%s] %3d%%\n" % ["█".repeat(n), "░".repeat(BAR - n), int(f * 100)], 0.0, 1.0, 1.6)
	t.tween_method(func(f: float) -> void: _shade(BLOCK_PX, f), 0.8, 1.0, 0.3)
	await t.finished
	respawn.call()
	await _type("> APETION %04d  ONLINE. CONTINUE.\n" % (specimen + 1))
	await get_tree().create_timer(0.8).timeout
	_label.text = ""
	t = create_tween().set_trans(Tween.TRANS_QUAD).set_ease(Tween.EASE_OUT)
	t.tween_method(func(b: float) -> void: _shade(b, b / BLOCK_PX), BLOCK_PX, 1.0, 1.2)
	await t.finished
	visible = false
	playing = false


func _shade(block: float, fade: float) -> void:
	_mat.set_shader_parameter("block", block)
	_mat.set_shader_parameter("fade", fade)


## Types `line` after what the label already shows.
func _type(line: String) -> void:
	var head := _label.text
	var t := create_tween()
	t.tween_method(func(n: int) -> void: _label.text = head + line.substr(0, n), 0, line.length(), line.length() / CHARS_PER_S)
	await t.finished
