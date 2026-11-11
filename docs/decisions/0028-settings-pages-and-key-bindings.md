# 0028 – Settings as rows of data, key bindings, a frame cap

- **Status:** accepted
- **Date:** 2026-11-11

## Context
The settings menu had five rows, each hand-written in `main.rs`: its text, its click, its slider by row
number, its line in the file (ADR 0023). `docs/design/settings.md` lays out eleven pages; the developer asked
for Display, Graphics, Controls, Keys and Accessibility first, and decided that play caps its frame rate at
the screen's refresh rate (an uncapped loop held 0.93 of a core, ADR 0025 S). Keys were `KeyCode`s in the
input code.

## Options considered
1. More hand-written rows. Each new setting touches five places; the Keys page alone is 29 rows.
2. **Every setting a row of data** (`settings::DEFS`): key, label, kind (choice, slider, stepped number),
   default. Values are all `f64`, a choice its index. The file, the menu's text, the sliders, cycling, the
   JSON dump and the tests come from the table.
3. Capping the frame rate by the present mode (waiting for the display) or by sleeping until the next frame
   is due.

## Decision
- **Option 2**, in `crates/client/src/settings.rs`. A new setting is one line in `settings!` and one entry in
  a page's list. A row can stand on two pages (brightness is on Display and Accessibility).
- **A setting gets a row only when what it changes exists** (the design's rule 1). Built: Display
  (resolution, monitor, mode, frame cap, cap in the background, wait for the display, field of view,
  brightness), Graphics (preset, detail, sun shadows, bounce light, reflections, lamp light, block textures,
  stars), Controls (sensitivity, zoomed sensitivity, invert look, zoom held or pressed once, the double tap's
  time), Keys, Accessibility (interface scale, high contrast, and three rows shared with other pages).
- **Key bindings:** `settings::ACTS` lists every action with two bindings, a key by its place on the board
  or a mouse button, and the input (`App::act`) its press and release make. The held keys are a set of
  bindings; `Settings::held` asks whether an action's is down. Esc, the wheel and the keys inside a text
  field are fixed. A clash is allowed and named on both rows.
- **The file** stays `key=value` with a `version=` line. A bad value keeps that row's default; a line this
  build does not know is written back as it was. `dump=1` prints what a file means as one JSON line.
- **Defaults draw today's picture.** Only play reads the file, and every graphics switch is off at its
  default: `Globals.opt` is 0, the brightness 1, the bounce on. The field of view's default stays 70: the
  gate's views moved by half their pixels when it was 80. High is the default preset (the developer).
- **The frame cap sleeps** until the next frame is due (`App::cap_hz`, in `about_to_wait`), on screen in
  play only: the refresh rate, half of it, a number, or none; 30 when the window is not in front. Waiting
  for the display is its own row, off by default. Off-screen play has no display and no cap, so the gate's
  feel run means what it did.
- **A display change that can lose the picture** (resolution, monitor, mode) opens "Keep these display
  settings?" and goes back after 10 s. Off screen there is no picture to lose and no question; `confirm=1`
  asks all the same.
- **The menu** shows as many rows as fit and scrolls (wheel, or the arrows); arrows and Enter do what the
  pointer does. The interface scale draws the whole HUD in its own pixels.
- **In the shaders:** bits in `Globals.opt.x` (sharp sun shadows, no reflections, flat block colours, no
  stars); `Post.cfg.w` and `Post.tune` (the bounce off with a flat fill from the sky, and the brightness as
  a curve over the finished picture).
- Scripted (`acts=`): `next`, `prev`, `go`, `nudge+`, `nudge-`, `scroll:n`, `bind:name`, `key+:name`,
  `key-:name`, beside `click:k` (a row of the whole page, not of the window shown).

## Checks
- Four tests in `settings.rs`: the file reads back and keeps unknown lines; nonsense and every row at its
  least and most; cycling, sliders and presets; every key has a name, every action a row, every setting a
  page, a clash is named.
- Two scripted off-screen runs with filmstrips: the pages, a preset, scrolling the Keys page, binding
  Forward to the up arrow and walking with it (the feel report's jerk at the press and the release), flat
  colours, the scale at 125 %, high contrast, a display change asked about and taken back.
- `tools/check.sh fast` passes: the six views within 0.01 % of their baselines, the feel run with
  `tests/gate-settings.txt`, `stand` and `fly` in budget.

## Consequences
- shortcut: no render scale, the first lever for a weaker GPU: the composite reads the G-buffer pixel for
  pixel. It needs an intermediate target and a scaled blit; its own package.
- shortcut: the frame cap, waiting for the display, the monitor row and exclusive fullscreen exist on
  screen only and were not run by a script. The developer's first play of the build is their test.
- shortcut: the bounce light's "off" fill is one flat colour from the sky; caves are lit by it too.
- shortcut: the presets hold five rows. Each preset as a benchmark row, and a first-start measurement that
  picks one, wait until a second machine needs them.
- Not built, with their systems: sprint and crouch, auto-step, the space mouse's other mode, sound,
  interface rows beyond the scale, the world's and the server's pages (`docs/design/settings.md`).
- `main.rs` grew; ADR 0025's package L splits it, and the menu's functions are one of its seams.
