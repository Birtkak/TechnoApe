# Playing: keys, commands, scripted input

Moved out of CLAUDE.md (principle 15) so a feature's keys do not touch the file every session loads.

- Play: `technoape.exe run=play onscreen=1` (WASD, space; left click digs a block (in survival held, by its hardness, ADR 0034), right click places one (in survival with flint in hand, on rock, it strikes: 4 strikes knap a hand axe, `/give hand_axe`; the tool in hand digs its blocks in hardness / speed and wears out, ADR 0110; with the hand axe or a pick in hand right click on rock or ore prospects, once a second, a use each: the chat names the ore within its head's `prospect` metres, rich, strong, some or traces, or the rock rings empty; loose ore samples, nuggets and pebbles, lie over every body, taken by hand, ADR 0138);
  H the build hammer, B its shape, wheel its height; V the bucket, left scoops, right pours;
  in space the mouse pitches you freely and Q/E roll, gravity rights you as it pulls (ADR 0024); C held zooms 3x;
  1-9 or the wheel the hotbar; Tab the inventory (ADR 0074, 0075: survival the hotbar and a worn pack's rows, a click lifts and puts, right click halves or puts one, shift-click moves across, outside throws a pile; creative every block and item in tabs and a search, a click takes a stack; middle click picks a block in the world; dug blocks fall as piles, walked over to pick up; in survival rock and hard ore leave their rubble, a falling block dug again into the slots, ADR 0093; `/give basket`, `hide pack`, `frame pack` open 9, 18, 27 slots worn); N drops a waypoint and creative shows the compass, where you are and the waypoints
  (ADR 0030; Esc, Waypoints lists them, a click names, colours and sets where one shows; K hides them, ADR 0036); space swims up in water, ADR 0020; T chats, / runs a command: /time, /playmode, /give,
  /place <block> [face i j layer], /structure <name> [seed] (a file of data/structures five blocks ahead, ADR 0052),
  /spawn <creature>[:speed] (a row of data/creatures.toml walking in place ahead: this player's preview, ADR 0053; `/spawn <creature> live` a real animal, ADR 0083: herds by biome from `data/herds.toml`, bots' `report` lists the animals near), /goto terra|luna|sun, /timewarp (. and , step it), /heal, /feed [n], /hurt <points>
  (survival's health and hunger, ADR 0040: impacts hurt, hunger drains, the bars over the hotbar are
  survival's; at 0 health the death screen's `click:0` respawns), /help; /sound <name> plays a row of `data/sounds.toml` (ADR 0032:
  a recording `data/sounds/<name>.wav` replaces the code-made one; a painting `data/textures/<block>[_top|_side|_bottom].png`
  replaces a block's pattern, live within a second, a refused file named on stderr (ADR 0045; `textures=dir` reads another folder); a model `data/models/<name>.toml` (ADR 0047, voxels of 1/8 m)
  is drawn where a block row names `model =` and in hand for a block of its name, live the same way (`models=dir`; `model=name[:clip]` on any run
  stands one 3 m ahead; `run=models n=` the benchmark); a sound row may be made `from` bases (other rows, the `base.*` set, or
  `data/sounds/base/<name>.wav`) through a `chain` of steps with a per-take `spread` (ADR 0048, the file's header), and
  a running game makes a changed `data/sounds.toml` or `data/sounds/` again within a second (`sound_dir=dir` reads both from another folder); `sounds=1` prints every sound in numbers (its bases, chain, `ms`);
  off-screen runs are silent unless `sound=1`, and a play run's JSON counts what it played under `audio`); Tab completes a command, the arrows pick, else the up arrow
  recalls the last line; in creative (/playmode creative) space twice flies, the wheel its speed (not the hotbar) up to
  10 km/s (ADR 0073); F3 the overlay of
  numbers; F9 a bug report; F10 the playtest menu (an eclipse of the sun or of Luna, ADR 0036); Esc the menu: settings in pages (Display, Graphics, Controls, Keys, Sound,
  Accessibility), every key rebindable (Keys: grouped, searchable, a shared key red), kept in `settings.txt`, ADR 0023 and 0028; `dump=1` prints a
  settings file's meaning as JSON). `acts="t:action,..."`
  scripts those inputs off-screen (ADR 0017; `say:/time set noon`, `f3`, `f9`, `type:text`, `tab`, `enter`, `esc`, `click:1`, `point:x:y`, `C`/`c` the zoom, `next`/`prev`/`go`/`nudge+` the menu by keys, `bind:KeyW`, `key+:KeyW`/`key-:KeyW` a real key through the bindings;
  `lines=stdin` runs each line another program writes to the game as if typed and sent (the Studio's Try in game),
  `seconds=` ends a play run, `settings=out/x.txt` keeps the real settings out of it), and a
  filmstrip shows the result.
- Bots (`technoape-bot`): `prospect dx dy dz` strikes the prospecting tool in hand on that block and prints the answer
  as one JSON line, `{"bot":..,"prospect":[["copper","strong"],..]}` (empty: nothing in range); `hold k` first.
