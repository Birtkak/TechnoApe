extends GdUnitTestSuite

## ADR 0152: the tamed wolf as a companion. Brain decisions on a stub world (flat ground, y <= 0):
## follow, teleport when left behind, sit/stay, never hurts its owner, defends against a gloam hound
## and a hostile wolf (who fights back), warns once, heals on meat, saves and loads.

func _setup() -> Array:
	var main: Node = auto_free(load("res://test/companion_stub_main.gd").new())
	main.player = auto_free(load("res://test/companion_stub_player.gd").new())
	main.player.set_meta("player_id", "player")
	var mm: MobManager = auto_free(MobManager.new())
	mm.configure(MobManager.load_defs())
	mm.main = main
	return [mm, main]


func _mob(mm: MobManager, species: String, pos: Vector3) -> Mob:
	var m: Mob = auto_free(Mob.create(mm, species, mm.defs[species], false))
	m.position = pos
	mm._active.append(m)
	return m


func _tame(mm: MobManager, owner: Node3D, at: Vector3) -> Mob:
	var w := _mob(mm, "wolf", at)
	w.aggressive = true
	w.set_companion(owner)
	return w


func test_tamed_wolf_follows_and_teleports() -> void:
	var s := _setup()
	var mm: MobManager = s[0]
	var owner: Node3D = s[1].player
	owner.position = Vector3(0, 1, 0)
	var w := _tame(mm, owner, Vector3(Settings.m2v(10.0), 1, 0))
	assert_str(w.owner_id).is_equal("player")
	assert_bool(w.aggressive).is_false()
	w._think_goal()
	assert_str(w.goal).is_equal("follow")
	assert_float(w._speed).is_greater(Settings.m2v(2.0))
	owner.position = Vector3(Settings.m2v(11.0), 1, 0)   # close: it stops and watches
	w._think_goal()
	w._stop()
	w._think_goal()
	assert_str(w.goal).is_equal("idle")
	owner.position = Vector3(Settings.m2v(60.0), 1, 0)   # left far behind: back at the owner's side
	w._think_goal()
	assert_float(w.position.distance_to(owner.position)).is_less(Settings.m2v(4.0))
	assert_int(mm.stats["teleported"]).is_equal(1)


func test_sit_and_never_hurts_owner() -> void:
	var s := _setup()
	var mm: MobManager = s[0]
	var owner: Node3D = s[1].player
	owner.position = Vector3(0, 1, 0)
	var w := _tame(mm, owner, Vector3(Settings.m2v(10.0), 1, 0))
	assert_str(mm.interact(w, {})).is_equal("sit")
	w._think_goal()
	assert_str(w.goal).is_equal("sit")
	assert_vector(w._target).is_equal(Vector3.INF)
	assert_str(mm.interact(w, {})).is_equal("follow")
	w._think_goal()
	assert_str(w.goal).is_equal("follow")
	assert_bool(mm.attack(w, 3, owner.position)).is_false()   # the owner's blow does nothing
	assert_int(w.health).is_equal(14)
	assert_float(w._provoked_t).is_equal(0.0)


func test_defends_against_a_hound_and_is_bitten_back() -> void:
	var s := _setup()
	var mm: MobManager = s[0]
	var owner: Node3D = s[1].player
	owner.position = Vector3(0, 1, 0)
	var w := _tame(mm, owner, Vector3(Settings.m2v(2.0), 1, 0))
	var hound := _mob(mm, "gloam_hound", Vector3(Settings.m2v(6.0), 1, 0))
	hound.aggressive = true
	w._think_goal()
	assert_str(w.goal).is_equal("defend")
	assert_object(w._foe).is_same(hound)
	w.position = hound.position - Vector3(Settings.m2v(0.5), 0, 0)
	w._attack_cd = 0.0
	var hp := hound.health
	w._think_goal()
	assert_int(hound.health).is_less(hp)
	hound._invuln_t = 0.0
	hound._attack_cd = 0.0
	hound.position = w.position + Vector3(Settings.m2v(0.5), 0, 0)
	hound._think_goal()   # fights back at the companion, not the owner
	assert_str(hound.goal).is_equal("attack")
	assert_int(w.health).is_less(14)


func test_defends_what_the_owner_hits_and_warns_once() -> void:
	var s := _setup()
	var mm: MobManager = s[0]
	var owner: Node3D = s[1].player
	owner.position = Vector3(0, 1, 0)
	var w := _tame(mm, owner, Vector3(Settings.m2v(2.0), 1, 0))
	var hound := _mob(mm, "gloam_hound", Vector3(Settings.m2v(20.0), 1, 0))
	hound.aggressive = true
	w._warn_t = 0.0
	w._think_goal()
	assert_int(mm.stats["warned"]).is_equal(1)
	assert_str(w.goal).is_not_equal("defend")   # still far: growl, no charge
	w._warn_t = 0.0
	w._think_goal()
	assert_int(mm.stats["warned"]).is_equal(1)
	var boar := _mob(mm, "boar", Vector3(Settings.m2v(5.0), 1, 0))
	mm.main.player.position = owner.position
	mm.owner_foe["player"] = [boar, mm.game_time]
	hound.position = Vector3(Settings.m2v(40.0), 1, 0)
	w._foe = null
	w._think_goal()
	assert_object(w._foe).is_same(boar)


func test_meat_heals_slowly_and_saves() -> void:
	var s := _setup()
	var mm: MobManager = s[0]
	var owner: Node3D = s[1].player
	var w := _tame(mm, owner, Vector3(0, 1, 0))
	w.health = 4
	assert_bool(w.eat_meat()).is_true()
	assert_int(w.health).is_equal(10)
	w.health = 5
	w._timers(30.0)
	assert_int(w.health).is_equal(6)
	w.sitting = true
	var back: Mob = auto_free(Mob.create(mm, "wolf", mm.defs["wolf"], false))
	back.load_from(w.to_dict())
	assert_str(back.owner_id).is_equal("player")
	assert_bool(back.sitting).is_true()
	assert_int(back.health).is_equal(6)
	assert_bool(back.aggressive).is_false()
