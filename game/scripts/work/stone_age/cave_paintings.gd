class_name CavePaintings
extends Node3D

## Paintings on walls (ADR 0059): paint (ochre, charcoal or malachite ground on a stone and mixed
## with fat) right clicked on a face of a solid voxel paints a 0.5 m picture there as a Decal
## (projected onto the voxels, so it follows the rock); the motifs come in turn (hand stencils,
## aurochs, deer, horses, a hunter, dots). A right click on a painting paints the next motif over
## it. Each stroke uses the paint (its durability). Saved with the world.

const MOTIFS := {
	"hand": ["................", "......#.........", "....#.#.#.......", "....#.#.#.#.....",
		"....#.#.#.#.....", "....#.#.#.#.....", "....#######.....", "..#.#######.....",
		"..##########....", "...#########....", "....########....", ".....######.....",
		".....######.....", ".....######.....", "................", "................"],
	"aurochs": ["................", "................", "..#.............", "...#.......#....",
		"...######..#....", "..###########...", ".##############.", ".#############..",
		"..############..", "..###########...", "..##.......##...", "..#.#.....#.#...",
		"..#.#.....#.#...", "................", "................", "................"],
	"deer": ["...#.#..........", "....#...........", "...#.#..........", "....##..........",
		"....###.........", ".....#########..", ".....#########..", "......########..",
		"......#......#..", "......#......#..", "......#......#..", "................",
		"................", "................", "................", "................"],
	"horse": ["................", "................", "...##...........", "..####..........",
		"..#####.........", "....####........", ".....##########.", ".....##########.",
		"......#########.", "......##.....##.", "......#.#....#.#", "......#.#....#.#",
		"................", "................", "................", "................"],
	"hunter": ["......##........", "......##........", ".......#........", ".....####.......",
		"....#.##.#......", "...#..##..#.....", "..#...##...#....", ".#....##....#...",
		"......##........", ".....#..#.......", "....#....#......", "...#......#.....",
		"..#........#....", "................", "................", "................"],
	"dots": ["................", "................", "..##....##......", "..##....##...##.",
		"..............##", "......##........", "......##....##..", "..##........##..",
		"..##............", ".......##.......", ".......##...##..", "............##..",
		"...##...........", "...##.....##....", "..........##....", "................"],
}
const ORDER := ["hand", "aurochs", "deer", "horse", "hunter", "dots"]

var list: Array = []      # {pos: Vector3 (face centre), normal: Vector3i, motif, color, node}
var _tex := {}            # motif -> ImageTexture
var _next := 0


static func motif_image(motif: String) -> Image:
	var img := Image.create(32, 32, false, Image.FORMAT_RGBA8)
	var rows: Array = MOTIFS[motif]
	var rng := RandomNumberGenerator.new()
	rng.seed = hash(motif)
	for y in 32:
		for x in 32:
			var on := String(rows[y / 2])[x / 2] == "#"
			# blown pigment: a soft, uneven fill, rough at the edges
			var a := (0.75 + rng.randf() * 0.25) if on else (0.25 * rng.randf() if rng.randf() < 0.08 else 0.0)
			img.set_pixel(x, y, Color(1, 1, 1, a))
	return img


func _texture(motif: String) -> Texture2D:
	if not _tex.has(motif):
		_tex[motif] = ImageTexture.create_from_image(motif_image(motif))
	return _tex[motif]


func count() -> int:
	return list.size()


## A right click with paint on a voxel face. True when it took the click.
func paint(main: Node, hit: VoxelRaycastResult, held: Dictionary, _from: Vector3, _dir: Vector3) -> bool:
	var m: int = main.get_world_voxel(hit.position)
	if not MobManager.is_solid(m):
		return false
	var n: Vector3i = hit.previous_position - hit.position
	if n.length_squared() != 1:
		return true
	var centre := Vector3(hit.position) + Vector3(0.5, 0.5, 0.5) + Vector3(n) * 0.5
	var motif := String(ORDER[_next % ORDER.size()])
	for i in list.size():
		if list[i]["normal"] == n and (list[i]["pos"] as Vector3).distance_to(centre) < Settings.BLOCK * 0.6:
			motif = String(ORDER[(ORDER.find(list[i]["motif"]) + 1) % ORDER.size()])
			list[i]["node"].queue_free()
			list.remove_at(i)
			break
	_next = ORDER.find(motif) + 1
	add(centre, n, motif, Color(String(held["paint"].get("color", "#a8301e"))))
	main.work._wear_held()
	main.work._note("painted", held["id"])
	return true


func add(centre: Vector3, n: Vector3i, motif: String, color: Color) -> void:
	var d := Decal.new()
	d.texture_albedo = _texture(motif)
	d.modulate = color
	d.size = Vector3(Settings.BLOCK, 0.6, Settings.BLOCK)   # 0.5 m square, projected 0.6 voxel deep
	d.upper_fade = 0.1
	d.lower_fade = 0.1
	d.distance_fade_enabled = true
	d.distance_fade_begin = Settings.m2v(30.0)
	d.distance_fade_length = Settings.m2v(6.0)
	# a decal projects along its -Y: lay its Y along the face normal
	var up := Vector3(n)
	var side := Vector3.RIGHT if absf(up.x) < 0.9 else Vector3.BACK
	var fwd := side.cross(up).normalized()
	side = up.cross(fwd).normalized()
	if up.y == 0.0:
		# on a wall the picture stands upright: its "down" (+Z of the texture) points down
		fwd = Vector3.DOWN
		side = up.cross(fwd).normalized()
	d.basis = Basis(side, up, fwd)
	d.position = centre
	add_child(d)
	list.append({"pos": centre, "normal": n, "motif": motif, "color": color, "node": d})


func to_array() -> Array:
	var out := []
	for p in list:
		var c: Vector3 = p["pos"]
		var n: Vector3i = p["normal"]
		out.append({"pos": [c.x, c.y, c.z], "normal": [n.x, n.y, n.z], "motif": p["motif"], "color": (p["color"] as Color).to_html(false)})
	return out


func load_array(_main: Node, a: Array) -> void:
	for d in a:
		var p: Array = d.get("pos", [])
		var n: Array = d.get("normal", [])
		if p.size() == 3 and n.size() == 3 and MOTIFS.has(String(d.get("motif", ""))):
			add(Vector3(p[0], p[1], p[2]), Vector3i(int(n[0]), int(n[1]), int(n[2])), String(d["motif"]), Color(String(d.get("color", "#a8301e"))))
