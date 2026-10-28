class_name PlayerStains
extends RefCounted

## Noita's stains on the player: touching a liquid coats you and the coat
## fades over time. One stain at a time: a new one washes the old one off
## (developer, 2026-10-23). Each one does something:
##   wet      water: puts fire out and keeps it off
##   bloody   blood: cosmetic, drips on the ground
##   oily     oil: catching fire while oily burns twice as long
##   toxic    toxic sludge: 1 half-heart every 2 s while it lasts
##   burning  fire or lava (an oily coat catches from a flame): 1 half-heart a second
## Engine-agnostic: the player feeds it the materials it touches each step and
## applies the damage it returns (when the game mode takes damage).

const KINDS := ["wet", "bloody", "oily", "toxic", "burning"]
## seconds a full coat takes to fade
const FADE_S := {"wet": 12.0, "bloody": 25.0, "oily": 30.0, "toxic": 10.0, "burning": 4.0}
## seconds between half-hearts of damage while the stain lasts
const HURT_EVERY_S := {"toxic": 2.0, "burning": 1.0}
## coat colours (sRGB) for the body overlay, HUD icons and drips
const COLORS := {
	"wet": Color(0.35, 0.55, 0.95),
	"bloody": Color(0.55, 0.03, 0.05),
	"oily": Color(0.12, 0.10, 0.07),
	"toxic": Color(0.45, 0.85, 0.2),
	"burning": Color(1.0, 0.5, 0.1),
}
const OILY_BURN_FACTOR := 2.0

var amount := {}        # kind -> 0..1
var _hurt_timer := {}   # kind -> seconds until the next half-heart


func _init() -> void:
	clear()


func clear() -> void:
	for k in KINDS:
		amount[k] = 0.0
		_hurt_timer[k] = 0.0


func has(kind: String) -> bool:
	return float(amount.get(kind, 0.0)) > 0.0


func active() -> Array:
	return KINDS.filter(func(k): return has(k))


## The stain a terrain voxel value puts on whoever touches it ("" = none). A partly full
## pool voxel (fluid level, ADR 0068) counts as its liquid: the sludge pool in bug report
## 2026-10-27_17-54-29 was all such voxels, so it never stained.
static func stain_of(material: int) -> String:
	match GameMaterials.material_of_voxel(material):
		GameMaterials.WATER, GameMaterials.WATER_STATIC:
			return "wet"
		GameMaterials.BLOOD:
			return "bloody"
		GameMaterials.OIL:
			return "oily"
		GameMaterials.TOXIC_SLUDGE, GameMaterials.TOXIC_SLUDGE_STATIC:
			return "toxic"
		GameMaterials.FIRE, GameMaterials.LAVA, GameMaterials.LAVA_STATIC:   # generated pools are LAVA_STATIC (bug report 2026-10-28_08-26-42)
			return "burning"
	return ""


## Coats the player with `kind` (as if it touched that liquid). The new stain
## replaces whatever was on before; fire does not catch on a wet coat.
func apply(kind: String) -> void:
	var full := 1.0
	if kind == "burning":
		if has("wet"):
			return
		full = OILY_BURN_FACTOR if has("oily") else 1.0
		full = maxf(full, float(amount["burning"]))
	if not has(kind):
		# catching fire hurts at once; toxic after its first interval
		_hurt_timer[kind] = 0.0 if kind == "burning" else HURT_EVERY_S.get(kind, 0.0)
	for k in KINDS:
		if k != kind:
			amount[k] = 0.0
	amount[kind] = maxf(full, float(amount[kind]))


## One step: coats from the touched materials, then fading and damage.
## Lava sets you alight even when wet (it boils the water off). Returns the
## half-hearts of damage this step.
func update(delta: float, touching: Array) -> int:
	for m in touching:
		var k := stain_of(int(m))
		if k == "":
			continue
		if GameMaterials.material_of_voxel(int(m)) in [GameMaterials.LAVA, GameMaterials.LAVA_STATIC]:
			amount["wet"] = 0.0
		apply(k)
	var dmg := 0
	for k in KINDS:
		var a: float = amount[k]
		if a <= 0.0:
			continue
		if HURT_EVERY_S.has(k):
			_hurt_timer[k] -= delta
			while _hurt_timer[k] <= 0.0:
				dmg += 1
				_hurt_timer[k] += HURT_EVERY_S[k]
		amount[k] = maxf(0.0, a - delta / FADE_S[k])
	return dmg


## Body tint: the stains' colours mixed by amount, alpha = how stained (0..0.6).
func tint() -> Color:
	var total := 0.0
	var c := Color(0, 0, 0)
	for k in KINDS:
		var a: float = amount[k]
		if a > 0.0:
			c += COLORS[k] * a
			total += a
	if total <= 0.0:
		return Color(0, 0, 0, 0)
	c /= total
	c.a = clampf(total, 0.0, 1.0) * 0.6
	return c


func summary() -> Dictionary:
	var out := {}
	for k in active():
		out[k] = snappedf(float(amount[k]), 0.01)
	return out
