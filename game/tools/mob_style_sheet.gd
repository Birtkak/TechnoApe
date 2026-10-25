extends Node

## Renders the animals at each MobModel cell size (1 = 3.1 cm cells, 2 = 6.25 cm, 4 = the world's
## 12.5 cm voxel) for picking a style: one row per size, sheep, sheared sheep, pig, chicken, bear,
## from the front-left, with a strip of patterned grass voxels under them for scale.
##   godot_console.exe --path game res://tools/mob_style_sheet.tscn -- out=res://out/mobs/styles.png
## (a scene, not --script: --script runs without the autoloads, and the models read Settings)
## cells=1,2,4 picks the rows; walk=0.5 poses them mid-stride. Not headless (needs the renderer).
## film=1: a filmstrip instead: a sheep and a pig walking at their walk_m speed (mobs.json),
## a frame every 100 ms for 1.6 s, side on (motion is never judged from one frame).

var out_path := "res://out/mobs/styles.png"
var cells := [1, 2, 4]
var _walk := 0.0
var _film := false
var _t := 0.0
var _next_t := 0.0
const FILM_DT := 0.1
const FILM_FRAMES := 16
const WALK_M := {"sheep": 1.1, "pig": 1.2, "chicken": 1.0}
var _frames := 0
var _shots: Array[Image] = []
var _cams: Array[Camera3D] = []
var _models: Array[MobModel] = []


func _ready() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() != 2:
			continue
		match kv[0]:
			"out": out_path = kv[1]
			"walk": _walk = float(kv[1])
			"film": _film = kv[1] == "1"
			"cells":
				cells = []
				for c in kv[1].split(","):
					cells.append(int(c))
	var root3d := Node3D.new()
	add_child(root3d)
	if _film:
		cells = [MobModel.cell]
	var z := 0.0
	for c in cells:
		MobModel.cell = c
		var x := 0.0
		var lineup := [["sheep", false], ["pig", false]] if _film else [["sheep", false], ["sheep", true], ["pig", false], ["chicken", false], ["bear", false]]
		for e in lineup:
			var m := MobModel.create(e[0], e[1])
			root3d.add_child(m)
			m.position = Vector3(x, 0, z)
			m.rotation.y = -0.5 if not _film else PI / 2.0   # film: walking toward +x, side on
			if _film:
				m.position = Vector3(-3.0 - _models.size() * 3.5, 0, z)
			_models.append(m)
			x += 3.4 if e[0] != "bear" else 4.0
		var cam := Camera3D.new()
		cam.fov = 34
		root3d.add_child(cam)
		if _film:
			cam.look_at_from_position(Vector3(-4.6, 1.6, z + 7.5), Vector3(-4.6, 1.0, z))
		else:
			cam.look_at_from_position(Vector3(6.6, 4.2, z + 13.0), Vector3(6.6, 1.0, z))
		_cams.append(cam)
		z += 40.0
	MobModel.cell = 2
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-50, 30, 0)
	sun.light_energy = 1.1
	sun.shadow_enabled = true
	root3d.add_child(sun)
	var env := WorldEnvironment.new()
	var e2 := Environment.new()
	e2.background_mode = Environment.BG_COLOR
	e2.background_color = Color(0.55, 0.68, 0.85)
	e2.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	e2.ambient_light_color = Color(0.7, 0.7, 0.75)
	e2.ambient_light_energy = 0.8
	e2.tonemap_mode = Environment.TONE_MAPPER_ACES
	env.environment = e2
	root3d.add_child(env)
	# patterned ground: 0.5-unit tiles (a 12.5 cm voxel at this scale is 0.25 units: GRID cells = 1 unit = 0.5 m)
	var ground := MultiMeshInstance3D.new()
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.use_colors = true
	var bm := BoxMesh.new()
	bm.size = Vector3(0.25, 0.25, 0.25)
	var gm := StandardMaterial3D.new()
	gm.vertex_color_use_as_albedo = true
	gm.roughness = 1.0
	bm.material = gm
	mm.mesh = bm
	var nx := 90
	var nz := 40 * 4 * cells.size()
	mm.instance_count = nx * nz
	var rng := RandomNumberGenerator.new()
	rng.seed = 3
	var greens := [Color8(70, 120, 40), Color8(84, 140, 48), Color8(96, 152, 54), Color8(60, 104, 36), Color8(110, 162, 60)]
	var i := 0
	for iz in nz:
		for ix in nx:
			mm.set_instance_transform(i, Transform3D(Basis(), Vector3(-4.0 + ix * 0.25, -0.125, -6.0 + iz * 0.25)))
			mm.set_instance_color(i, greens[rng.randi_range(0, greens.size() - 1)])
			i += 1
	mm.instance_count = i
	ground.multimesh = mm
	root3d.add_child(ground)
	_cams[0].current = true
	DisplayServer.window_set_size(Vector2i(1200, 380))


func _process(delta: float) -> void:
	_frames += 1
	if _film:
		_film_step()
		return
	for m in _models:
		m.animate(delta, 0.0, 0.0, 0.2, 0.0, 0.0)
		if _walk > 0.0:
			for k in m.legs.size():
				m.legs[k].rotation.x = (0.6 if k in [0, 3] else -0.6) * _walk
	if _frames % 4 != 0:
		return
	var k := _frames / 4 - 1
	if k >= 1:
		_shots.append(get_viewport().get_texture().get_image())
	if k < _cams.size():
		_cams[k].current = true
		return
	var w := _shots[0].get_width()
	var h := _shots[0].get_height()
	var sheet := Image.create(w, h * _shots.size(), false, Image.FORMAT_RGB8)
	for j in _shots.size():
		sheet.blit_rect(_shots[j], Rect2i(0, 0, w, h), Vector2i(0, j * h))
	var abs_path := ProjectSettings.globalize_path(out_path)
	DirAccess.make_dir_recursive_absolute(abs_path.get_base_dir())
	sheet.save_png(abs_path)
	print(JSON.stringify({"mob_style_sheet": abs_path, "rows": _shots.size(), "cells": cells}))
	get_tree().quit(0)


## Walks the animals with a fixed step (so the strip shows game time, not render time) and grabs a
## frame every FILM_DT; then lays the frames side by side.
func _film_step() -> void:
	var dt := 1.0 / 60.0
	if _frames < 4:
		return
	for m in _models:
		var mps: float = WALK_M.get(m.species, 1.0)
		m.animate(dt, mps / Settings.VOXEL_M, 0.0, 0.0, 0.0, 0.0)
		m.position.x += mps * 2.0 * dt   # sheet units: 1 = 0.5 m
	_t += dt
	_cams[0].position.x = _models[0].position.x - 1.6
	if _t + 1e-6 >= _next_t:
		_next_t += FILM_DT
		_shots.append(get_viewport().get_texture().get_image())
	if _shots.size() < FILM_FRAMES:
		return
	var w := _shots[0].get_width() / 2
	var h := _shots[0].get_height() / 2
	var cols := 4
	var sheet := Image.create(w * cols, h * (FILM_FRAMES / cols), false, Image.FORMAT_RGB8)
	for j in _shots.size():
		var img := _shots[j]
		img.resize(w, h)
		sheet.blit_rect(img, Rect2i(0, 0, w, h), Vector2i((j % cols) * w, (j / cols) * h))
	var abs_path := ProjectSettings.globalize_path(out_path)
	DirAccess.make_dir_recursive_absolute(abs_path.get_base_dir())
	sheet.save_png(abs_path)
	print(JSON.stringify({"mob_film": abs_path, "frames": _shots.size(), "dt_ms": FILM_DT * 1000.0}))
	get_tree().quit(0)
