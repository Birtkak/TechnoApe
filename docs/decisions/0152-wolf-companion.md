# ADR 0152: The tamed wolf is a companion

Status: accepted (2026-11-07)

## Context
Feeding raw meat to a wolf set `tamed_by` (stone_age.gd) but nothing used it: a tamed wolf still hunted
and bit its owner. The "fun pass" wants one companion: the tamed wolf only (no dogs, no other species).

## Decision
1. **`Mob.owner_id`** ("" wild, "player" the local player, a remote player's name later) is the whole state, saved
   with the mob (`owner`, `sit`). `Mob.set_companion(owner)` sets it, clears `aggressive` and provocation.
   `_think_goal` hands a companion to `_companion_goal` before any predator goal, so it never hunts, flees or
   bites its owner; `MobManager.attack` ignores the owner's own blows on it.
2. **Follow:** beyond 6 m it walks to the owner (3.2 m/s, 5.4 m/s past 9 m, 7.6 m/s past 14 m, above the player's
   7 m/s sprint), stops at 3 m. `safe_ahead` lets it into water and it swims at full speed. Left beyond 22 m, or
   6 s beyond 9 m (stuck), it teleports to a standable spot 2-3 m from the owner. A companion never goes dormant;
   one saved far from its owner (reload after a teleport) wakes beside them.
3. **Sit/stay:** an empty hand on it toggles (`MobManager.interact` returns "sit"/"follow"; a small bark/call and the
   model tips back on its haunches).
4. **Defend:** it fights, in order, what the owner last hit or what last hurt the owner (`MobManager.owner_foe`,
   10 s), else the nearest night creature or aggressive/provoked predator within 10 m of it or the owner. A
   bite is the predator data's (4 half-hearts / 0.9 s) through `MobManager.attack`, so kills drop loot. The
   bitten hostile gets `_grudge` (8 s) and fights back at the wolf, so it can be hurt and die (14 half-hearts).
5. **Warn:** a threat within 24 m gets a growl (`mob/wolf`) and the line "Your dog growls: gloam hound." once per
   threat per 25 s (`companion_warn`); the wolf looks at it.
6. **Heal:** +1 half-heart per 25 s; raw or cooked meat held out heals 6 (`Mob.eat_meat`, used up only when hurt).
7. All numbers are data: `companion` block of the wolf in `mobs.json`. `/tame` (playtest) tames the wolf in aim.

## Name tag, line of sight (follow-up 2026-11-07)
- A `Label3D` over the companion shows its name ("Wolf" by default, saved as `pet_name`); `/name <text>` renames the
  companion in aim. A name tag *item* is not made (it needs a sprite and recipe).
- The growl samples the line to the threat every 2 voxels (`MobManager.line_clear`): nothing is reported through rock.

## Multiplayer (ADR 0145)
Mobs are still local to each client, so each client simulates its own player's companion. PROTO is 3: the 20 Hz
`_s_state` / `_c_state` relay carries `pet = [pos, yaw, speed, sitting, name]` (empty when none) and
`RemotePlayers.Avatar` draws it as a puppet wolf (eased, sit pose, name tag). `owner_id` is a player id; owners resolve
through `MobManager.owner_node`.
Remaining: the companion's fights and damage are not visible to others (only position, pose, name); a companion
is not in the server's save of that player (it lives in the client's mob file); a remote owner's hits do not
make the *local* simulation of their pet fight; no puppet pose for "defending".
