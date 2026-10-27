class_name FrameSheet
extends RefCounted

## Collects viewport frames as thumbnails and saves them as one contact-sheet
## PNG (row-major, left to right, top to bottom) so Claude can "watch" a run.

var thumb_w := 320
var thumb_h := 180
var columns := 5
var _frames: Array[Image] = []


func _init(p_thumb_w: int = 320, p_thumb_h: int = 180, p_columns: int = 5) -> void:
	thumb_w = p_thumb_w
	thumb_h = p_thumb_h
	columns = p_columns


func add(viewport: Viewport) -> void:
	var img := viewport.get_texture().get_image()
	img.resize(thumb_w, thumb_h, Image.INTERPOLATE_BILINEAR)
	_frames.append(img)


func count() -> int:
	return _frames.size()


func save(path: String) -> int:
	if _frames.is_empty():
		return ERR_DOES_NOT_EXIST
	var rows := int(ceil(float(_frames.size()) / columns))
	var sheet := Image.create(columns * (thumb_w + 2), rows * (thumb_h + 2), false, Image.FORMAT_RGB8)
	sheet.fill(Color(0.1, 0.1, 0.1))
	for i in _frames.size():
		var f := _frames[i]
		if f.get_format() != Image.FORMAT_RGB8:
			f.convert(Image.FORMAT_RGB8)
		var x := (i % columns) * (thumb_w + 2)
		var y := (i / columns) * (thumb_h + 2)
		sheet.blit_rect(f, Rect2i(0, 0, thumb_w, thumb_h), Vector2i(x, y))
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(path.get_base_dir()))
	return sheet.save_png(path)


## A burst to watch motion in a scripted check (craft_film=1): `frames` consecutive frames saved as
## one sheet at `path`. Run with `--fixed-fps 60` so every frame is 1/60 s of game time however long
## reading it back stalls.
static func burst(viewport: Viewport, path: String, frames := 36) -> void:
	var sheet := FrameSheet.new(320, 180, 6)
	for i in frames:
		await viewport.get_tree().process_frame
		sheet.add(viewport)
	sheet.save(path)
