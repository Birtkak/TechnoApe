# 0023 – Chat, commands, the F3 overlay and bug reports

- **Status:** accepted
- **Date:** 2026-11-10

## Context
The developer asked for a chat like Minecraft's (lines in the bottom-left that fade, T to type, `/` for a
command; first commands /time, /playmode, /give), an F3 overlay of numbers, and an F9 bug report: a form to
describe the bug that saves a screenshot and the engine's state, so Claude can look at reports later without
having been there. The HUD so far drew rectangles and a 3 x 5 digit font only.

## Options considered
1. Text: an 8 x 8 bitmap font as a WGSL constant, a rectangle per character (font8x8, MIT, the public-domain
   basic set) – no texture, no atlas, fits the existing rectangle pipeline; blocky, which suits the look.
   A TTF rasteriser (fontdue, ab_glyph) – smooth text, but an atlas, a texture and a bigger dependency.
2. Commands on the client or on the server. On the server they work the same for every client, bots
   included, go through the flight recorder and replay, and change authoritative state only where it lives.
3. Bug reports as a file per report, or a folder per report with a Markdown file, the screenshot and a copy
   of the save.

## Decision
- **Font:** the game's own, `data/font.txt` (2026-11-10; it was font8x8's basic set before): squared
  capitals 5 x 7, the round ones with their top-left corner cut, lowercase as small capitals (the capital
  without rows 1 and 4, on the baseline) unless the file draws one. `hud::font_cells` reads it into 8 x 8
  cells and `hud::font_wgsl` puts them into `overlay.wgsl` as `FONT` at start; a test checks every
  printable character is drawn. Glyphs 32 to 126 of a `Rect` are ASCII characters; text is drawn two pixels
  a font pixel with a shadow (`Paint::text_flat` without, for dark text on a light box).
- **The menus' look (HUD brackets, the developer's pick of three):** a shade down the left that fades out,
  the page's title large in capitals by a cyan bar with a readout under it, and a numbered list ("01
  Singleplayer"); the button under the pointer is a solid cyan box in corner brackets with dark text. The
  bug form is a dark panel with cyan corner brackets. The title page shows the logo instead of a title:
  TECHNOAPE from the font at 12 px a dot, white over cyan in the lower three rows with a navy shadow, the O
  a planet lit from the upper right with a tilted orbit and a moon (`hud::logo`).
- **Chat:** `ToServer::Chat(String)` and `ToClient::Chat(String)`. The server passes a line to everyone as
  `<name> text`, says when a player joins or leaves, and runs a line that starts with `/` as a command,
  answering its sender alone. Lines are cut to 256 characters, control characters dropped.
  The client shows the last ten lines for ten seconds, more while typing. T opens the bar, `/` opens it with
  a `/`, Enter sends, Esc closes; while it is open keys and the mouse go to it. The client's own notes (the
  welcome line, bug report results) go into the chat too.
- **Commands:** `/time [set <hh:mm|sunrise|day|noon|sunset|night|midnight|seconds> | add <seconds>]` (names
  and hours go forward to the next such hour at the spawn, `sky::next_hour`; seconds set the world time
  itself, and the tick counter wraps under it when that is before the session's start), `/playmode
  [creative|survival]`, `/give <block> [count]` (by name, 64 by default), `/goto planet|moon|sun` (planet,
  home or spawn: the planet's spawn; the moon: a standing point on its day side; the sun, creative only: a
  spot five sun radii from its centre, looking at it), `/help` (a table, one command a line). Anyone may run
  them. Up and down in the bar bring back the lines sent before.
- **Creative flight:** space twice within 0.3 s flies or lands; the wheel sets the speed (x1.25 a notch, up to
  `phys::FLY_MAX` 250 m/s), space rises, shift sinks. It is `Input.fly` and `Input.down` in the shared
  physics: no gravity, the box still collides, no step-up. The server zeroes `fly` for survival players.
- **Teleports** move the player at rest (`Server::put`); the client takes a correction of 2 m or more as one
  and looks the server's way, level.
- **Esc menu (play):** the game darkens under "Paused": Back to game, Settings, Main menu, Quit game; the pointer is
  free while it is open and the world runs on. Settings: resolution (those that fit the screen; for
  exclusive fullscreen those it can switch to), display (windowed with a title bar, windowed fullscreen:
  borderless at the screen's size, fullscreen: exclusive at the chosen size, borderless when the screen has
  no such mode), field of view 60-110, mouse sensitivity 0.25-3x, detail distance (`Opts::k` 3-8). Left click
  goes to the next value, right click to the one before; each change applies at once and is written to
  `settings.txt` (key=value lines; `settings=` names another). Only play reads it, so benchmarks and views
  keep the defaults. A new size remakes the surface, depth, capture buffer and the bounce light's targets
  (`App::resize`). Scripted: `esc`, `click:k`, `rclick:k`.
- **Title menu:** a start without arguments (a double click) opens "TechnoApe": Singleplayer, Settings,
  Quit game, over a throwaway world (`title=1 save=0 record=0`) seen from orbit (`App::title_view`): 1.7 km up, just
  out of the air, the sun 100 degrees from the planet's centre so most of the disc in view is lit, the view
  rolled so the planet's limb runs low on the left and the sun stands high in the middle, drifting around
  the line to the sun. The HUD and chat are hidden under it. Singleplayer lists the
  newest six saves under saves/ by name, New world (the first free `saves/world-N.sav`, a seed from the
  clock) and Back. Picking one closes this world (the local server saves as it stops) and starts the game
  again as `run=play onscreen=1 save=<file>`; the pause menu's Main menu goes back to the title the same
  way. A fresh process per world, so no state of the old world (chunks, traced builds, the prediction)
  has to be torn down by hand; the cost is the window closing and opening again.
- **F3:** two columns of the client's state: build, frame and GPU times, position on the body, the block at
  the feet and the aimed one, the clock, the sun and moon, mode and tool, chunks, quads, lamps, players;
  the GPU, backend, RAM and VRAM, workers, seed and sizes, server, save and recording.
- **F9:** the next frame is copied (without the form), then the form takes the text. Enter writes
  `bugs/<UTC time>/` on a thread: `report.md` (the text, a run=stand line that shows the same view, the F3
  lines, the inventory, the last 50 chat lines, the arguments), `screen.png` at full size, and `world.sav`, a
  copy of the save when there is one. The commit comes from build.rs (`+changes` when tracked files differ).
- Scripted inputs (`acts=`) reach all of it: `t`, `/`, `type:text`, `back`, `enter`, `cancel`, `say:text`,
  `f3`, `f9`, `space`, `fly`, `up`, `down`, `earlier`, `later`, `tab`, `C`/`c`, `warp+`/`warp-`, `esc`, `click:k`, `point:x:y` (the pointer, to see a button lit). Bots have `say <text>` and print what they receive as `{"bot":..,"chat":..}`.

## Consequences
- Reports are where to look first for a bug the developer saw; `bugs/` is not in git.
- shortcut: every player may run every command; an operator role when servers go public.
- shortcut: no cursor movement inside a line, ASCII only.
- shortcut: the title lists six saves, no scrolling, renaming or deleting; those when there are many worlds.
- shortcut: the menu's text and buttons keep their pixel size at every resolution; a UI scale setting when
  4K screens look small.
- The streamer counts a child chunk that itself splits as resolved when it decides whether a coarse chunk
  may stop drawing; before, coming back from far away (/goto) left coarse chunks around the camera for good.
- The world save in a report can be up to 30 s old; the session's recording, when there is one, is exact.
- The soak sends a chat line or a command now and then, so they run under the replay check too.

## Addendum 2026-11-10: completion, zoom, time warp (from the developer's bug reports)
- Tab completes a command: `engine::net::complete` lists the words that can stand where the cursor is (the
  commands, their options, block names for /give), drawn over the chat bar. Once typing shows the list, the
  arrows pick from it; otherwise they recall sent lines. A server test runs every line the list can build
  and checks the commands know it.
- C held zooms 3x (eased over about 0.1 s, the mouse 3x slower). Creative is /playmode only: C no longer
  switches it.
- /timewarp n (1 to 10000, up, down; . and , step it) makes the server's clock tick n times faster. `You`
  carries the warp, and the client runs its sky clock at it between messages, easing onto the server's.
  shortcut: only the clock speeds up, not the physics; a player leaving a body under warp keeps a real-time
  velocity.
- Flight goes up to 1 km/s (`phys::FLY_MAX`).
- The sky (ADR 0022) gains stars and a faint galaxy band, fixed in the inertial frame, where the sky is dark.
  The sun is white at its true size in space. In the air the light it crosses reddens it: yellow by day,
  orange to red low down. Its glare makes it look bigger. The sunlight on the ground takes the same colour.
- Sea quads are drawn in 8x8 pieces: a whole quad's chord sagged up to 0.25 m under the sphere and opened
  seams onto the seabed where a neighbour's corner met its edge.
- The view's up stays the local up unless the look is straight up or down. Swapping it at 78 degrees made
  the camera jitter while walking and looking down.
- Streaming ranks requests by distance in metres, holes before refinements; ranked by chunk edges, every
  level's leaves scored alike and loaded in no visible order.
- A step carries the world time the client took it at (`Input::t`); the server steps it at that time when it
  is within two real seconds of its own clock (`phys::step_at`), and the client's corrections replay each
  step at its own time. Out of a body's frame up and the frame change follow where the bodies are, so with
  each side on its own clock a flight at 1 km/s drifted 7 m a second and was corrected all the way.

## Addendum 2026-11-11: F3 in groups, the tick graph, the looked-at label, removing a world
- **F3** is eight titled panels (`hud::Section`, `hud::panels`, `App::debug_sections`): Performance,
  Simulation, Position, World, Player on the left; Drawn, Machine, Session on the right. Labels are dim,
  values white, and a value with a limit is green, amber from three quarters of it, red over it (ADR 0004:
  the frame's worst 1 % against 8.3 ms, RAM and VRAM). The text shrinks (2, 1.5, 1 pixels a dot) before the
  two columns would meet or the left one reach the chat. The bug report's F3 lines are the same rows as
  `[Group]` and `label: value`.
- **Simulation speed:** a local server's ticks are timed in play too (`Server::tick_ms`, drained every frame
  into `App::ticks`, ten seconds of them). The panel shows the typical and worst tick against the 50 ms a
  tick has, the ticks a second reached of 20, and a bar per tick with a line at half a tick (ADR 0004's
  budget for the tick): green under the line, amber over it, red over a whole tick. A remote server's times
  stay there. A played run's JSON line carries no tick numbers.
- **What the crosshair is on** is named top middle with a swatch of its colour, and "gives light" or "metal"
  under it where that holds (`hud::looking_at`); hidden under F3, the menus and the title.
- **The outline of the aimed block** turns slowly through the colours (a turn in 12 s, the hues running
  along its edges) and is three pixels wide (the ghost drawn nine times a pixel apart, `overlay.wgsl`); a held
  dig stays red, a held place green.
- **Removing a world:** right click or Delete on a world of the Singleplayer list asks first; the row that
  removes is never the one under the pointer that asked. The save moves to `saves/removed/` under the
  first free name (`menu::remove_world`): nothing is deleted or written over, and putting it back is moving
  the file. Scripted: `rclick:k`, then `click:` the removing row; `remove` is the Delete key.
- Left and right arrows change values on the settings' pages only; elsewhere Enter presses.
- shortcut: no setting yet to hide the looked-at label or to pick the outline's look; they go on the
  Interface page of `docs/design/settings.md` when it is built.
