# 0040 – Health, hunger, impact and death

- **Status:** accepted
- **Date:** 2026-11-11

## Context
Survival needs a first, simple health and hunger (the developer, 2026-11-11): health that only checks
impacts for now, hunger that slowly goes down and a bit quicker when you move and dig, a death screen and a
respawn, and an overlay that shows survival from creative at a glance. Both are to grow deeper later. The
developer's correction the same day: it is impact damage, not fall damage. A slow landing from a Luna cliff
should cost little or nothing; hitting a surface fast should kill whatever the gravity.

## Options considered
1. Damage from the height fallen. Simple, but wrong under any pull but Terra's, and blind to a fast
   sideways hit.
2. Damage from the speed along the pull at landing. Holds under any gravity, but only for the ground.
3. Damage from the speed the body loses to something solid, in any direction (chosen).

## Decision
- **The server's, per player** (`engine::vitals::Vitals`, stepped in `server::live`): health and hunger
  0 to 20 points each, ten segments on the HUD, a half segment a point. Kept in the save; sent to the owning
  client only (`ToClient::Vitals`) when what the HUD shows changes. The client only draws them.
- **Impact.** `vitals::impact` reads each physics step from its two ends, with no change to `phys.rs`: the
  part of the velocity that points into what the step did not move (or the floor took away on a landing) is
  stopped, and the impact is the speed lost along it against the motion the body came in with. Pressing
  against a wall, or turning round on the floor, loses nothing. Points = v^2 / (2 G) - 3, rounded, with G
  Terra's surface pull (`phys::G`): up to 7.7 m/s is free, then a point per metre of a Terra fall, so
  21 m/s (23 m on Terra) kills from full. The rule is in speed, so a body with a weaker pull lands slower and
  is hurt less; once bodies have their own gravity, a Luna cliff of 20 m costs a point at most.
  Landing in water costs nothing (the water is asked only for a hit that would hurt).
- **Hunger** falls by 20 in one game day (1200 s) standing, twice as fast walking, and 0.05 per block dug.
  It is counted per physics step the player itself takes, which the server takes at real time, so a time warp
  does not starve anyone. Above 14 hunger health comes back a point per 8 s; at 0 it goes a point per 10 s,
  down to 1: hunger alone never kills while there is no food (`shortcut:` in `Vitals::step`).
- **Creative** has neither: nothing drains, nothing hurts (creative flight is immune for now: a default),
  and the bars are not drawn. Survival shows the bars instead of the word SURVIVAL over the hotbar.
- **Death** at 0 health: the server keeps the cause ("ape hit the ground at 24 m/s", "a wall", "a
  ceiling"), tells everyone in chat, steps the player with an input that moves nothing (`vitals::still`;
  its client does the same, so the prediction holds), refuses its edits, bucket, moves and /goto. The client
  opens the death screen (`Menu::Dead`): the cause, Respawn and To the title, by mouse and keys like the
  other pages; Esc does not close it. `ToServer::Respawn` puts the player at Terra's spawn with full health
  and hunger; the inventory is kept (nothing to lose yet). A dead player who leaves is dead when it comes back.
- **Until food exists:** `/heal`, `/feed [points]` (`/feed 0` empties hunger), `/hurt <points>` (survival
  only), so the overlay and the death screen can be tried.
- **The overlay** (`hud::vitals`): two panes of the HUD's glass over the hotbar, health left, hunger right,
  ten cells each lit by half, filled from each pane's outer end where its sign stands: a cross for health, a
  fuel cell that empties for hunger. A lost point flashes white for 0.6 s; health at 4 or less pulses and turns
  its corners the warning colour; empty hunger is outlined in it. It follows the interface scale and the high
  contrast setting. F3's Player group has Health and Hunger; a play run's JSON has `vitals`.
- **Sound:** `hurt` in `data/sounds.toml`, played by the client when its own health drops.
- **Saves:** version 5. A save of version 4 loads, its players at full health and hunger.
- **Bots:** a bot that dies respawns at once (the soak's and the bot crate's). The soak's bots switch between
  survival and creative at random, so a survival bot that drops off a flight can die; the replay still matches.

## Consequences
- CPU: per physics step of each player a few vector operations (`impact`) and a few float operations
  (`Vitals::step`); a water lookup and a pull only for a hit that hurts; per tick one comparison per player
  for the message. Per frame about 110 rectangles more in survival, built on the frame thread in
  microseconds. Not timed separately (other sessions were compiling).
- Survival can now walk fast horizontally only by falling; when thrusters or vehicles come, `impact` already
  counts a sideways hit, but creative stays immune until a taste call says otherwise.
- The scale (3 m free, 1 point a metre) is Terra's fall in metres, written as a speed; other harm (lava,
  air, vacuum) adds causes to `Vitals::hurt`.
- Review: not yet reviewed by a fresh reader (to do before merging into main, working principle 12).

## Added 2026-11-11: the developer on the look
Shown the first bars (ten two-half cells a row, a fuel cell for hunger): "no that is very ugly", then "keep
the colors and the health symbol but make the hunger symbol a stomach and change the bars", and of three
drawn bar styles, "B". **Changed:** each is one smooth bar that empties towards its sign (a green cross, an
orange stomach), with its number at the inner end; the stretch lost flashes, low health pulses with warning
corners, empty hunger is outlined. Four rows in the playtest menu (F10) show the bars, the warnings, the
death screen and an impact. The death screen's look has not been judged yet.

## Added 2026-11-11: the signs, and the review
- **The signs.** The developer on the stomach: "make the stomach icon look better", "not quite there yet",
  a picture of the kind he meant (outlined, lit, shaded: "like this but in our color"), "maybe have icons
  hold more pixels", "darken the outline pixels for the health symbol", "make sure icons fit inside their
  box". Both signs are now 28 px in four tones (a dark outline, the body, a light, a shaded side), drawn
  from shapes by `tools/hud_icons.py`, and their panes are 32 px high so they sit inside.
- **The review found, and this fixed:** a player that died part-way through a tick's steps kept walking for
  the rest of them; turning round in the step that lands added the turn to the impact (6 m/s down read as
  10): only what is lost along the body's up counts there, with a test seen failing first.
- **The review found, and left:** a sideways impact cannot hurt in survival today (walking is too slow to
  pass the safe speed and only creative flies), so its test only proves the measure; a ceiling never
  hurts; leaving creative in mid-flight keeps the speed, so the landing can kill; and a changed client can
  dodge an impact by asking for a move or for creative just before it, as it can use /heal: the server
  trusts its players until it has roles (the same shortcut as every command).
