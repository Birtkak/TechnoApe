class_name WildModels
extends RefCounted

## Bodies of the wild animals of ADR 0058, in MobModel's cells (16 per 0.5 m, so a cell is 3.1 cm;
## part C's half-voxel look merges them to 6.25 cm, so faces sit on even cells, 2 x 2 or more).
## MobModel asks here first: has() / layout() / build() / animate(). Paints are MobModel palette
## indices + 1; the colours of these animals are appended to MobModel.PALETTE (BASE on).

const BASE := 16   # MobModel.PALETTE.size() before these
const DEER := BASE + 0
const DEER_LIGHT := BASE + 1
const ANTLER := BASE + 2
const BOAR := BASE + 3
const BOAR_DARK := BASE + 4
const TUSK := BASE + 5
const RABBIT := BASE + 6
const RABBIT_LIGHT := BASE + 7
const FOX := BASE + 8
const FOX_WHITE := BASE + 9
const FOX_DARK := BASE + 10
const WOLF := BASE + 11
const WOLF_LIGHT := BASE + 12
const WOLF_DARK := BASE + 13
const DUCK_HEAD := BASE + 14
const DUCK_BODY := BASE + 15
const BILL := BASE + 16
const FROG := BASE + 17
const FROG_BELLY := BASE + 18
const CRAB := BASE + 19
const CRAB_DARK := BASE + 20
const FISH := BASE + 21
const FISH_BELLY := BASE + 22
const FISH_FIN := BASE + 23
const BAT := BASE + 24
const BAT_WING := BASE + 25
const SPIDER := BASE + 26
const SPIDER_EYE := BASE + 27
const DART := BASE + 28        # the poison dart frog's warning blue
const DART_DARK := BASE + 29
const GLOAM := BASE + 30        # the gloam hound's dusk-violet ramp (ADR 0129: cooler, more saturated shadows)
const GLOAM_LIGHT := BASE + 31
const GLOAM_DARK := BASE + 32
const GLOAM_EYE := BASE + 33
const CROW := BASE + 34          # crow: blue-black ramp, warm grey flight feathers, a horn beak (ADR 0129)
const CROW_LIGHT := BASE + 35
const CROW_DARK := BASE + 36
const BEAK := BASE + 37
const SALMON := BASE + 38        # salmon: silver-pink flanks over a cooler blue-grey back
const SALMON_BELLY := BASE + 39
const SALMON_FIN := BASE + 40
const JELLY := BASE + 41         # jellyfish: a pale violet bell, warmer rim, a hot pink heart that catches the eye
const JELLY_LIGHT := BASE + 42
const JELLY_DARK := BASE + 43
const JELLY_HEART := BASE + 44
const SALA := BASE + 45          # cave salamander: a violet-black ramp with warm yellow spots, lit from the top left
const SALA_LIGHT := BASE + 46
const SALA_DARK := BASE + 47
const SALA_SPOT := BASE + 48
const PALETTE: Array = [
	[150, 102, 62], [218, 190, 152], [222, 208, 172], [96, 74, 58], [62, 46, 36], [238, 230, 208],
	[164, 132, 100], [232, 222, 206], [210, 104, 40], [242, 234, 222], [52, 36, 30],
	[132, 130, 126], [200, 196, 188], [70, 68, 68], [42, 112, 72], [156, 134, 108], [240, 172, 44],
	[86, 150, 62], [206, 214, 142], [204, 72, 42], [140, 42, 26], [112, 142, 162], [222, 228, 222], [76, 98, 112],
	[70, 56, 50], [42, 34, 34], [54, 42, 44], [214, 34, 34], [36, 96, 224], [18, 22, 34],
	[64, 54, 84], [104, 94, 108], [30, 22, 56], [130, 255, 214],
	[40, 40, 54], [96, 92, 98], [18, 18, 38], [70, 62, 58],
	[214, 132, 120], [236, 210, 196], [96, 104, 132],
	[176, 150, 214], [228, 206, 236], [112, 86, 168], [244, 96, 168],
	[46, 40, 54], [78, 70, 84], [26, 20, 40], [246, 198, 58],
]
const EYE := 7          # MobModel.EYE
const SKIN_DARK := 3    # MobModel.SKIN_DARK: hooves
const PINK_DARK := 5
const YELLOW := 12

## Per species: [part, role, position in cells]. Roles: body, head, leg, wing, tail. Legs in the
## order MobModel.animate swings them (pairs: front left, front right, back left, back right, ...).
const LAYOUT := {
	"deer": [["body", "body", Vector3(0, 24, 0)], ["head", "head", Vector3(0, 36, 15)],
		["leg", "leg", Vector3(5, 26, 13)], ["leg", "leg", Vector3(-5, 26, 13)], ["leg", "leg", Vector3(5, 26, -13)], ["leg", "leg", Vector3(-5, 26, -13)]],
	"boar": [["body", "body", Vector3(0, 10, 0)], ["head", "head", Vector3(0, 16, 16)],
		["leg", "leg", Vector3(6, 12, 11)], ["leg", "leg", Vector3(-6, 12, 11)], ["leg", "leg", Vector3(6, 12, -11)], ["leg", "leg", Vector3(-6, 12, -11)]],
	"rabbit": [["body", "body", Vector3(0, 4, 0)], ["head", "head", Vector3(0, 10, 6)],
		["leg", "leg", Vector3(3, 6, 4)], ["leg", "leg", Vector3(-3, 6, 4)], ["hind", "leg", Vector3(3, 6, -4)], ["hind", "leg", Vector3(-3, 6, -4)]],
	"fox": [["body", "body", Vector3(0, 9, 0)], ["head", "head", Vector3(0, 15, 11)], ["tail", "tail", Vector3(0, 16, -12)],
		["leg", "leg", Vector3(3, 10, 8)], ["leg", "leg", Vector3(-3, 10, 8)], ["leg", "leg", Vector3(3, 10, -8)], ["leg", "leg", Vector3(-3, 10, -8)]],
	"wolf": [["body", "body", Vector3(0, 14, 0)], ["head", "head", Vector3(0, 23, 14)], ["tail", "tail", Vector3(0, 25, -15)],
		["leg", "leg", Vector3(4, 16, 11)], ["leg", "leg", Vector3(-4, 16, 11)], ["leg", "leg", Vector3(4, 16, -11)], ["leg", "leg", Vector3(-4, 16, -11)]],
	"duck": [["body", "body", Vector3(0, 6, 0)], ["head", "head", Vector3(0, 13, 6)],
		["leg", "leg", Vector3(2.5, 7, 0)], ["leg", "leg", Vector3(-2.5, 7, 0)], ["wing", "wing", Vector3(5, 14, -1)], ["wing", "wing", Vector3(-5, 14, -1)]],
	"frog": [["body", "body", Vector3(0, 3, -1)], ["head", "head", Vector3(0, 4, 5)],
		["leg", "leg", Vector3(5, 4, 3)], ["leg", "leg", Vector3(-5, 4, 3)], ["hind", "leg", Vector3(6, 5, -4)], ["hind", "leg", Vector3(-6, 5, -4)]],
	"gloam_hound": [["body", "body", Vector3(0, 14, 0)], ["head", "head", Vector3(0, 23, 14)], ["tail", "tail", Vector3(0, 25, -15)],
		["leg", "leg", Vector3(4, 16, 11)], ["leg", "leg", Vector3(-4, 16, 11)], ["leg", "leg", Vector3(4, 16, -11)], ["leg", "leg", Vector3(-4, 16, -11)]],
	"crow": [["body", "body", Vector3(0, 5, -1)], ["head", "head", Vector3(0, 9, 6)], ["tail", "tail", Vector3(0, 8, -7)],
		["leg", "leg", Vector3(2, 6, 0)], ["leg", "leg", Vector3(-2, 6, 0)], ["wing_l", "wing", Vector3(4, 11, 0)], ["wing_r", "wing", Vector3(-4, 11, 0)]],
	"salmon": [["body", "body", Vector3(0, 0, 0)], ["head", "head", Vector3(0, 1, 10)], ["tail", "tail", Vector3(0, 4, -9)]],
	"jellyfish": [["body", "body", Vector3(0, 10, 0)], ["head", "head", Vector3(0, 19, 0)], ["leg", "leg", Vector3(3, 11, 3)], ["leg", "leg", Vector3(-3, 11, 3)],
		["leg", "leg", Vector3(3, 11, -3)], ["leg", "leg", Vector3(-3, 11, -3)]],
	"poison_frog": [["body", "body", Vector3(0, 3, -1)], ["head", "head", Vector3(0, 4, 5)],
		["leg", "leg", Vector3(5, 4, 3)], ["leg", "leg", Vector3(-5, 4, 3)], ["hind", "leg", Vector3(6, 5, -4)], ["hind", "leg", Vector3(-6, 5, -4)]],
	"cave_salamander": [["body", "body", Vector3(0, 3, 0)], ["head", "head", Vector3(0, 3, 9)], ["tail", "tail", Vector3(0, 4, -9)],
		["leg", "leg", Vector3(5, 4, 5)], ["leg", "leg", Vector3(-5, 4, 5)], ["leg", "leg", Vector3(5, 4, -5)], ["leg", "leg", Vector3(-5, 4, -5)]],
	"crab": [["body", "body", Vector3(0, 4, 0)], ["head", "head", Vector3(0, 6, 6)],
		["leg_l", "leg", Vector3(8, 7, 3)], ["leg_r", "leg", Vector3(-8, 7, 3)], ["leg_l", "leg", Vector3(8, 7, -1)], ["leg_r", "leg", Vector3(-8, 7, -1)],
		["leg_l", "leg", Vector3(8, 7, -5)], ["leg_r", "leg", Vector3(-8, 7, -5)]],
	"fish": [["body", "body", Vector3(0, 0, 0)], ["head", "head", Vector3(0, 1, 10)], ["tail", "tail", Vector3(0, 4, -9)]],
	"bat": [["body", "body", Vector3(0, 3, 0)], ["head", "head", Vector3(0, 10, 3)],
		["leg", "leg", Vector3(2, 4, -2)], ["leg", "leg", Vector3(-2, 4, -2)], ["wing_l", "wing", Vector3(4, 9, 0)], ["wing_r", "wing", Vector3(-4, 9, 0)]],
	"cave_spider": [["body", "body", Vector3(0, 6, -6)], ["head", "head", Vector3(0, 8, 4)],
		["leg_l", "leg", Vector3(7, 12, 6)], ["leg_r", "leg", Vector3(-7, 12, 6)], ["leg_l", "leg", Vector3(7, 12, 3)], ["leg_r", "leg", Vector3(-7, 12, 3)],
		["leg_l", "leg", Vector3(7, 12, 0)], ["leg_r", "leg", Vector3(-7, 12, 0)], ["leg_l", "leg", Vector3(7, 12, -3)], ["leg_r", "leg", Vector3(-7, 12, -3)]],
}


static func has(sp: String) -> bool:
	return LAYOUT.has(sp)


## Every part name of a species (MobModel.prewarm).
static func parts(sp: String) -> Array:
	var out := []
	for e in LAYOUT[sp]:
		if not e[0] in out:
			out.append(e[0])
	return out


## Builds the species' nodes on a MobModel (its _ready).
static func layout(m: MobModel) -> void:
	var s := 1.0 / MobModel.GRID
	for e in LAYOUT[m.species]:
		var mi: MeshInstance3D = m._part(e[0], e[2] * s)
		match e[1]:
			"body": m.body = mi
			"head": m.head = mi
			"leg": m.legs.append(mi)
			"wing": m.wings.append(mi)
			"tail": m.tails.append(mi)


## Tails and fins (MobModel.animate calls this last); mps = ground speed in m/s.
static func animate(m: MobModel, mps: float) -> void:
	for t in m.tails:
		match m.species:
			"fish": t.rotation.y = sin(m._time * (4.0 + mps * 10.0)) * 0.45
			"fox", "wolf": t.rotation.y = sin(m._time * 2.2) * 0.15 + sin(m._phase) * 0.1
			"cave_salamander": t.rotation.y = sin(m._phase * 2.0) * 0.35   # the tail snakes with each step
	if m.species == "fish":
		m.body.rotation.y = -sin(m._time * (4.0 + mps * 10.0)) * 0.08
	elif m.species == "crab":
		for i in m.legs.size():   # legs scuttle sideways
			m.legs[i].rotation.z = sin(m._phase * 1.6 + i * 1.3) * 0.35 * clampf(mps * 2.0, 0.0, 1.0) * (1.0 if i % 2 == 0 else -1.0)


# ---------------- cell models ----------------

## A species drawn as another's shape in its own colours: paints `from -> to`.
static func _recolor(spec: Array, swap: Dictionary) -> Array:
	var c: PackedInt32Array = spec[0]
	for i in c.size():
		if c[i] > 0 and swap.has(c[i] - 1):
			c[i] = swap[c[i] - 1] + 1
	return spec


## The jungle's poison dart frog (bug 2026-11-02_18-54-25): the frog's shape in warning blue with black blotches.
static func _dart_frog(spec: Array, part: String) -> Array:
	var c: PackedInt32Array = spec[0]
	var size: Vector3i = spec[1]
	var rng := RandomNumberGenerator.new()
	rng.seed = hash("poison_frog" + part)
	for i in c.size():
		if c[i] == FROG + 1 or c[i] == FOX_DARK + 1:
			c[i] = DART + 1
		elif c[i] == FROG_BELLY + 1:
			c[i] = DART_DARK + 1
	_speckle(c, size, rng, c.size() / 12, DART_DARK)
	return spec


static func _box(c: PackedInt32Array, size: Vector3i, from: Vector3i, to: Vector3i, paint: int) -> void:
	MobModel._box(c, size, from, to, paint + 1)


static func _speckle(c: PackedInt32Array, size: Vector3i, rng: RandomNumberGenerator, n: int, paint: int, lo := Vector3i.ZERO, hi := Vector3i(-1, -1, -1)) -> void:
	var top := size - Vector3i.ONE if hi.x < 0 else hi
	for i in n:
		var p := Vector3i(rng.randi_range(lo.x, top.x), rng.randi_range(lo.y, top.y), rng.randi_range(lo.z, top.z))
		if c[p.x + size.x * (p.y + size.y * p.z)] > 0:
			MobModel._put(c, size, p, paint + 1)


## [cells, size, origin (voxels)] as MobModel._build returns it.
static func build(sp: String, part: String) -> Array:
	if sp == "poison_frog":
		return _dart_frog(build("frog", part), part)
	if sp == "salmon":
		return _recolor(build("fish", part), {FISH: SALMON, FISH_BELLY: SALMON_BELLY, FISH_FIN: SALMON_FIN})
	if sp == "gloam_hound":
		return _recolor(build("wolf", part), {WOLF: GLOAM, WOLF_LIGHT: GLOAM_LIGHT, WOLF_DARK: GLOAM_DARK, YELLOW: GLOAM_EYE})   # wolf eyes are YELLOW; EYE is its nose
	var rng := RandomNumberGenerator.new()
	rng.seed = hash(sp + part)
	var s := 1.0 / MobModel.GRID
	var size := Vector3i.ONE
	var c := PackedInt32Array()
	var o := Vector3.ZERO
	match sp + "/" + part:
		"deer/body":
			size = Vector3i(16, 16, 36)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 0), size, DEER)
			_speckle(c, size, rng, 90, DEER_LIGHT, Vector3i(0, 8, 4), Vector3i(15, 15, 33))
			_box(c, size, Vector3i(0, 0, 2), Vector3i(16, 3, 34), DEER_LIGHT)         # pale belly
			_box(c, size, Vector3i(4, 8, 0), Vector3i(12, 16, 2), DEER_LIGHT)         # white rump
			_box(c, size, Vector3i(6, 12, 0), Vector3i(10, 16, 1), DEER)              # the tail over it
			o = Vector3(-8, 0, -18)
		"deer/head":
			# a neck rising from the shoulders, the head forward on it, ears, antlers
			size = Vector3i(14, 34, 24)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(4, 0, 0), Vector3i(10, 18, 8), DEER)
			_box(c, size, Vector3i(4, 0, 4), Vector3i(10, 12, 8), DEER_LIGHT)         # throat
			_box(c, size, Vector3i(2, 14, 4), Vector3i(12, 24, 18), DEER)
			_box(c, size, Vector3i(4, 14, 18), Vector3i(10, 20, 24), DEER)            # muzzle
			_box(c, size, Vector3i(4, 14, 18), Vector3i(10, 16, 24), DEER_LIGHT)
			_box(c, size, Vector3i(6, 18, 22), Vector3i(8, 20, 24), EYE)              # nose
			for x in [2, 10]:
				_box(c, size, Vector3i(x, 20, 14), Vector3i(x + 2, 22, 16), EYE)
			_box(c, size, Vector3i(0, 22, 8), Vector3i(4, 26, 10), DEER)               # ears
			_box(c, size, Vector3i(10, 22, 8), Vector3i(14, 26, 10), DEER)
			for x in [4, 8]:                                                           # antlers: beam, two tines
				_box(c, size, Vector3i(x, 24, 10), Vector3i(x + 2, 32, 12), ANTLER)
				_box(c, size, Vector3i(x, 30, 12), Vector3i(x + 2, 34, 14), ANTLER)
				_box(c, size, Vector3i(2 if x == 4 else 10, 28, 10), Vector3i(4 if x == 4 else 12, 30, 12), ANTLER)
			o = Vector3(-7, -2, -2)
		"deer/leg":
			size = Vector3i(6, 26, 6)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 0), size, DEER)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(6, 2, 6), SKIN_DARK)
			_box(c, size, Vector3i(0, 2, 0), Vector3i(6, 10, 6), DEER_LIGHT)
			o = Vector3(-3, -26, -3)
		"boar/body":
			size = Vector3i(20, 20, 34)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(20, 18, 34), BOAR)
			_box(c, size, Vector3i(6, 18, 6), Vector3i(14, 20, 30), BOAR_DARK)        # the bristly ridge
			_speckle(c, size, rng, 260, BOAR_DARK)
			_box(c, size, Vector3i(8, 10, 0), Vector3i(12, 12, 1), BOAR_DARK)         # tail
			o = Vector3(-10, 0, -17)
		"boar/head":
			size = Vector3i(16, 16, 18)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(16, 14, 12), BOAR)
			_speckle(c, size, rng, 60, BOAR_DARK, Vector3i(0, 0, 0), Vector3i(15, 13, 11))
			_box(c, size, Vector3i(4, 2, 12), Vector3i(12, 10, 18), BOAR_DARK)        # snout
			_box(c, size, Vector3i(4, 2, 17), Vector3i(12, 8, 18), PINK_DARK)         # nose disc
			for x in [4, 10]:
				_box(c, size, Vector3i(x, 4, 17), Vector3i(x + 2, 6, 18), EYE)        # nostrils
			for x in [2, 12]:
				_box(c, size, Vector3i(x, 2, 12), Vector3i(x + 2, 8, 14), TUSK)       # tusks
				_box(c, size, Vector3i(x, 10, 11), Vector3i(x + 2, 12, 12), EYE)
			_box(c, size, Vector3i(0, 14, 4), Vector3i(4, 16, 8), BOAR_DARK)          # ears
			_box(c, size, Vector3i(12, 14, 4), Vector3i(16, 16, 8), BOAR_DARK)
			o = Vector3(-8, -6, -1)
		"boar/leg":
			size = Vector3i(6, 12, 6)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 0), size, BOAR_DARK)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(6, 2, 6), EYE)
			o = Vector3(-3, -12, -3)
		"rabbit/body":
			size = Vector3i(10, 10, 14)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 0), size, RABBIT)
			_speckle(c, size, rng, 30, RABBIT_LIGHT)
			_box(c, size, Vector3i(0, 0, 2), Vector3i(10, 2, 14), RABBIT_LIGHT)
			_box(c, size, Vector3i(4, 6, 0), Vector3i(8, 10, 2), RABBIT_LIGHT)        # cotton tail
			o = Vector3(-5, 0, -7)
		"rabbit/head":
			size = Vector3i(8, 18, 10)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(8, 8, 10), RABBIT)
			for x in [0, 6]:
				_box(c, size, Vector3i(x, 8, 2), Vector3i(x + 2, 18, 4), RABBIT)      # long ears
				_box(c, size, Vector3i(x, 10, 4), Vector3i(x + 2, 16, 5), RABBIT_LIGHT)
				_box(c, size, Vector3i(x, 4, 6), Vector3i(x + 2, 6, 8), EYE)
			_box(c, size, Vector3i(2, 2, 9), Vector3i(6, 4, 10), PINK_DARK)
			o = Vector3(-4, -2, -2)
		"rabbit/leg":
			size = Vector3i(2, 6, 4)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 0), size, RABBIT)
			o = Vector3(-1, -6, -2)
		"rabbit/hind":
			size = Vector3i(4, 6, 8)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 2, 2), Vector3i(4, 6, 8), RABBIT)               # haunch
			_box(c, size, Vector3i(0, 0, 0), Vector3i(4, 2, 8), RABBIT_LIGHT)         # long foot
			o = Vector3(-2, -6, -4)
		"fox/body", "wolf/body":
			var wolf := sp == "wolf"
			size = Vector3i(14, 14, 30) if wolf else Vector3i(10, 10, 24)
			c = MobModel._cells(size)
			_box(c, size, Vector3i.ZERO, size, WOLF if wolf else FOX)
			_box(c, size, Vector3i(0, 0, 2), Vector3i(size.x, 2, size.z - 2), WOLF_LIGHT if wolf else FOX_WHITE)
			if wolf:
				_box(c, size, Vector3i(2, 11, 6), Vector3i(12, 14, 22), WOLF_DARK)    # the dark saddle
				_speckle(c, size, rng, 180, WOLF_LIGHT)
			else:
				_speckle(c, size, rng, 50, FOX_DARK, Vector3i(0, 6, 0))
			o = Vector3(-size.x / 2.0, 0, -size.z / 2.0)
		"fox/head", "wolf/head":
			var wolf := sp == "wolf"
			var k := 2 if wolf else 0   # wolves: a bigger head, a longer muzzle
			var fur := WOLF if wolf else FOX
			size = Vector3i(10 + k, 14 + k, 14 + 4 * k)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(10 + k, 8 + k, 8 + k), fur)
			_box(c, size, Vector3i(2, 0, 8 + k), Vector3i(8 + k, 4 + k, 14 + 4 * k), WOLF_LIGHT if wolf else FOX_WHITE)   # muzzle
			_box(c, size, Vector3i(2, 2 + k, 8 + k), Vector3i(8 + k, 4 + k, 14 + 4 * k), fur)
			_box(c, size, Vector3i(4, 2, 12 + 4 * k), Vector3i(6 + k, 4 + k, 14 + 4 * k), EYE)   # nose
			for x in [0, 6 + k]:
				_box(c, size, Vector3i(x, 8 + k, 2), Vector3i(x + 4, 14 + k, 4), WOLF_DARK if wolf else FOX_DARK)   # ears
				_box(c, size, Vector3i(x + (0 if x == 0 else 2), 4 + k, 7 + k), Vector3i(x + (2 if x == 0 else 4), 6 + k, 8 + k), YELLOW if wolf else EYE)
			o = Vector3(-(10 + k) / 2.0, -4, -2)
		"fox/tail", "wolf/tail":
			var wolf := sp == "wolf"
			if wolf:
				size = Vector3i(6, 8, 16)
				c = MobModel._cells(size)
				_box(c, size, Vector3i(0, 2, 0), Vector3i(6, 8, 16), WOLF)
				_box(c, size, Vector3i(0, 2, 0), Vector3i(6, 8, 4), WOLF_DARK)   # the tip
				_box(c, size, Vector3i(1, 0, 4), Vector3i(5, 2, 12), WOLF)   # it droops
				o = Vector3(-3, -6, -16)
			else:
				# a bushy brush: narrow root, fat middle, white tip, fluff sticking out of the middle
				size = Vector3i(10, 12, 22)
				c = MobModel._cells(size)
				_box(c, size, Vector3i(2, 4, 16), Vector3i(8, 10, 22), FOX)               # root
				_box(c, size, Vector3i(1, 2, 8), Vector3i(9, 11, 16), FOX)                # the thick of it
				_box(c, size, Vector3i(0, 3, 10), Vector3i(10, 9, 14), FOX)               # puffed out
				_box(c, size, Vector3i(2, 0, 6), Vector3i(8, 4, 14), FOX_DARK)             # dark underside
				_box(c, size, Vector3i(3, 4, 0), Vector3i(7, 8, 6), FOX_WHITE)             # white tip
				_speckle(c, size, rng, 30, FOX_DARK, Vector3i(0, 6, 8), Vector3i(9, 11, 18))
				o = Vector3(-5, -8, -22)
		"fox/leg", "wolf/leg":
			var wolf := sp == "wolf"
			size = Vector3i(4, 16, 4) if wolf else Vector3i(4, 10, 4)
			c = MobModel._cells(size)
			_box(c, size, Vector3i.ZERO, size, WOLF if wolf else FOX_DARK)
			_box(c, size, Vector3i.ZERO, Vector3i(4, 2, 4), WOLF_DARK if wolf else FOX_DARK)
			o = Vector3(-2, -size.y, -2)
		"duck/body":
			size = Vector3i(10, 12, 16)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 2), Vector3i(10, 10, 16), DUCK_BODY)
			_box(c, size, Vector3i(2, 6, 0), Vector3i(8, 12, 2), DUCK_BODY)          # the tail tip up
			_box(c, size, Vector3i(0, 6, 12), Vector3i(10, 10, 16), DUCK_HEAD)        # a green collar
			_box(c, size, Vector3i(0, 4, 12), Vector3i(10, 6, 14), FOX_WHITE)         # white ring
			_speckle(c, size, rng, 40, FOX_DARK, Vector3i(0, 0, 2), Vector3i(9, 5, 11))
			o = Vector3(-5, 0, -8)
		"duck/head":
			size = Vector3i(8, 14, 12)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(2, 0, 0), Vector3i(6, 6, 4), DUCK_HEAD)
			_box(c, size, Vector3i(0, 4, 0), Vector3i(8, 12, 8), DUCK_HEAD)
			_box(c, size, Vector3i(2, 4, 8), Vector3i(6, 8, 12), BILL)
			for x in [0, 6]:
				_box(c, size, Vector3i(x, 8, 4), Vector3i(x + 2, 10, 6), EYE)
			o = Vector3(-4, -2, -2)
		"duck/leg":
			size = Vector3i(2, 7, 4)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(2, 7, 2), BILL)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(2, 1, 4), BILL)
			o = Vector3(-1, -7, -1)
		"duck/wing":
			size = Vector3i(2, 8, 10)
			c = MobModel._cells(size)
			_box(c, size, Vector3i.ZERO, size, DUCK_BODY)
			_box(c, size, Vector3i(0, 0, 2), Vector3i(2, 2, 6), DUCK_HEAD)            # the speculum
			o = Vector3(-1, -8, -5)
		"frog/body":
			size = Vector3i(12, 8, 12)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 0), size, FROG)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(12, 2, 12), FROG_BELLY)
			_speckle(c, size, rng, 30, FOX_DARK, Vector3i(0, 4, 0))
			o = Vector3(-6, 0, -6)
		"frog/head":
			size = Vector3i(12, 10, 8)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(12, 6, 8), FROG)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(12, 2, 8), FROG_BELLY)
			for x in [0, 8]:
				_box(c, size, Vector3i(x, 6, 2), Vector3i(x + 4, 10, 6), FROG)        # bulging eyes on top
				_box(c, size, Vector3i(x + (0 if x == 0 else 2), 8, 4), Vector3i(x + (2 if x == 0 else 4), 10, 6), EYE)
			_box(c, size, Vector3i(2, 2, 7), Vector3i(10, 3, 8), FOX_DARK)            # the wide mouth
			o = Vector3(-6, -1, 0)
		"frog/leg":
			size = Vector3i(4, 4, 4)
			c = MobModel._cells(size)
			_box(c, size, Vector3i.ZERO, size, FROG)
			o = Vector3(-2, -4, -2)
		"frog/hind":
			size = Vector3i(4, 6, 10)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 2, 2), Vector3i(4, 6, 10), FROG)                # folded thigh
			_box(c, size, Vector3i(0, 0, 0), Vector3i(4, 2, 8), FROG_BELLY)
			o = Vector3(-2, -6, -5)
		"crab/body":
			size = Vector3i(16, 6, 12)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 0), size, CRAB)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(16, 2, 12), CRAB_DARK)
			_speckle(c, size, rng, 40, CRAB_DARK, Vector3i(0, 4, 0))
			o = Vector3(-8, 0, -6)
		"crab/head":
			# two big claws forward and eyes on stalks
			size = Vector3i(20, 10, 8)
			c = MobModel._cells(size)
			for x in [0, 14]:
				_box(c, size, Vector3i(x, 0, 2), Vector3i(x + 6, 6, 8), CRAB)
				_box(c, size, Vector3i(x, 2, 6), Vector3i(x + 6, 4, 8), CRAB_DARK)    # the pincer gap
			for x in [6, 12]:
				_box(c, size, Vector3i(x, 2, 0), Vector3i(x + 2, 8, 2), CRAB_DARK)
				_box(c, size, Vector3i(x, 8, 0), Vector3i(x + 2, 10, 2), EYE)
			o = Vector3(-10, -2, 0)
		"crab/leg_l", "crab/leg_r":
			size = Vector3i(6, 8, 2)
			c = MobModel._cells(size)
			var l := part == "crab/leg_l" or part == "leg_l"
			_box(c, size, Vector3i(0, 6, 0), Vector3i(6, 8, 2), CRAB)                 # out, then down
			_box(c, size, Vector3i(4 if l else 0, 0, 0), Vector3i(6 if l else 2, 8, 2), CRAB_DARK)
			o = Vector3(0 if l else -6, -8, -1)
		"fish/body":
			size = Vector3i(6, 8, 20)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 0), size, FISH)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(6, 2, 20), FISH_BELLY)
			_box(c, size, Vector3i(0, 6, 0), Vector3i(6, 8, 20), FISH_FIN)            # dark back
			_box(c, size, Vector3i(2, 8, 6), Vector3i(4, 8, 12), FISH_FIN)
			_speckle(c, size, rng, 24, FISH_FIN, Vector3i(0, 3, 0), Vector3i(5, 6, 19))
			o = Vector3(-3, 0, -10)
		"fish/head":
			size = Vector3i(6, 6, 4)
			c = MobModel._cells(size)
			_box(c, size, Vector3i.ZERO, size, FISH)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(6, 2, 4), FISH_BELLY)
			for x in [0, 4]:
				_box(c, size, Vector3i(x, 2, 0), Vector3i(x + 2, 4, 2), EYE)
			o = Vector3(-3, 0, 0)
		"fish/tail":
			size = Vector3i(2, 10, 8)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 2, 4), Vector3i(2, 8, 8), FISH_FIN)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(2, 4, 4), FISH_FIN)             # forked
			_box(c, size, Vector3i(0, 6, 0), Vector3i(2, 10, 4), FISH_FIN)
			o = Vector3(-1, -5, -8)
		"bat/body":
			size = Vector3i(8, 8, 8)
			c = MobModel._cells(size)
			_box(c, size, Vector3i.ZERO, size, BAT)
			_speckle(c, size, rng, 20, BAT_WING)
			o = Vector3(-4, 0, -4)
		"bat/head":
			size = Vector3i(8, 10, 6)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(8, 6, 6), BAT)
			for x in [0, 6]:
				_box(c, size, Vector3i(x, 6, 2), Vector3i(x + 2, 10, 4), BAT_WING)    # ears
				_box(c, size, Vector3i(x, 2, 5), Vector3i(x + 2, 4, 6), EYE)
			_box(c, size, Vector3i(2, 0, 5), Vector3i(6, 2, 6), PINK_DARK)
			o = Vector3(-4, -1, -2)
		"bat/leg":
			size = Vector3i(2, 4, 2)
			c = MobModel._cells(size)
			_box(c, size, Vector3i.ZERO, size, BAT_WING)
			o = Vector3(-1, -4, -1)
		"bat/wing_l", "bat/wing_r":
			# a membrane on finger bones, spread wide
			size = Vector3i(16, 2, 12)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 2), Vector3i(16, 2, 10), BAT_WING)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(10, 2, 2), BAT_WING)
			for z in [3, 7]:
				_box(c, size, Vector3i(2, 0, z), Vector3i(16, 2, z + 1), BAT)
			var l := part == "wing_l"
			o = Vector3(0 if l else -16, -1, -6)
			if not l:
				c = _mirror_x(c, size)
		"jellyfish/body":
			size = Vector3i(14, 10, 14)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(14, 6, 14), JELLY)               # the bell
			_box(c, size, Vector3i(2, 6, 2), Vector3i(12, 9, 12), JELLY)
			_box(c, size, Vector3i(2, 6, 2), Vector3i(12, 7, 12), JELLY_LIGHT)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(14, 1, 14), JELLY_DARK)          # the shaded rim
			_box(c, size, Vector3i(5, 2, 5), Vector3i(9, 6, 9), JELLY_HEART)          # the glowing heart inside
			o = Vector3(-7, 0, -7)
		"jellyfish/head":
			size = Vector3i(8, 2, 8)
			c = MobModel._cells(size)
			_box(c, size, Vector3i.ZERO, size, JELLY_LIGHT)                            # the lit crown of the bell
			o = Vector3(-4, 0, -4)
		"jellyfish/leg":
			size = Vector3i(2, 14, 2)
			c = MobModel._cells(size)
			_box(c, size, Vector3i.ZERO, size, JELLY_LIGHT)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(2, 6, 2), JELLY_DARK)            # the trailing ends fade darker
			o = Vector3(-1, -14, -1)
		"cave_salamander/body":
			size = Vector3i(8, 4, 18)
			c = MobModel._cells(size)
			_box(c, size, Vector3i.ZERO, size, SALA)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(8, 1, 18), SALA_DARK)           # the shaded belly
			_box(c, size, Vector3i(0, 3, 0), Vector3i(4, 4, 18), SALA_LIGHT)          # lit from the top left
			for z in [2, 7, 12]:
				_box(c, size, Vector3i(1 + (z % 4), 3, z), Vector3i(3 + (z % 4), 4, z + 2), SALA_SPOT)
			_box(c, size, Vector3i(5, 3, 15), Vector3i(7, 4, 17), SALA_SPOT)
			o = Vector3(-4, 0, -9)
		"cave_salamander/head":
			size = Vector3i(6, 4, 6)
			c = MobModel._cells(size)
			_box(c, size, Vector3i.ZERO, size, SALA)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(6, 1, 6), SALA_DARK)
			_box(c, size, Vector3i(0, 3, 0), Vector3i(3, 4, 6), SALA_LIGHT)
			_box(c, size, Vector3i(2, 3, 1), Vector3i(4, 4, 3), SALA_SPOT)
			for x in [0, 5]:
				_box(c, size, Vector3i(x, 2, 3), Vector3i(x + 1, 3, 5), EYE)
			o = Vector3(-3, 0, 0)
		"cave_salamander/tail":
			size = Vector3i(4, 3, 14)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 8), Vector3i(4, 3, 14), SALA)
			_box(c, size, Vector3i(1, 0, 0), Vector3i(3, 2, 8), SALA)                 # it tapers
			_box(c, size, Vector3i(1, 1, 0), Vector3i(2, 2, 8), SALA_LIGHT)
			_box(c, size, Vector3i(1, 2, 9), Vector3i(3, 3, 11), SALA_SPOT)
			o = Vector3(-2, -1, -14)
		"cave_salamander/leg":
			size = Vector3i(2, 4, 4)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(2, 4, 2), SALA_DARK)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(2, 1, 4), SALA_DARK)             # splayed toes
			o = Vector3(-1, -4, -1)
		"crow/body":
			size = Vector3i(8, 8, 14)
			c = MobModel._cells(size)
			_box(c, size, Vector3i.ZERO, size, CROW)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(8, 2, 14), CROW_DARK)            # the shaded underside
			_box(c, size, Vector3i(2, 6, 2), Vector3i(6, 8, 12), CROW_LIGHT)          # sheen along the back, lit from above
			o = Vector3(-4, 0, -7)
		"crow/head":
			size = Vector3i(8, 8, 12)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(8, 8, 8), CROW)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(8, 2, 8), CROW_DARK)
			_box(c, size, Vector3i(2, 2, 8), Vector3i(6, 6, 12), BEAK)                 # the heavy beak
			_box(c, size, Vector3i(2, 2, 8), Vector3i(6, 3, 12), CROW_DARK)
			for x in [0, 6]:
				_box(c, size, Vector3i(x, 4, 5), Vector3i(x + 2, 6, 7), EYE)
			o = Vector3(-4, -2, -2)
		"crow/tail":
			size = Vector3i(6, 2, 10)
			c = MobModel._cells(size)
			_box(c, size, Vector3i.ZERO, size, CROW_DARK)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(6, 2, 2), CROW_LIGHT)            # pale tips
			o = Vector3(-3, -1, -10)
		"crow/leg":
			size = Vector3i(2, 6, 4)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(2, 6, 2), BEAK)
			_box(c, size, Vector3i(0, 0, 0), Vector3i(2, 1, 4), BEAK)
			o = Vector3(-1, -6, -1)
		"crow/wing_l", "crow/wing_r":
			size = Vector3i(18, 2, 12)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 0, 2), Vector3i(12, 2, 12), CROW)
			_box(c, size, Vector3i(12, 0, 0), Vector3i(18, 2, 10), CROW_LIGHT)        # spread flight feathers
			for x in [13, 15, 17]:
				_box(c, size, Vector3i(x, 0, 0), Vector3i(x + 1, 2, 10), CROW_DARK)    # the gaps between them
			_box(c, size, Vector3i(0, 0, 10), Vector3i(12, 2, 12), CROW_DARK)         # leading edge
			var lw := part == "wing_l"
			o = Vector3(0 if lw else -18, -1, -6)
			if not lw:
				c = _mirror_x(c, size)
		"cave_spider/body":
			size = Vector3i(16, 12, 18)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(1, 0, 0), Vector3i(15, 12, 18), SPIDER)
			_speckle(c, size, rng, 60, FOX_DARK)
			_box(c, size, Vector3i(6, 10, 4), Vector3i(10, 12, 14), SPIDER_EYE)       # a red mark on the back
			o = Vector3(-8, 0, -9)
		"cave_spider/head":
			size = Vector3i(10, 8, 8)
			c = MobModel._cells(size)
			_box(c, size, Vector3i.ZERO, size, SPIDER)
			for e in [Vector3i(2, 4, 7), Vector3i(6, 4, 7), Vector3i(0, 6, 7), Vector3i(8, 6, 7)]:
				_box(c, size, e, e + Vector3i(2, 2, 1), SPIDER_EYE)
			_box(c, size, Vector3i(2, 0, 7), Vector3i(4, 2, 8), BOAR_DARK)             # fangs
			_box(c, size, Vector3i(6, 0, 7), Vector3i(8, 2, 8), BOAR_DARK)
			o = Vector3(-5, -2, 0)
		"cave_spider/leg_l", "cave_spider/leg_r":
			# bent like a real spider's: out from the body, up to a high knee, then a long diagonal down
			size = Vector3i(18, 20, 2)
			c = MobModel._cells(size)
			_box(c, size, Vector3i(0, 12, 0), Vector3i(6, 14, 2), SPIDER)            # out from the body
			_box(c, size, Vector3i(4, 14, 0), Vector3i(8, 18, 2), SPIDER)            # up
			_box(c, size, Vector3i(6, 18, 0), Vector3i(12, 20, 2), SPIDER)           # over the knee
			_box(c, size, Vector3i(6, 17, 0), Vector3i(10, 20, 2), FOX_DARK)         # dark knee joint
			_box(c, size, Vector3i(10, 12, 0), Vector3i(14, 18, 2), SPIDER)          # down, a step at a time
			_box(c, size, Vector3i(12, 6, 0), Vector3i(16, 12, 2), SPIDER)
			_box(c, size, Vector3i(14, 0, 0), Vector3i(18, 6, 2), SPIDER)
			_box(c, size, Vector3i(15, 0, 0), Vector3i(18, 2, 2), FOX_DARK)          # the foot
			var l := part == "leg_l"
			o = Vector3(0 if l else -18, -12, -1)
			if not l:
				c = _mirror_x(c, size)
		_:
			push_error("WildModels: no part %s/%s" % [sp, part])
			return [MobModel._cells(Vector3i.ONE), Vector3i.ONE, Vector3.ZERO]
	return [c, size, o * s]


static func _mirror_x(c: PackedInt32Array, size: Vector3i) -> PackedInt32Array:
	var out := MobModel._cells(size)
	for z in size.z:
		for y in size.y:
			for x in size.x:
				out[(size.x - 1 - x) + size.x * (y + size.y * z)] = c[x + size.x * (y + size.y * z)]
	return out
