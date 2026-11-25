# Player and feel

Movement, sprint and swimming, building like Lay of the Land, health, hunger, thirst, breath, death and its cost,
carrying, the first night, the journal and the event list, aiming at things, chat commands and the debug UI,
settings pages and key bindings, the HUD's look and navigation, the keys page and the playtest menu. What holds
today. Numbers in brackets are the ADRs (`archive/NNNN-*.md`) a decision came from; INDEX.md lists every number.

## The developer's calls

- A third-person view on F5, "front/back like minecraft" [0208].
- The character is animated, the other players have models too, and there are different species and coloured
  monkeys, one given at random to a client the first time it joins a universe [0209].
- Survival needs a first, simple health and hunger; damage is from **impact** (the speed lost to something solid, any
  direction), not from the height fallen: a slow landing from a Luna cliff costs little, a fast hit kills whatever
  the gravity [0040].
- Empty hunger kills, now that there is food [0210]. The bars: one smooth bar each that empties towards its sign
  (a green cross for health, an orange stomach for hunger), number at the inner end; icons 28 px in four tones; "keep
  the colors and the health symbol", the stomach redrawn until it looked right [0040].
- Mining and building like Lay of the Land: shape tools yes, physics on the world yes, blueprints yes, terrain
  sculpting no. Bare hands dig and place single blocks; the good building options belong to a build hammer [0017].
- Carrying, death, first night, journal (all the recommendation): the ape starts with the hotbar's 9 slots and packs add
  rows; death drops what the ape carried as a pile where it died, waking at its sleeping place, else the spawn, with
  full needs; the first night in the open hurts and does not kill on a temperate plain; each ape has its own journal
  [0047]. (also items-and-crafting.md)
- Four more calls [0047]: a dug block falls to the ground as a pile and is picked up by walking over it (against the
  recommendation of straight into the slots); a worn-out tool leaves nothing (its metal goes to a "worn away" row of
  the body's ledger); food has four groups (protein, vegetables and fruit together, dairy, carbohydrates), how they act
  being Claude's reading, whether a missing group should ever hurt is open; a known journal entry is exact, with the
  numbers of the ape's own try (left to Claude). (also items-and-crafting.md)
- The frame cap: play caps its frame rate at the screen's refresh rate; High is the default graphics preset [0028].
- A place is a body, latitude, longitude and height; compass and waypoints, creative's until survival earns them; the
  HUD's look: small letters in the font, one dark glass panel, little cubes in the hotbar, the world dimmed under a
  menu, rows moving in (all five proposals taken) [0030].
- The menus' look: "HUD brackets" (picked of three) [0023]. Bug-report keys: F9 [0023].
- Luna a fixed part of Terra, perfect eclipses, the 2 degree tilt, the wider totality (sun 0.95 of Luna), Luna's
  gravity from its radius, a 30 minute day, glow the night rather than move Luna, keep the stars true [0041].
  (also bodies-and-orbits.md, sky-weather-and-water.md)
- "Add a sprint mode": hold only, Left Ctrl, 1.5 times the forward walk (the numbers are calls) [0118]. Swimming
  "like Minecraft" [0118].
- Aiming at things: a click on the thing itself, left takes it, held right picks it up with the hand empty [0144].

## Movement

- Shared fixed-step physics in `crates/engine/src/phys.rs`, run by the server per tick and by the client per frame for
  its own player (prediction), the server's position winning [0007]. Radial gravity of ADR 0007 is replaced: every
  body pulls and the player has its own orientation (ADR 0024, see grid-and-planets.md / bodies-and-orbits.md).
- Walk 4.3 m/s (`phys::WALK`), the horizontal velocity is the wished one on the ground and in the air [0007]. Jump
  `phys::JUMP` 4.87 to 6.84 m/s so that it is 1.25 blocks on Terra at any planet size [0007, amended 0041, 0132].
  Terra's `world::G` is 19.62 m/s2; every body's pull follows it, Luna 3.7 times less [0132, 0041].
- The box is 0.6 x 0.6 x 1.8 m, eye 1.6 m (`WIDTH`, `HEIGHT`, `EYE`), aligned with the columns and layers of the face
  under the feet, tested exactly against every cell it overlaps in grid space (so a position the physics accepted
  stays free); cells past a face edge are found through space. Moved axis by axis in substeps of at most 0.25 m,
  each bisected to the first contact. Step-up climbs exactly one layer. The player's size in cells means its size in
  metres follows the grid [0007].
- Fixed step DT = 1/60 s on both sides, each carrying the leftover time; the same inputs give the same path at any
  frame or tick rate [0007].
- The server refuses a solid block in any cell a player's box overlaps [0007].
- Wire: the client sends `Input` (move, jump, look); the server steps it every tick until a `Move` (fly or teleport,
  taken as sent in creative). Numbered steps, replay and eased corrections: ADR 0019 (see multiplayer-saves-and-data.md)
  [0007]. `Move` is unchecked; survival must reject or limit it (open, with the operator role).
- A step carries the world time it was taken at (`Input::t`); the server steps it at that time when within two real
  seconds of its own clock (`phys::step_at`), and corrections replay each step at its own time [0023].
- The view's up stays the local up unless the look is straight up or down [0023].
- Creative flight: space twice within 0.3 s flies or lands; the wheel sets the speed (x1.25 a notch, up to
  `phys::FLY_MAX` 10,000 m/s); space rises, shift sinks. `Input.fly` and `Input.down` in the shared physics: no
  gravity, the box still collides, no step-up. The server zeroes `fly` for survival players [0023, amended 0073: the
  cap was 250 then 1 km/s].
- Teleports move the player at rest (`Server::put`); the client takes a correction of 2 m or more as one and looks
  the server's way, level [0023].
- C held zooms 3x (eased over about 0.1 s, the mouse 3x slower). Creative is `/playmode` only [0023].

### The third-person view on F5 [0208, 2026-11-25]
F5 (the action `person`, rebindable; the act `f5`) turns the view: first person, third person from behind, third
person from the front, first person again. The camera sits `CAM_REACH` 4 m from the eye along the look (behind it, or
ahead looking back), its up the eye's; a ray in 5 cm steps stops it 0.25 m short of the first solid block, so it never
looks through the ground. Only the camera moves: the aim, the dig and the place, what the server is sent, the body's
shadow and the held lamp stay the eye's; the world is streamed, heard and fogged from the camera. In third person the
hand, the held item and the HUD's block in hand are not drawn and the player's own figure is: the ape at rest, as
tall and as placed as its shadow's boxes (ADR 0119), turned to the look's heading; not drawn with the camera within
0.6 m of the eye. A spectator has no body (ADR 0202) and stays in first person whatever F5 says. The view is not kept
in the settings or the save: a session starts in first person. The crosshair stays in every view (in the back view it
is off the aim by the camera's 4 m).
Open: its head's pitch and what it holds. [amended 0209: the figure walks and the other players have figures]

### The players' looks and figures [0209, 2026-11-25]
A player is drawn as an ape of one of the **looks**: the models of `data/models` named `ape <species> <colour>`,
four species (gorilla, chimp, orangutan, mandrill) in five coats (brown, black, ginger, white, blue), painted by
`tools/paint_figures.py` on the plain ape's rig. A player's look is `looks::look_of(seed, name, n)`: a hash of the
universe's seed and the player's name over the looks in the order of their names, so it is as good as random, the
same on every client and at every visit, and another in another universe; no look is sent or saved (the server and
the wire were another writer's when this was made). With no looks in the data everyone is the plain `ape`.
The figures (in `run=play` only): every other player of the roster within 128 m is drawn in its look, and its shadow's boxes with it,
eased from where its figure was to where the newest roster puts it over `looks::SPAN` 1.1 s, a little over a roster, so a walker never stands in between (the
roster comes once a second, ADR 0117), a move of 30 m or more put, not walked. A figure plays `p.idle`, `p.walk`
or `p.run`, made by the walk maker (ADR 0053) from the `ape` row of `data/creatures.toml` at the player's own
walk and sprint speeds for the plain ape and every look; the clip's time is the metres walked over that speed, so
the feet do not slide; over 1.25 times the walk speed it runs; this player's own figure stands in the air or
flying. The bare arm of first person is the look's `arm_r`. `MAX_MQUADS` is 1 << 19 (8 MB) for the looks' quads.
Alternatives rejected: a tint in the shader over one model (colours outside the palette, ADR 0081; no species).
Open: the look kept in the server's save and sent with the roster (adding a look to the data moves the players'
looks until then); the roster at the tick's rate to those near (a figure is up to a second late, and whoever
moves walks, also a flyer or a swimmer); the head's pitch, the held item in the hand, the swing of a dig; the
inventory's figure is still the plain ape; a way to choose a look.

## Sprint

- Hold-only key "Sprint (hold)", default Left Ctrl, Keys > Moving, rebindable, `key.sprint=` in `settings.txt` [0118].
- Speed: the forward share of the walk times 1.5 (`phys::SPRINT`): 6.45 m/s straight ahead; strafe and back stay 4.3;
  a diagonal's forward share only. Applies walking (survival or creative), in the air too (a jump keeps the speed),
  with under 0.3 of the body in water. Not in creative flight, not dead (`vitals::still` clears it), not in a menu
  (keys are not held; losing the focus clears the held keys) [0118].
- `phys::step` returns whether the step sprinted (the sprint was applied and the body went a twentieth further than the
  walk's share would): a sprint into a wall, up a ledge or in deep water is none. Drain and the wider view go by it [0118].
- Drain: the walk's drain times 3 (`vitals::SPRINT_DRAIN`): 6x standing, a point every 15 s. No sprint at hunger 6 of 20
  or less (`vitals::SPRINT_FLOOR`) [0118].
- View: +8 degrees field of view, eased linearly over 0.15 s while the last predicted step sprinted. No reduce-motion
  setting exists, so it widens for everyone [0118].
- Wire: `Input::sprint` is `#[serde(skip)]` (so `Input`'s bytes are unchanged); `ToServer::Sprints { first, inputs:
  Vec<(Input, bool)> }` beside `Steps` and `ToServer::Sprint(Input, bool)` beside `Input`, appended at the end;
  `ToServer::steps` / `input` pick the new variant only when an input sprints; the server turns them back with the bit
  set (`ToServer::unpacked`) right after it logs the message, so the recorder writes the bit. Old recordings read as
  not sprinting [0118].
- The floor is exact: `You` carries the server's hunger to the bit after its last answered step; `vitals::gate`
  refuses the bit per step on both sides, the client predicting the server's hunger by `vitals::drained` / `rate`
  [0118].
- Bots: `sprint on` / `sprint off`; every `walk` after `on` sprints, and a `walk` looks at its target (pitch from the
  height difference). The soak's walkers set the bit on 30 % of inputs, drawn last. `acts=`: `key+:ControlLeft`.
  The state hash includes the pitch [0118].
- shortcut: a dig's hunger (`Vitals::dug`) is not predicted; a dig within a few steps of the floor can cost one
  correction [0118].
- A sprint pressed in the air speeds the air move too (no "began on the ground" state in `Player`) [0118].

## Swimming

All in `phys::step`, shared by server and prediction; no new field, message or save change [0118].

- Wading (on the bottom, head out): walk at `WALK * (1 - sub / 2)`, jumps as on land; a sprint still sprints under 0.3
  of the body in. Floating (head out, no stroke): buoyancy holds 0.8 of the body under, jump swims up at 2 m/s, the down
  key dives at 2 m/s [0118, building on ADR 0020].
- Under: feet more than `HEIGHT + UNDER` (2.0 m) under, no stroke, with hysteresis (a sinking body stays under to 1.9
  m, a rising one leaves at 2.1 m): no gravity or buoyancy; velocity eases at `WATER_EASE` to the ground's way at
  `SWIM_SLOW` plus `SWIM` up on jump, down on the down key (Shift, "Fly down, swim down"), else sinks at `SINK`.
- Stroke: sprint and forward with more than `STROKE_SUB` (0.6) of the body in (0.5 once swimming fast, off the ground):
  the velocity eases to the look, pitch included, at `SWIM_FAST`; strafe at `SWIM_SLOW`, the two together clamped to
  `SWIM_FAST`; at the surface the rise is clipped at the float line.
- Climbing out: jump held (or a stroke) in real water near the surface, look not down (pitch over -0.3), a wall ahead
  at the feet whose top is at or over the surface with room to stand within 1.5 m: up at least 2 m/s, at a ledge `CLIMB`
  6 m/s. Elsewhere a stroke slides along a wall; in a puddle jump is the old jump.
- Constants (each one in `phys.rs`, taste calls): `SINK` 0.6 m/s, `SWIM` 2 m/s, `SWIM_FAST` 4 m/s, `SWIM_SLOW` 2.15 m/s
  (`WALK / 2`), `WATER_EASE` 4 /s, `UNDER` 0.2 m, `STROKE_SUB` 0.6, `CLIMB` 6 m/s.
- A stroke that moved along the look is a sprint (drains the sprint's rate); a stroke against a wall does not. At the
  floor the server refuses the bit and the body swims slowly. Creative walking swims as survival (no hunger).
- `Input.down` is read only in flight, under water and floating with 0.3 of the body in. The camera stays upright (no
  prone view, roll or bob); a prone pose for other players is a later model step.
- Known: the client's water levels can lag the server's by up to a tick while water flows, so near a moving surface a
  step's mode, `sprinted` and hunger drain can differ for a step; the next `You` reconciles it.
- Recordings: a recording whose players went under water, swam at a wall with jump or held down while floating replays
  differently from the first such step on; others replay to the same hashes.

## Building like Lay of the Land

- Shape tools (the box: block, brush, line, wall, floor; the cylinder; the cone): pick a shape, set a size, a ghost
  shows it, a click digs or fills it. Shapes go on the wire as shapes (ADR 0014) and the shared rule decides their cells
  [0017].
- Bare hands dig and place single blocks. The build hammer places shapes: right-drag; B cycles box, cylinder, cone (was
  Q, moved by ADR 0024 [0017, amended 0024]); a box spans the drag; a cylinder or cone is centred where the press was
  with the drag as radius and grows up from the ground; the wheel sets its height in blocks (8 by default) while the
  hammer holds one; white ghost for where, green while dragging; digging stays one block. The 1 m brush stays in the
  engine (`edit::brush`) for later tools [0017, 0036].
- Aim 8 m (the client aims; ADR 0144 for things). Hotbar: nine slots, held counts in survival, a gold strip in creative
  [0017].
- World physics (loose blocks fall and slide, unsupported builds collapse) and blueprints (capture a volume, name it,
  paste it, materials from storage) were decided yes; they are built as their own systems (ADR 0079 etc., see
  mining-and-digging.md / items-and-crafting.md). Terrain sculpting: no [0017].
- The server accepts any shape from anyone like the creative switch; it checks for a hammer once items exist [0017].
- `acts=` scripts the same inputs, e.g. `acts="1:c,2:h,2.2:b,3:R,3.1:m:-200:0,4:r"` [0017].

## Health, hunger and impact

- Server's, per player (`engine::vitals::Vitals`, stepped in `server::live`): health and hunger 0 to 20 points, ten
  segments on the HUD, a half segment a point. Kept in the save; sent to the owning client only
  (`ToClient::Vitals`) when what the HUD shows changes; the client only draws them [0040].
- Impact: `vitals::impact` reads each physics step from its two ends, no change to `phys.rs`: the part of the velocity
  pointing into what the step did not move (or the floor took away on a landing) is stopped, and the impact is the speed
  lost along it against the motion the body came in with. Pressing a wall or turning on the floor loses nothing; only
  what is lost along the body's up counts when turning in the landing step. Points = v^2 / (2 G) - 3, rounded, with G
  Terra's surface pull (`phys::G`): a point per metre of a Terra fall after 3 m free. With G = 19.62 the free landing
  is 10.8 m/s and 30 m/s kills from full [0040, 0132]. A weaker pull lands slower and is hurt less. Landing in water
  costs nothing [0040].
- Hunger falls by 20 in one game day (1800 s) standing, twice as fast walking, 0.05 per block dug, counted per physics
  step the player itself takes (a time warp starves nobody). Above 14 hunger health returns a point per 8 s; at 0 it
  goes a point per 10 s, to death ("<name> starved") [0040, 0041, 0210].
- Thirst (a third need): 0 to 20 in `Vitals`, drained per physics step at hunger's rate (`vitals::rate`: a walk twice,
  a sprint six times): empty in 1,200 s standing, 600 s walking. Empty: a point of health every 10 s to death ("<name>
  died of thirst"), no healing while empty. An ape that never drinks nor eats dies at 1,400 s (unit test). `/thirst [n]`
  sets it [0145].
- Drinking: the use (place) button held with nothing in hand on the water cell before the aimed block, within 6 m
  (`items::BENCH_REACH`), alive, in survival: empty to full in 2 s. The sea gives nothing. Only a truly empty hand
  drinks (client and server); death and respawn let go of a held drink; the server judges a cell fresh once, when the
  held cell comes [0145].
- Salt is told from generation, no new water state: a cell is salt when its column's generated water is the open sea's
  (`World::salt`, `Ground::reach == SEA`); the bake's rivers and lakes and poured water are fresh [0145]. Known
  misreadings: a pit beside the sea filled by it reads fresh; the tidal edge flips with the tide; water poured above a
  sea column's surface reads salt [0145].
- The use button's order: a block's window (ADR 0140), then a pick-up aimed at by its own faces (0144), then water in
  reach drinks (0145), then claiming a bed (0148) (`play::claims`) [0145, 0148].
- Breath, one condition with three causes: `vitals::Breath` (0 to 20, a fraction kept) fed one `Option<Choke>` a step
  (`Buried`, `Water`, `Space`). The server judges the eye (`pos + up * EYE`): a solid block is `Buried`, else water over
  it (`World::water_surface`) is `Water`, else more than `AIR_TOP_M` (1,500 m, `world::AIR_TOP_M`, the height where the
  client's sky already thins the air to nothing) over the nearest body that has air (`Frames::over_air`) is `Space`.
  Luna has no air, so a survival ape on Luna runs out of air [0146, building on 0079's buried breath].
- Lengths: buried 10 s, water and space 30 s; then a point of health a second (`CHOKE`) to death ("<name> was buried",
  "<name> drowned", "<name> ran out of air"). Air restores none to full in 2 s and the second's count starts again
  (replaces ADR 0079's instant reset). Creative and a respawn reset it to full. Not saved (lives in the server's
  `Client`, a load starts full); not in the state hash [0146].
- Wire: `ToClient::Vitals { health, hunger, thirst, breath: u8 (whole points, rounded up), dead }`, sent when it
  changes [0145, 0146]. The breath HUD pane over health's, on the left, only while the breath is under 20; F3's Player
  section shows it; the run JSON's `vitals` and bots' `report` carry it as a fourth number [0146].
- Creative has neither need: nothing drains, nothing hurts (creative flight is immune), the bars are not drawn [0040].
  Survival shows the bars instead of the word SURVIVAL over the hotbar [0040].
- Until food exists: `/heal`, `/feed [points]` (`/feed 0` empties hunger), `/hurt <points>` (survival only) [0040].
  Low thirst's "slower digging", heat's "half again faster" (warmth, needs step 5), a mouthful taking water from the
  ledger, and a salt bit per column when water is carried far are not built [0145].
- The overlay (`hud::vitals`): two panes of the HUD's glass over the hotbar, health left, hunger right, 32 px high,
  smooth bars as above; a lost point flashes white for 0.6 s; health at 4 or less pulses and turns its corners the
  warning colour; empty hunger is outlined in it. Follows the interface scale and high contrast. F3's Player group has
  Health and Hunger; a play run's JSON has `vitals`. The signs are drawn by `tools/hud_icons.py` [0040].
- Sound `hurt` in `data/sounds.toml`, played when the client's own health drops [0040].
- Bots that die respawn at once; the soak's bots switch survival and creative at random [0040].
- Open: a sideways impact cannot hurt in survival today; a ceiling never hurts; leaving creative in mid-flight keeps the
  speed; the server trusts a changed client (it can dodge an impact by asking for a move) until it has roles [0040].

### Eating [0210, 2026-11-25]
- An item's row in `data/items.toml` may say `feeds = n`: the hunger points one fills (0 to 20; 0 or left out is not
  food; a row over 20 is refused). The first foods are what lies on the ground: berries 2, mushroom 3 (raw forage
  feeds little, needs.md "Food"; to tune in play).
- The place button with a food in hand eats one (`play::food_in_hand`, after a block's window, a pick-up, a drink and
  a bed in the use button's order, before the bucket, knapping and a place). `ToServer::Eat`, appended to the enum:
  the server eats one of the hand's slot when alive, in survival, the item feeds and hunger is under full; else
  nothing. Hunger rises by `feeds`, to full at most; the items' ledger counts the one eaten. Answered by the usual
  `Vitals` and `Inventory`. Bots: `eat`; the soak's bots eat at random and are given berries.
- With food in the world empty hunger takes health a point per 10 s to death, "<name> starved" (the shortcut of 0040
  is gone). An ape that neither eats nor drinks still dies of thirst first (1,400 s).
- Alternatives rejected: a friend's draft (2026-11-25: 0-100 float vitals, its own journal and death cost, Rust-coded
  berry spawns) duplicated `Vitals`, the data journal and the clutter that already lays berries.
- Open: food groups, spoiling, cooking doubling `feeds`, a sound and a bite's animation, the journal's first meal,
  holding the button to eat on; the creatures that give meat (the next step: hurting animals).

## Death and what it costs

- At 0 health the server keeps the cause ("ape hit the ground at 24 m/s", "a wall", "a ceiling"), tells everyone in
  chat, steps the player with an input that moves nothing (`vitals::still`, the client does the same), refuses its
  edits, bucket, moves and `/goto`. The client opens the death screen (`Menu::Dead`): the cause, Respawn and To the
  title; Esc does not close it. A dead player who leaves is dead when it comes back [0040].
- `ToServer::Respawn`: at the sleeping place, else Terra's spawn, full health and needs [0047, 0148]. The inventory is
  not kept: everything carried is laid as a pile where the body lies (`Server::lay`), at the respawn, not the moment of
  death (ADR 0074's piles) [0148, replaces 0040's kept inventory]. Out of a block it goes upwards; in water it falls
  to the bed; in the air at most `FALL` (64) layers, so a death in the air or space leaves a floating pile. The items'
  ledger holds: nothing lost or made. Anyone may take from it. Nothing despawns [0148, 0047].
- The server keeps two marks per player (`save::Marks`: the death pile's cell list, the sleeping place) and sends
  `ToClient::Marks { death, home }` (message tag 21, appended last, pinned by a test). The client draws them beside its
  waypoints [0148].
- Death mark: the cells the death's things went into, lowest first; a cell leaves the list once its pile is gone (a pile
  laid there later does not bring it back); the mark goes when the list is empty; when a pile falls or is lifted
  (`resettle`) its cells move with it in every player's list. Only the last death that laid anything is marked. The
  client gets the lowest cell, drawn as a "Death" waypoint, bone white, plus icon, visible from any body [0148].
- Sleeping place: a bed of leaves (any `* leaves` block) with a torch in one of the 26 cells around it and room to stand
  on it; the two cells over the bed hold no block and the eye of an ape standing on it is out of water. The cooking fire
  (needs step 4) joins `vitals::sleeping_place` when it comes. Claimed by the use button (right click) with an empty
  hand, last in the button's order, within reach and with nothing solid on the straight line from the eye to where the
  ape would stand (a line, not the server's aim, since the server holds the look a tick behind). No sleep and no night
  skipped (a shared world has one clock). Answers "you will wake here" or "no sleeping place: ..."; the HUD shows a
  green "Home" waypoint. A respawn wakes the ape with its feet on the bed (`vitals::wake_at`) while it qualifies; else
  Terra's spawn and Home is forgotten. Works on Luna; two players may share a bed [0148].
- Creative loses nothing: it cannot die (`/hurt` refused, breath reset), so it leaves no pile [0148].
- A workbench's grid goes into the ape's pack at death (`uncraft`); a crucible's slots stay the crucible's (death only
  closes its window); no item on a cursor on the server. An ape that leaves while dead keeps its things in the save with
  its name and lays them when it returns and respawns [0148].
- Save version 18 keeps both marks; 17 and older load with neither [0148].
- Old clients cannot join a new server (and the reverse): `Vitals` gained fields and `Marks`, `Events`, `Journal` are
  new messages, and the protocol has no version; the friends' client is updated with the deploy (ADR 0130) [0145, 0146,
  0148, 0149, 0150].
- Open: rooms and habitats as sleeping places; the globe map's view of the marks; a waypoint shared in chat; effects and
  sounds at a death or its pile (a shortcut names the journal events that should raise them) [0148].
- Open (the first night, a design call, unbuilt in 0047): cold on the first night in the open drains hunger faster and
  costs some health on a temperate plain, a fire or cave makes it free, the poles and high ranges kill at any hour. A
  scripted bot's first night is its test [0047].

## Carrying

- The ape starts with the hotbar's 9 slots; worn gear adds rows: a woven basket 9, a hide pack 18, a frame pack 27.
  Stacks are Minecraft's (64 by default) [0047]. (also items-and-crafting.md)
- A dug block falls to the ground as a pile and is picked up by walking over it; a dug shape gathers its drops into
  few piles. A worn-out tool leaves nothing; its metal goes to a "worn away" ledger row [0047].
- Food has four groups; a variety reward with no penalty is only proposed (`needs.md`); whether a missing group hurts
  is open [0047].
- Open: warmth tuned against one sentence, the sleeping place a thing to claim [0047].

## Aiming at things

- The client aims, the server checks what it is named (as for a dig); no wire change. `clutter::aim_thing` (engine,
  beside `edit::aim`) walks the cells as `edit::aim` does and in each air cell with a not-taken pick-up (`pickup_at`)
  tests the thing's own faces as drawn (`clutter::thing_faces`, turned and set as shader_models.wgsl `still_local`
  does); a miss goes on to the block behind [0144, amending 0082].
- A thing is aimed at only when a click would take it (not with the bucket in hand, not when its items do not fit, not
  when its take is on its way) [0144].
- With no button held the aimed thing's face edges take the block outline's place (`hud::thing_ghost`) [0144].
- Left click takes it at once, whatever is in hand. Right click held `PICK_HOLD` 0.3 s on the same thing picks it up only
  with the hand empty (no hotbar item, hammer or bucket); looking off, letting go or taking something cancels. With
  anything in hand right click does what it did (place, strike, pour, open a window). Both send `ToServer::Take {
  seq, at }`; one click one take [0144].
- The frame thread asks only height tiles already built (`Body::tile_if_built`); `Body::tile` is for workers and the
  server [0144]. The server checks `Take` by reach, that one lies in the named cell, not taken, fits [0144, 0082].
- shortcut: a face reaching over its cell's side is hit only from within its cell [0144].
- Open: whether right click should also gather by sweeping while held; thinner outline for tiny things [0144].

### Felt temperature, and the look-at panel names fluids with heat and hardness [0199, 2026-11-24]
`World::felt(point, live)` (engine `world/felt.rs`) is the temperature felt at a point: the operative temperature
(ASHRAE 55, ISO 7726). The air there is `Ground::air` (above ground the weather's live `Air.temp` the server sends,
else the baked climate with the season and the hour; under ground the rock's blended in with depth). The mean radiant
temperature is a globe's (ISO 7726: a small sphere, a standing body) by rays: `RAYS` 384 directions evenly over the
sphere (a Fibonacci lattice fixed in the world's frame, so a still eye's value is still), each marched cell to cell (the
grid taken as straight over a centimetre to find the next wall: each cell looked at once) out to `RAY_REACH` 48 m or
`RAY_CELLS` 96 cells. A ray stops at the first block that is solid and opaque (it sees the surroundings: nothing) or
at the first cell holding a glowing fluid (a crater's lake or stored lava), which it sees at its row's `heat.surface`
where it has one (lava 375 C: a crust with hot cracks, a flux near 10 kW/m2), else at the fluid's own temperature;
water, glass, leaves and plants let it through. T_mrt^4 = T_s^4 + (1/N) sum over hits of eps (T_hit^4 - T_s^4), T_s
the air's: view factors, what hides what, a hemisphere's 0.5 and a sum at or under 1 follow from the count, with no
clamp and no cosine. felt = (h_c T_air + h_r T_mrt) / (h_c + h_r), h_r = 4 eps sigma T_m^3 at the mean of the two
(eps 0.95), h_c 3.1 W/m2K (a person at rest in still air, ASHRAE Fundamentals ch. 9). In a glowing fluid (an eye in
lava) felt is the fluid's own temperature, not its crust's. On a body without air T_s is the ground's temperature at the
surface under the point (`Body::rest_heat` at the column's top cell; `liquid::AMBIENT` without a map), h_c is 0 and felt
is T_mrt. Sources add up by what they fill of the view: 2 columns beside a stream of lava 1 wide and 10 long about 52 C
by hand (F 0.042, 16 of 384 rays; by rays 46 to 48 C along it), half a metre over a floor of lava filling the lower
half of the view 270.6 C mean radiant by hand (by rays 264.0 C, the planet's curve taking the floor's far
edge under the rays nearest the horizon); the air over lava is the cell's own (no plume yet). Fire (0191) adds its cells as hits in the one
marked place once it is built.
The client marches the rays a few a frame (`World::march`: whole rays while the cells looked at stay within
`FRAME_PROBES` 400 a frame, a set in a few dozen frames), only in run=play where the HUD shows it, and works the value
out from the last whole set every `SENSE_S` 0.25 s; a set begun more than 8 m from the eye is begun again, and one from
more than 32 m forgotten (after a teleport or at the spawn the value is the air's alone until a set is whole). A
benchmark run marches none in its frames: its JSON's `felt` and `look` are worked out once at its end, all rays at once
(`World::felt`); a bot's `report` likewise.
The HUD shows it after the compass line's place text as `12.4 °C` (the HUD font's degree sign is the cell after '~', glyph 127, `hud::DEGREE`); F3's World group a
`Felt` row (the air and the mean radiant temperature); the run's JSON `"felt":{temp,air,mrt}` and `"look":{name,note}`;
a bot's `report` `"felt"`.
The look-at panel's lines are made again in the frame the aimed cell or its block changes (`edit::aim`, every frame as
before), else every `SENSE_S`, name and note together, so the panel follows the crosshair and never shows one target's
name with another's note. Its target is `edit::look`: `edit::aim`'s walk that stops also at the first step into a fluid other
than the one the eye is in (the sea, a river, a lake, a crater's lava, stored water, lava, oil). A block's lines: its
name; its temperature (in the generated ground under its top cell the rock's `rest_heat`, the top cell and what is
over it the air's there; a fluid's frozen block at most its freeze point), `hardness` and `tool` of its
`data/blocks.toml` row, light and metal. A fluid's: its row's name (`Sea water` where `World::salt`), its own
temperature (water by its place, not under its freeze point; generated fluid by `rest_heat`, a lake's lava its
chamber's 1150 C, not its crust's; stored lava and oil at the node's heat where kept, else the row's pour heat) and its
depth (none for a crater's lake, whose lava runs on down its conduit).
No wire change: a client keeps no node's heat, so stored lava reads its pour heat until it freezes.
- shortcut: a wall blocks all radiant heat and passes none by conduction, until blocks' rows carry conductivity, heat
  capacity and emissivity (the gases-and-fire step: insulation, rooms holding heat).
- shortcut: a hot surface beyond 48 m is not seen: a crater's lake beyond it adds about 2 C (17 C felt at 60 m from a
  lake's edge by the lake rule before rays, air 15 C).
Open: a message for the looked-at node's heat if the pour heat misleads; the felt value feeding body temperature.

## The journal

- The event list (journal step 1) [0149]: `engine::events::Event`, a typed enum; `ToClient::Events(Vec<Event>)` is
  variant 22 (after `Marks`), sent once a tick before that tick's `Edits`, cut into parts of at most `EDITS_PART`
  (100,000). Kinds in wire order, pinned in `events::tests`: 0 `Edit { at, was, now, by: Option<player> }`, 1 `Fall
  { from, to, block }`, 2 `Worn { item, by }`, 3 `Taken { at, row, by }`, 4 `Craft { recipe, n, by }`, 5 `Fire { at,
  on }`, 6 `Died { by }`, 7 `Laid { at, by }`. A new kind is added at the end; a shipped field never changes.
- Raised in the tick's order by the systems that decide: `Edit` by `edit` (every changed cell, drops and rubble
  included), `/place`, `/structure`, the unheld torch, the torch water put out, a plant cut; `Fall` per move of
  `Falls::step`; `Worn` by `edit` and `prospect`; `Taken` by `take`; `Craft` by `craft`; `Fire` by `furnaces` beside
  `Fires`; `Died` where a death is announced; `Laid` at the Respawn arm. Every changed cell sent in `Edits` has an
  `Edit` or `Fall` before it; a joiner's whole list and a save's load edits have none [0149].
- Who gets them: every `Edit` and `Fall` goes to every client; the others go to clients within `PERCEIVE` (80 m, the
  animals' `SEE`) and the player itself (an event that happened to a player counts as where that player is) [0149].
- Client: `fx::Pending` holds events until their edits arrive; an event pairs with the edit that makes its cell what it
  says (a falling stack moves lowest first); `fx::server_edits`. An `Edit` plays its break or place (32 effects a frame,
  the first three per message tried for a sound); a `Fall` glides in with grains and the slump's hiss; other cells change
  silently; `Worn` for this player plays the tool's snap and chips; the own dig and place are still played at the click
  (`Predictor`). The pair heuristic is gone [0149].
- Not in the recording or the state hash (a pure result of the tick) [0149]. Not moved onto the list: predicted effects,
  the prospecting knock, animal voices, `Fires`, hurt from `Vitals`, thunder, UI sounds [0149].
- Journal data (step 2) [0150]: `data/journal.toml`, built into the binary (`engine::journal::JOURNAL`), a row an entry:
  `name`, `age`, `title`, `question`, `text`, `noticed_by`, `known_by`, `points_at`. Checked at load, failures naming the
  entry: names unique; `age` one of progression's ages (steam, lv, mv, hv, ev, iv, exotic); title, question and text
  present; at least one `known_by`; every `points_at` another entry; every trigger of a built kind naming what exists:
  `look:block:<block>` and `have:<item>` (names as `data/items.toml` writes them, `_` refused). `do:`, `see:`, `need:`
  are refused ("comes with the process runner (step 4)"). Six draft entries (flint, hand axe, plant fibre, clay,
  copper, crucible).
- Three states (`journal::State`): unknown 0, noticed 1, known 2 (on wire and disk, pinned). A `noticed_by` trigger raises
  to noticed, `known_by` to known; nothing lowers one but the command. A known entry notices every unknown entry it
  points at. A trigger-to-rows table is built at load [0150].
- Per client a `Vec<State>` in the file's order; not run for a creative ape, a dead one, or one before its `Hello`. `have:`
  is judged at the end of the tick after the events when `inv_dirty` (the slots changed), every stack including the
  crafting grid's cells, and also on the tick the name arrives; items carried over from creative count in survival.
  `look:`: the server runs `edit::aim` from the eye (feet + up x 1.6) along the look it holds, 8 m, every 10 ticks;
  blocks only (shortcut: things and piles wait for `look:thing:`). Bots set a look with `look dx dy dz` [0150].
- `ToClient::Journal(Vec<(String, State)>)`, variant 23, pinned `[23, 1, 5, "flint", 2]`, by name not index; once the
  name is known every not-unknown entry, then each tick the changed ones (a diff against what it was told). Owner only
  [0150].
- Save version 19 added each player's `journal: Vec<(String, State)>` (not-unknown entries by name, in file order); 18 or
  older loads with all unknown. An entry the file does not have is kept aside (`Client::kept`), not sent, saved again
  untouched. A client's journal is hashed only when some entry is not unknown, so a recording without it keeps its hash
  [0150]. (Current save version is 22; see multiplayer-saves-and-data.md.)
- `/journal` lists entries, `/journal <entry> unknown|noticed|known` sets one; open to all until the operator role
  [0150]. `data/journal.toml` is not live: read once [0150]. Instincts are unbuilt [0150].
- The notices are chat lines "Journal: <question>" and "Journal: learned <title>" (none for the first message), no sound
  [0150, 0151].
- The book (step 3) [0151]: key J opens and closes (Keys row "Journal", rebindable); Esc closes; `Menu::Book`, pointer
  freed, nothing paused, not over the title or the death screen. Left the list (`book::Book::rows`): noticed entries by
  question first, then known entries under a heading per age by title, each group in file order; unknown entries are not
  listed and an empty group has no heading; long rows cut with "..". Right the page (`Book::page`): known entry title,
  text, and "LEADS TO" lines for noticed or known entries it points at (click to choose); noticed entry its question
  only; braces in text (`{temperature}`) not filled until step 4. A changed entry gets a dot until chosen (client-only,
  not saved). Text wraps by the book's `wrap`; list and page scroll by the wheel with a thin bar, scroll clamped in
  `Book::scrolls`. Text at the HUD's size (2 px a font px), page title 3 px; list row 28 px, page line 22 px, title room
  40 px. `book::layout`: a window at most 1100 x 680 UI pixels, 24 px from the edges, list 42 % of the inner width.
  Scripts: `journal`, `entry:<name>`. A menu opening lets go a held pick-up or drink (`play::let_go` via `set_menu`).
  No wire or save change; the visual pass comes later.
- Open: ordering open questions by the need they touch; a rest time for "the crosshair rested on it"; a soft sound and
  a line over the hotbar for a new question; saving the change marks [0150, 0151].

## Chat, commands and the debug UI

- Font: the game's own `data/font.txt`: squared capitals 5 x 7, round ones with the top-left corner cut; ADR 0030
  gives it real small letters (8 rows for a letter with a tail). `hud::font_cells` reads it into 8 x 8 cells,
  `hud::font_wgsl` puts them into `overlay.wgsl` as `FONT`; glyphs 32 to 126 of a `Rect` are ASCII; text two pixels a
  font pixel with a shadow (`Paint::text_flat` without). A test checks every printable character is drawn [0023, 0030].
  Open taste call: the squared N and M beside real small letters [0030].
- Menus' look: a shade down the left that fades, the page title large in capitals by a cyan bar with a readout under it,
  a numbered list ("01 Singleplayer"), the button under the pointer a solid cyan box in corner brackets with dark text;
  the bug form a dark panel with cyan corner brackets; the title page shows the logo (`hud::logo`: TECHNOAPE at 12 px a
  dot, white over cyan in the lower three rows with a navy shadow, the O a planet with a tilted orbit and a moon) [0023].
- Chat: `ToServer::Chat(String)` / `ToClient::Chat(String)`. The server passes a line as `<name> text`, says when a
  player joins or leaves, runs a line starting with `/` as a command answering its sender alone; lines cut to 256
  characters, control characters dropped. The client shows the last ten lines for ten seconds, more while typing. T
  opens the bar, `/` opens it with a `/`, Enter sends, Esc closes; keys and mouse go to it. Up and down bring back sent
  lines. Commands run on the server (the same for bots, in the recorder and replay) [0023].
- Commands: `/time [set <hh:mm|sunrise|day|noon|sunset|night|midnight|seconds> | add <seconds>]` (names go forward to
  the next such hour at the spawn, `sky::next_hour`; seconds set the world time), `/playmode [creative|survival]`,
  `/give <block> [count]` (64 by default), `/goto` (Terra and Luna by name, `home` or `spawn` for Terra's spawn; the sun
  in creative only, five sun radii from its centre; `planet` and `moon` are gone [0036]), `/help`, `/timewarp n` (1 to
  10000, up, down; `.` and `,` step it: the server's clock ticks n times faster, `You` carries the warp, the client
  eases its sky clock; only the clock speeds up, not the physics), `/place <block> [face i j layer]` (the block under the
  feet or the one F3's "Feet in" names; replaces what is there, never a cell a player stands in; negative face refused;
  free) [0036], `/heal`, `/feed`, `/hurt`, `/thirst`, `/journal`. shortcut: every player may run every command until
  an operator role exists [0023, 0036, 0145, 0150].
- Tab completes a command: `engine::net::complete` lists the words that can stand at the cursor (commands, options,
  block names); once the list shows, arrows pick from it. A server test runs every line the list can build [0023].
- The soak sends a chat line or a command now and then, so they run under the replay check [0023].
- Esc menu (play): the game darkens under "Paused": Back to game, Settings, Main menu, Quit game; the pointer is free
  and the world runs on. Left click goes to the next value, right click to the one before; each change applies at once
  and is written to `settings.txt` (`settings=` names another). Only play reads it. A new size remakes the surface, depth,
  capture buffer and bounce light's targets (`App::resize`) [0023, 0028]. Arrows change values on the settings pages
  only; elsewhere Enter presses [0023].
- Title menu: a start without arguments opens "TechnoApe": Singleplayer, Settings, Quit game, over a throwaway world
  (`title=1 save=0 record=0`) seen from orbit (`App::title_view`: 1.7 km up, the sun 100 degrees from the planet's
  centre, the view rolled, drifting around the line to the sun). The HUD and chat are hidden under it. Singleplayer
  lists the newest six saves under `saves/` by name, New world (the first free `saves/world-N.sav`, seed from the clock)
  and Back. Picking one closes this world and starts the game again as `run=play onscreen=1 save=<file>`; Main menu
  from the pause menu goes back the same way: a fresh process per world; the old window stays until the new one has
  drawn (ADR 0038 A) [0023]. Open: one process, one window, a loading screen (the developer's "separate task next")
  [0041].
- Removing a world: right click or Delete on a world of the Singleplayer list asks first; the row that removes is never
  the one under the pointer that asked. The save moves to `saves/removed/` under the first free name
  (`menu::remove_world`); nothing is deleted. Scripted: `rclick:k`, then `click:` the removing row; `remove` is Delete
  [0023].
- F3: eight titled panels (`hud::Section`, `hud::panels`, `App::debug_sections`): Performance, Simulation, Position,
  World, Player on the left; Drawn, Machine, Session on the right. Labels dim, values white, a value with a limit green,
  amber from three quarters of it, red over it (ADR 0004: the frame's worst 1 % against 8.3 ms, RAM and VRAM). Text
  shrinks (2, 1.5, 1 pixels a dot) before the columns would meet. The bug report's F3 lines are the same rows as `[Group]`
  and `label: value` [0023].
- Simulation panel: a local server's ticks timed in play (`Server::tick_ms` drained into `App::ticks`, ten seconds):
  typical and worst tick against the 50 ms a tick has, ticks a second of 20, a bar per tick with a line at half a tick
  (green under, amber over, red over a whole tick). A remote server's times stay there [0023].
- The looked-at label: what the crosshair is on named top middle with a swatch, "gives light" or "metal" under it
  (`hud::looking_at`); hidden under F3, menus and the title [0023].
- The aimed block's outline turns slowly through the colours (hues along its edges) and is three pixels wide (the ghost
  drawn nine times a pixel apart, `overlay.wgsl`); held dig stays red, held place green. Default now an RGB wave,
  saturated, a turn every 3 s (taste default) [0023, 0041].
- F9 bug report: the next frame is copied (without the form), then the form takes the text. Enter writes `bugs/<UTC
  time>/` on a thread: `report.md` (text, a `run=stand` line for the same view, F3 lines, inventory, the last 50 chat
  lines, the arguments), `screen.png` at full size, `world.sav` (a copy of the save when there is one, up to 30 s old).
  The commit comes from build.rs (`+changes` when tracked files differ). `bugs/` is not in git [0023].
- Scripted inputs (`acts=`): `t`, `/`, `type:text`, `back`, `enter`, `cancel`, `say:text`, `f3`, `f9`, `space`, `fly`,
  `up`, `down`, `earlier`, `later`, `tab`, `C`/`c`, `warp+`/`warp-`, `esc`, `click:k`, `rclick:k`, `point:x:y`, `remove`.
  Bots have `say <text>` and print what they receive as `{"bot":..,"chat":..}` [0023].
- shortcuts: no cursor movement inside a line, ASCII only; the title lists six saves, no scrolling or renaming; the
  menu's text keeps its pixel size at every resolution (the interface scale in ADR 0028 covers the HUD); no setting yet
  to hide the looked-at label or pick the outline's look (the Interface page of `docs/design/settings.md`) [0023].

## Settings pages and key bindings

- Every setting is a row of data (`settings::DEFS` in `crates/client/src/settings.rs`): key, label, kind (choice,
  slider, stepped number), default; values are `f64`, a choice its index. A new setting is one line in `settings!` and
  one entry in a page's list; a row can stand on two pages. A setting gets a row only when what it changes exists
  [0028].
- Built pages: Display (resolution, monitor, mode, frame cap, cap in the background, wait for the display, field of
  view, brightness), Graphics (preset, detail, sun shadows, bounce light, reflections, lamp light, block textures,
  stars), Controls (sensitivity, zoomed sensitivity, invert look, zoom held or pressed once, the double tap's time), Keys,
  Accessibility (interface scale, high contrast, `waypoints_shown`, and rows shared with other pages). Window resolution
  choices are those that fit the screen; display modes: windowed, windowed fullscreen (borderless at the screen's size),
  fullscreen (exclusive at the chosen size). Field of view 60-110, sensitivity 0.25-3x, detail distance (`Opts::k`
  3-8) [0023, 0028].
- Key bindings: `settings::ACTS` lists every action with two bindings (a key by its place on the board or a mouse
  button) and the input (`App::act`) its press and release make. `Settings::held` asks if an action's binding is down.
  Esc, the wheel and keys inside a text field are fixed. A clash is allowed and named on both rows [0028].
- The file stays `key=value` with a `version=` line; a bad value keeps the row's default; an unknown line is written back
  as it was. `dump=1` prints what a file means as one JSON line [0028].
- Defaults draw today's picture; only play reads the file; every graphics switch is off at its default (`Globals.opt`
  0, brightness 1, bounce on). Default field of view 70 (the gate's views moved by half their pixels at 80). High is the
  default preset [0028].
- The frame cap sleeps until the next frame is due (`App::cap_hz`, in `about_to_wait`), on screen in play only: the
  refresh rate, half of it, a number, or none; 30 when the window is not in front. Waiting for the display is its own
  row, off by default. Off-screen play has no display and no cap [0028].
- A display change that can lose the picture (resolution, monitor, mode) opens "Keep these display settings?" and goes
  back after 10 s; off screen there is no question; `confirm=1` asks anyway [0028].
- The menu shows as many rows as fit and scrolls (wheel or arrows); arrows and Enter do what the pointer does. The
  interface scale draws the whole HUD in its own pixels [0028].
- Shader switches: bits in `Globals.opt.x` (sharp sun shadows, no reflections, flat block colours, no stars);
  `Post.cfg.w` and `Post.tune` (bounce off with a flat fill from the sky; brightness as a curve over the finished
  picture) [0028].
- Scripted (`acts=`): `next`, `prev`, `go`, `nudge+`, `nudge-`, `scroll:n`, `bind:name`, `key+:name`, `key-:name`,
  `click:k` (a row of the whole page) [0028].
- The Keys page: `Settings::key_rows(query)`: a search row, then each group's heading and actions, then the defaults; a
  query keeps an action with a word that starts with it or a key named by it. A fixed-width table; `hud::menu` takes a
  tone per row: red for a shared key, plain text for a heading [0036].
- Waypoint marks hidden by the action `waypoint_overlay`, default K (setting `waypoints_shown`, Accessibility) [0036].
- Wheel: a cylinder's or cone's height while the hammer holds one, else the flight speed while flying, else the hotbar,
  round its ends [0036].
- shortcuts: no render scale (the composite reads the G-buffer pixel for pixel); the frame cap, wait-for-display,
  monitor row and exclusive fullscreen are on-screen only and not scripted; the bounce light's "off" fill is one flat
  colour from the sky; the presets hold five rows. Not built with their systems: crouch, auto-step, the space mouse's
  other mode, sound rows, interface rows beyond the scale, the world's and server's pages (`docs/design/settings.md`)
  [0028]. (Sprint was in the "not built" list and was built by ADR 0118.)

## Navigation and the HUD's look

- A place is a body, a latitude, a longitude and a height. North is the body's spin axis; Terra's longitude 0 runs
  through the spawn, a moon's through the middle of the face turned to its planet (`crates/client/src/nav.rs`) [0030].
- A compass strip and waypoints: earned in survival, always there in creative (the tool that earns them does not exist
  yet). N drops a waypoint; the pause menu lists and removes them; kept beside the save as `<save>.waypoints` and go with
  a removed world [0030].
- A waypoint has a colour, an icon (diamond, square, plus, ring) and a view: on its own body (default), always, or within
  a radius (0.5 to 100 km). File line `body|x|y|z|colour|icon|view|name`; old `body|x|y|z|name` lines still read.
  Waypoints on another body are drawn where that body is now (`World::place`). A waypoint set to "always" is drawn
  through the bodies in front of it [0036]. shortcuts: no rename or recolour from other pages, one on another body is
  not shown in the old form, a remote server's waypoints are not kept (need a file per server) [0030].
- Every panel is one dark glass with the accent down its side (`hud::PANEL`): F3, chat, hotbar, labels, menu rows. The
  hotbar shows each block as a little cube, top and sides in the block's colours (flat colours, not textures). The world
  is dimmed under a menu (a blur needs a pass, not done). A menu page's rows come in from the left over 0.14 s and the
  lit row moves out 8 pixels in 0.1 s; click targets do not move; a scripted off-screen run shows no motion (`motion=1`
  does) [0030].
- Open: the globe map waits for the bake (`worldgen-plan.md`) and the attitude ball for the jetpack [0030]; the look has
  no settings yet (the Interface page) [0030].

## The playtest menu (F10)

- Action `playtest`: the clock goes to the row's moment with `/time set`, the player to where it is seen with `Move`,
  with a line saying what, when and which way to look [0036]. shortcut: a row reads the client's clock, a little off
  under a time warp [0036].

### Spectator mode, and the playtest menu rebuilt on the orbits' own times [0202, 2026-11-24]
The developer's calls (2026-11-24): F10's rows are the sky's sights only, the old set-ups removed: the eclipses and
Luna's eight phases, each found in the world's own orbits forward from now (`sights::find`, `World::pose`: nothing is
posed by hand, ADR 0177, 0181), within 3 home years. Rows: Sol totally and partly hidden by Luna from Terra (total: where
the line from Sol through Luna meets the ground, 20 s early; partial: a line-up whose axis misses Terra, at the place
with most of Sol hidden where Sol is 5 degrees up); Luna totally or partly in Terra's umbra (her centre and four limb
points; seen from under her); Terra over Sol from Luna's near side; the gas giant's moon over Sol from the giant's
cloud tops (a ring if its disc is the smaller); each phase at its elongation (0, 45 ... 315 degrees), from the darkest
sky with Luna 20 degrees up, a month whose phase falls in an eclipse passed over. A row puts the player 40 m over the
place (over the trees; an eclipse of the sun's eye on the shadow's axis at that height, `sights::stand`, and its line
gives the share hidden from the eye), the head tilted up to the sight, and a survival player into spectator mode, so the sea does
not drown and the ground does not hide it (Claude's call, from the shots: a survival player sank under the sea and a
lunar eclipse was watched from inside a tree). No annular row for Luna: from no ground on Terra does she look smaller than Sol (the
sun is sized for a perfect total, ADR 0041). And spectator mode, as Minecraft's, a mode of `/playmode` (the developer:
`/playmode spectator`, back with `/playmode survival` or `creative`; anyone may, as every command): the server's state per
player, not saved (a save keeps where leaving would put the player, so no format, save or hash changes); its steps are
`phys::ghost_at` (along the look at the flight speed, the wheel's, through blocks and fluids, no gravity, no box, at
the input's own time on both sides; held `GHOST_FLOOR` over the body's bottom layer); its edits, digs, bucket, takes,
item moves, strikes, crafts, uses, choices, drinks, creative toggle, `/give`, `/place` and `/structure` are refused
(kept: `Move`, which F10 and teleports use, and `Hand`, the hotbar slot only); going into it ends a held dig and a
drink. It is in no cell (an edit, `/place`, `/structure` or a fall may fill where it hangs), picks up nothing, takes
no harm, hunger, thirst or breath, learns nothing, and no other client is sent it in `Players`. The world round it is
kept loaded and simulated as round a player, and the herds near it are kept awake, but they do not react to it (no
fleeing, no targeting: it is not one of their players; the overseer's call, 2026-11-24). It is told with
`ToClient::Spectator(bool)` (the last variant). Leaving it (by command, and what a save or a takeover by its own name
keeps) puts the player standing on the open ground of its column (`phys::open_ground`: the first solid from the sky
down, so not in a cave nor in the air, swimming on water, unhurt); over lava, a gas body or in space (over two body
radii out), the open ground where it went in, else that place. The HUD drops the bars, the held block, the hand, the
aimed block's outline and look-at panel, and footsteps; the crosshair and hotbar stay.
Alternatives rejected: `/spectator` as its own command (the developer chose a mode of `/playmode`).
Replaces: 0036's eclipse rows on `nav::next_eclipse` and the playtest set-ups (light, bars, death, flight, Luna).

## Play-test fixes of 2026-11-11 evening (mixed batch)

- (also rendering-and-lighting.md) Drawn quads grow by 1 mm (`DRAW_GROW`, shader.wgsl `vs`) to close pixel holes where a
  merged quad's edge meets neighbours; `o.local` keeps the exact corner so textures do not slide [0041]. `texel()` hashes
  the grass fringe by the column along a side (sides facing +-j had the u axis as height) [0041]. The selection outline's
  corners are placed as `place` does (`hud::ghost`, test `a_ghost_sits_where_the_mesh_draws_its_block`), the real frame
  size rides in `opt.zw`, and the overlay's uniform structs match shader.wgsl (the missing `ctr`) [0041]. Lamp light:
  `3/(1+d^2/4)` to 12 m (`LAMP_R` 12 in shader.wgsl and gpu.rs, grid list 7^3 cells a lamp) [0041, later changed: checked in
  code, `LAMP_R` is 24.0 in `crates/client/src/gpu.rs`]. A run other than play reads `settings=` when given [0041].
- (also sky-weather-and-water.md) `NIGHT_GLOW` in `sky_colour`: in air, at night, a faint blue brighter towards the
  horizon; the fog's horizon colour (frame.rs `haze`) adds the same glow. Luna orbits at 2.83 Terra radii in the old data
  (see Luna below); stars stay fixed in space (flying 328 m/s round a 4 km planet turns the sky 4.5 degrees a second)
  [0041].
- (also bodies-and-orbits.md, sky-weather-and-water.md) Luna: `data/bodies.toml` gives Luna `size = 3.7` and `orbit =
  60.0` (instead of `n` and `orbit_min`), Terra `sun = 23500.0` (the sun's distance in its radii). `engine::bodies::columns
  (i, n)` gives Luna's columns: n / 3.7 to the next 32 (1664 under 6144; 416 / 832 / 3328 / 6656 under 1536 / 3072 /
  12288 / 24576); `moon=` overrides it and a save keeps the `moon_n` it was made with. `Sky::new` derives the sun's
  radius so that from the ground under Luna at a new moon the sun is `SUN_UNDER_MOON` (0.95) of Luna's size; `Sky::new`
  asserts the moon clears the planet's ground. On the 4 km Terra: Luna about 1150 m radius 255 km out, the sun about 435
  km radius 99.7 Mm out. Test `luna_is_a_fixed_part_of_terra_and_just_covers_the_sun`. `MOON_AT` 10,000 km and the
  inertial origin -10^10 m so no body's frame (1.5 x its sphere of influence) reaches another's at any size. Axial tilt
  of Luna's orbit 2 degrees. Terra stays G on every size, Luna's `surface_g` is G times the radius over Terra's (one
  density), so a jump on Luna is about 4.4 to 4.66 m [0041, amended 0132]. Eclipses over 10 years: 19 solar of 34 new
  moons, 11 total somewhere; lunar blood moons about two minutes [0041].
- `sky::DAY` is 1800 s (a 30 minute day); month (16 days), year (54 days) and the sky's period (432 days) are counted in
  days; clock hours, `/time set` and F10 follow `DAY`; fixed views stand at time 0, 9 am (also sky-weather-and-water.md)
  [0041].
- Saves: version 6 added with the moved frames (a player on Luna in an older save is moved with its grid; one in space goes
  to the spawn keeping what it held) [0041]. (Superseded by later versions; see multiplayer-saves-and-data.md.)
- Luna's chunk tree: roots come from the power of two in its columns (1664 is 13 x 128): kept, no column count near 3.7
  has a small odd part; a root grid that can coarsen past the power of two is a follow-up [0041].

## No longer holds

- 0040's "hunger alone never kills while there is no food" (health stopped at 1 when starving): replaced by 0210,
  starving kills.

- 0036's playtest rows: the two eclipses from `nav::next_eclipse` (one sky cycle, Terra and Luna only) and the set-ups
  for light, the bars, death, flight and Luna: replaced by the sky's rows of ADR 0202, "Spectator mode, and the playtest
  menu rebuilt on the orbits' own times" (2026-11-24).

- 0007's radial gravity toward the nearest body's centre and a 9.81 gravity: replaced by 0024 (every body pulls, own
  orientation) and 0132 (G 19.62).
- 0007's collision by 45 sampled points and its "1.2 x 1.2 x 3.6 cells" box in 0.5 m blocks: replaced by grid-space
  collision (its own revision) and the 1 m blocks of ADR 0033 (the box is 0.6 x 0.6 x 1.8 m).
- 0007's snap over 1 m: replaced by numbered steps and eased corrections (ADR 0019).
- 0007's jump of 4.8 m/s: replaced by `phys::JUMP` 4.87 to 6.84 (0132).
- 0017's bare-hand 1 m cube dig (revised the same day to single blocks); its Q key for the shape (now B, ADR 0024); its
  H key for the hammer "until items exist" (items exist).
- 0023's `phys::FLY_MAX` 250 m/s and 1 km/s: replaced by 10,000 m/s (0073).
- 0023's font as "lowercase as small capitals": replaced by real small letters (0030).
- 0023's `/goto planet|moon`: replaced by `/goto` Terra, Luna, `home`, `spawn` (0036).
- 0023's outline turning once in 12 s: replaced by a turn every 3 s (0041).
- 0023's day of 1200 s in hunger/clock arithmetic: replaced by `sky::DAY` 1800 s (0041).
- 0040's "the inventory is kept" on respawn: replaced by piles at death (0047, 0148); its fuel-cell hunger icon and ten
  half-cell bars: replaced by the smooth bars and stomach (0040, same ADR, later section); its 3 m free landing at 7.7 m/s
  and 21 m/s kill: now 10.8 and 30 (0132); its save version 5 (now far older than the current version).
- 0040's hunger 1200 s per 20 points: now 1800 s (0041's day).
- 0041's Luna gravity "by the square-root rule" (0037) and the lamp radius 12 and 3/(1+d^2/4): the first replaced by
  radius-proportional gravity, the second changed since (code: `LAMP_R` 24.0).
- 0041's totality at 0.995 of Luna's size: replaced by 0.95.
- 0146's instant breath reset (0079): replaced by 2 s recovery (in 0146 itself).
