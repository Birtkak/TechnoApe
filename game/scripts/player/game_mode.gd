class_name GameMode
extends RefCounted

## Game modes (ADR 0018): named rule sets that Player, Main, the HUD and the
## inventory screen consult. Names, ids and short forms follow Minecraft, so
## `/gamemode 1`, `/gamemode c` and `/gamemode creative` all work.

const SURVIVAL := "survival"
const CREATIVE := "creative"
const SPECTATOR := "spectator"   # flies through everything, touches nothing, nothing touches it (2026-10-25)

const RULES := {
	SURVIVAL: {"id": 0, "can_fly": false, "instant_break": false, "drops": true,
		"consumes_items": true, "takes_damage": true, "item_picker": false, "interacts": true},
	CREATIVE: {"id": 1, "can_fly": true, "instant_break": true, "drops": true,
		"consumes_items": false, "takes_damage": false, "item_picker": true, "interacts": true},
	SPECTATOR: {"id": 3, "can_fly": true, "instant_break": false, "drops": false,
		"consumes_items": false, "takes_damage": false, "item_picker": false, "interacts": false,
		"always_fly": true, "noclip": true},
}

const ALIASES := {
	"0": SURVIVAL, "s": SURVIVAL, "survival": SURVIVAL,
	"1": CREATIVE, "c": CREATIVE, "creative": CREATIVE,
	"3": SPECTATOR, "sp": SPECTATOR, "spectator": SPECTATOR,
}


## Mode name for what a player typed ("1", "c", "Creative"), or "" when unknown.
static func parse(text: String) -> String:
	return String(ALIASES.get(text.strip_edges().to_lower(), ""))


## One rule of a mode; unknown modes fall back to survival's rules.
static func rule(mode: String, key: String) -> bool:
	var r: Dictionary = RULES.get(mode, RULES[SURVIVAL])
	return bool(r.get(key, false))


static func names() -> Array:
	return RULES.keys()
