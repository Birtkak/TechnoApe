# The settings menu

- **Status:** design proposal, 2026-11-11. Every default and range is a first proposal for the developer to
  change. ADR 0023 holds what exists today: resolution, display mode, field of view, mouse sensitivity and
  detail distance, in `settings.txt`.
- For: the coding session that builds it, one page at a time (order at the end).
- **Built 2026-11-11 (ADR 0028):** the page frame and Display, Graphics, Controls, Keys and Accessibility,
  the rows whose systems exist. Not yet: render scale, HDR, the first-start measurement, each preset as a
  benchmark row, and every row marked with a system to come.

## Rules
1. **A setting exists only when the thing it changes exists.** No greyed-out rows for systems to come. The
   "when" column says which system brings each row; rows marked *now* can be built today.
2. **Every change shows at once.** No Apply button. The one exception is a display change that can leave
   the player without a picture (resolution, display mode, monitor): it reverts after 10 s unless confirmed.
3. **Three owners, three places.** A setting belongs to the player on this machine (`settings.txt`), to the
   world (in the save, set when the world is made, changed by its owner), or to the server (its own file,
   changed by an operator). The menu shows all three but says whose each is, and a player on someone
   else's server sees the world's and the server's rows read-only.
4. **Defaults hold the budget.** On the reference machine the default of every row together holds ADR
   0004. A preset names a point on the cost curve; a single row never needs explaining with a number of
   milliseconds, but each graphics row shows its cost as one to three bars.
5. **Benchmarks and views never read it** (ADR 0023). `settings=` names another file for a scripted run.
6. **Nothing is lost.** An unknown key in the file is kept and written back; a bad value falls back to the
   default for that row only. The file stays `key=value`, readable and editable by hand.
7. **Reachable without a mouse and without eyes.** Arrow keys and Enter work everywhere a click does;
   every row is scriptable (`acts=`) and listed by `settings=... dump=1` as JSON, so a test can set and read
   any of them (principle 10).

## The pages
The pause menu's Settings and the title's Settings open the same pages. Order: Display, Graphics, Controls,
Keys, Sound, Interface, Game, Accessibility, World, Server, then Debug when it is switched on.

### Display (the screen; all *now* unless said)
| Setting | Values | Default | Notes |
|---|---|---|---|
| Monitor | the connected screens | the one the window is on | reverts unless confirmed |
| Display mode | windowed / windowed fullscreen / fullscreen | windowed fullscreen | exists |
| Resolution | those the mode allows | the screen's own | exists |
| Frame cap | the screen's refresh rate / half of it / 30 / 60 / 120 / 144 / 240 / none | refresh rate | the developer's call, 2026-11-11: an uncapped loop holds a core the simulation needs |
| Cap in the background | 30 / same as above | 30 | when the window has no focus; a server on the same PC keeps its cores |
| Wait for the display (vsync) | off / on | by the cap package's measurement | whichever caps with less input delay becomes the default; the other stays a choice |
| Render scale | 50 to 100 % in steps | 100 % | the 3D picture drawn smaller and scaled up; the interface stays sharp. The first lever for a weaker GPU |
| Field of view | 60 to 110 | 70 | exists; the zoom key divides it. 70 stays the default: benchmarks and the reference views are drawn at the defaults |
| Brightness | a gamma curve, with a test picture: a cave mouth at dusk | middle | the ray-traced night is dark on purpose; this is the screen's correction, not a cheat |
| HDR output | off / on | off | later, with its own spike |

### Graphics (what is drawn and how well)
A **preset** row first: Low, Medium, High, Ultra, Custom. Choosing one sets every row below; changing a row
shows Custom. High is the default and is what ADR 0004 is measured at. Low must hold 60 fps on a GPU half
the reference; a GPU without ray queries gets the page's fallback (see "Without ray tracing").

| Setting | Values | Default | When |
|---|---|---|---|
| Detail distance | `Opts::k` 3 to 8 | 4 | exists; the biggest single cost |
| Sun shadows | sharp / soft | soft | *now*; soft is the penumbra from the sun's real size |
| Bounce light | off / half resolution / full | half | *now* (ADR 0025, C); off leaves a flat sky fill |
| Reflections | off / metals and water / everything shiny | metals and water | *now* |
| Lamp light | nearest 16 / 64 / all in range | all in range | *now* (ADR 0025, H) |
| Smoothing | fewer passes / standard | standard | *now*; fewer is cheaper and noisier in motion |
| Block textures | 16 px / flat colours | 16 px | *now*; flat is also an accessibility choice |
| Water | simple / reflecting / with splashes | with splashes | *now* (ADR 0020) |
| Particles | few / normal / many | normal | *now* for splashes; dust and sparks with fine cells |
| Stars and galaxy | off / on | on | *now* |
| Clouds and weather | flat / lit volumes | lit volumes | with the air (W3) |
| Plants | near only / to the horizon | to the horizon | with the biomes (W3), then living plants (W7) |
| Animals and creatures drawn | distance | far | with creatures |
| Other players' detail | name tags, held tools, full animation | all | with player models |
| Machines' moving parts | still when far / always | still when far | with networks: a thousand turning shafts is a cost |
| Screen shake | 0 to 100 % | 100 % | with the first thing that shakes; also on Accessibility |

**Without ray tracing.** The engine needs hardware ray queries today. If the game is ever to start on a
GPU without them, that is a second lighting path and its own ADR; until then the title screen says plainly
why it cannot start, and names the GPU it found. Not a settings row.

### Controls (how the player's hands map to the ape; *now* unless said)
| Setting | Values | Default |
|---|---|---|
| Mouse sensitivity | 0.25 to 3x | 1x (exists) |
| Sensitivity while zoomed | follows the zoom / own value | follows |
| Invert look up and down | off / on | off |
| Raw mouse input | on / off | on |
| Sprint | hold / press once | hold |
| Crouch | hold / press once | hold |
| Zoom | hold / press once | hold |
| Auto-step up half blocks | on / off | on |
| In space: the mouse | pitches and yaws freely (ADR 0024) / keeps the horizon | freely |
| In space: roll keys | Q and E / swapped | Q and E |
| Build hammer: the wheel | height / size, with a held key for the other | height (ADR 0017) |
| Place where I look / where I stand | the aimed face / the block ahead of the feet | aimed face |
| Controller | off / on, with stick dead zones, curves and trigger thresholds | off; later, own spike |

### Keys
One row per action, grouped as the play line in CLAUDE.md groups them: moving, digging and placing, tools
(hammer, bucket), the hotbar, flight and space, chat and commands, the interface (F3, F9, the menu).
- Click a row, press a key or a mouse button; Esc cancels, Backspace clears. Two keys per action.
- A clash is shown on both rows and allowed only between groups that are never live together (a menu key
  and a movement key).
- "Back to defaults" per group and for all. The bindings are in `settings.txt` as `key.<action>=<name>`,
  by the key's position on the board, so WASD stays under the same fingers on an AZERTY board.
- Every action the player can do has a row. A test fails when an input in the code has none.

Auto jump (Controls and Accessibility, off): a ledge one block high in the way of a walk is jumped
(ADR 0033).

### Sound (with the audio system, the roadmap's last step of the first playable)
Built with the first cut of sound (ADR 0032): master, world, footsteps, interface, mute in the background,
sound in space. The other rows arrive with their sounds.
| Setting | Values | Default |
|---|---|---|
| Master, world, machines, creatures, weather, footsteps, interface, music | 0 to 100 % each | 100, music 60 |
| Output device | the system's, or one by name | the system's |
| Speakers | headphones (3D for two ears) / stereo / surround | headphones |
| Reverb in caves and rooms | off / on | on |
| Sound in space | realistic (only through what you touch and your suit) / softened / full | realistic |
| Mute in the background | off / on | on |
| Voice chat | off / push to talk / open, input device, others' volume by distance | off; with the dedicated server |
| Subtitles for sounds | off / on, with the direction | off; also on Accessibility |

### Interface
| Setting | Values | Default | When |
|---|---|---|---|
| Interface scale | 75 to 200 % | by the screen's height | *now*; ADR 0023 lists it as a shortcut to pay |
| Hide the interface | a key | – | *now*, for screenshots |
| Crosshair | shape, size, colour | small cyan cross | *now* |
| Hotbar | always / fades when idle | always | *now* |
| Chat | lines shown, seconds they stay, width, opacity | 10, 10 s | *now* (ADR 0023) |
| Block outline and the hammer's ghost | colour, thickness | cyan | *now* |
| Names over players | always / when aimed at / never | always | with player models |
| Units | metric; a 24 hour or 12 hour clock | 24 hour | *now* for the clock |
| Tips while learning | on / off | on | with the journal (crafting) |
| Compass and position | off / compass / with coordinates | compass | in survival with the first tool that gives one (knowing where you are is earned); always there in creative |
| Language | the game's languages | the system's | when text leaves ASCII (ADR 0023's shortcut) |

### Game (this player's own comfort; nothing here changes the world's rules)
| Setting | Values | Default | When |
|---|---|---|---|
| Player name | text | the system's user name | *now* |
| Auto-pick-up | on / off | on | *now* |
| Confirm before dropping or destroying a stack | on / off | on | *now* |
| Break a block: hold or click each | hold / click | hold | with mining tools |
| Blueprint paste asks first | on / off | on | with blueprints |
| Pause when the menu opens | singleplayer only: on / off | off (the world runs on, ADR 0023) | *now* |
| Autosave notice | off / on | off | *now* |
| F9 bug reports include the save | on / off | on | *now* |

### Accessibility (a page of its own, so nobody has to hunt)
| Setting | Values | Default |
|---|---|---|
| Text size | follows the interface scale / larger | follows |
| A smoother font for long text | off / on | off; the blocky font stays for titles |
| High contrast interface | off / on | off |
| Colour-blind palettes | none / three kinds; changes ore signs, wire and pipe colours, status lights | none |
| Never by colour alone | – | always on: a state shown by colour also has a shape or a word. A rule for the content, checked in review |
| Reduce flashing and lightning | off / on | off |
| Screen shake, view bob, the roll in space | 0 to 100 % each | 100 % |
| Field of view changes with speed | on / off | on |
| Hold or toggle for every held input | per action | hold |
| One-handed layouts | off / left / right | off |
| Subtitles for sounds, with direction | off / on | off |
| Time to answer timed prompts | normal / longer / none | normal |
| Arachnophobia-style swaps for creatures | off / on | off; with creatures, if any need it |

### World (the world's rules; set when it is made, shown read-only to guests)
The New world screen is this page, plus what can never change afterwards.
| Setting | Values | Default | Can change later |
|---|---|---|---|
| Name | text | "World N" | yes |
| Seed | number or text, or the curated default (ADR 0026) | the curated world | no |
| Planet size | standard 4 km / huge 8 km / small | standard | no |
| Play mode | survival / creative | survival | yes (`/playmode`) |
| Hostile creatures | off / day and night / night only, and how hard they hit | on (ADR 0027) | yes |
| Needs (thirst, food, air, heat) | off / gentle / normal / harsh | normal | yes |
| Death | drop and wake at Home / keep everything | drop (open question) | yes |
| Day length | 10 / 20 / 40 minutes | 20 | no: the tide and the seasons hang on it |
| Seasons | off / on, and the year's length | on (ADR 0026) | no |
| Ore richness | the budget's slack, 0.5 to 2x | 1x | no: it is in the bake |
| World physics (collapse, loose blocks) | off / on | on | yes |
| Weather damage, pollution, radiation | off / on each | on | yes |
| Commands | everyone / the owner / off | owner | yes |
| Friends may join | off / by invitation / anyone on the network | by invitation | yes |

### Server (the dedicated server's own file; an operator's page in the menu)
Name and message of the day, port, most players, a password or an allow-list, who is an operator, save
every N minutes and how many backups to keep, the view distance it will serve, a tick-time warning line,
whether flight recordings are kept (ADR 0011). With the dedicated server (ADR 0005); none of it today.

### Debug (hidden until `debug=1` in the file or a command switches it on)
The F3 overlay's pages, the frame-time graph, wireframe, chunk and level borders, the lattice (ADR 0025),
show the server's position beside the predicted one, show corrections as they happen, a fake network
(delay, jitter, loss) for feel testing, the meshing workers' count, `greedy=0`, and the time-warp step.
Each is an existing argument or overlay given a row; a tester should not need the command line.

## The file
- `settings.txt`, `key=value`, grouped by page with a comment line per group; `key.*` for bindings.
- A `version=` line. A newer game reads an older file and fills what is missing; it never rewrites a key
  it does not know.
- World settings are in the save (ADR 0016) and travel with it. Server settings are in `server.txt`
  beside `saves/server.sav`.
- First start: resolution and display from the screen, the preset from a 5 s measurement of the title
  scene (frame time at High; step down until the budget holds), and that is said in one line on the title
  page with the preset it chose.

## Checks
- A test loads a file with every key at its lowest, at its highest and at a nonsense value: no crash, the
  nonsense rows at their defaults, unknown keys still in the file after a save.
- A test walks every page by keys alone and reaches every row.
- Per graphics row, a view at each value in `tests/views/` so a row that stops doing anything fails.
- Each preset as a benchmark row: Low, Medium, High, Ultra on the reference machine.
- The played run of the gate uses `tests/gate-settings.txt`; it gains a key per page as pages land.

## Order to build
1. **The page frame:** pages, keyboard movement, the confirm-or-revert for display changes, the file's
   version and unknown-key rule, `dump=1`. Display gains the frame cap, render scale and monitor.
2. **Keys.** It unblocks every later control and is the most asked-for page in any game.
3. **Graphics with presets**, each row measured and viewed; the first-start measurement.
4. **Controls, Interface, Game**, the rows marked *now*.
5. **Accessibility**, the rows whose systems exist; the "never by colour alone" rule goes into CLAUDE.md's
   conventions the day the first coloured state appears.
6. **World** as the New world screen once the world generation gives it something to choose (W2 on).
7. Sound, Server and the rest arrive with their systems; each system's design names its rows here.

## Decided by the developer, 2026-11-11
- **High is the default preset and what ADR 0004 is measured at.**
- **A compass and coordinates are earned in survival and always shown in creative.**
- **Sound in space is realistic by default:** only what you touch and your suit carry it.
- **A world's owner may change most world rows at any time, for now.** It is for playtesting and is looked
  at again later; the rows marked "no" stay fixed because the bake or the sky hangs on them.
- **Built first, in one piece:** Display, Graphics, Controls, Keys and Accessibility, the rows whose
  systems exist.
