# NNNN – The Studio looks like a modern creative tool

- **Status:** accepted
- **Date:** 2026-11-18

## Context
The developer, after the Maps page: the Studio still looked "the same and bland", and "more like this"
felt clunky; "think more modern Adobe tools". The Studio was a dark egui frame in the game's pixel font,
with bluish greys, framed buttons on every action, coloured bands on section titles and a variations wall
whose actions sat under each picture.

## Options considered
1. Keep egui's default look and retune colours only: cheap, but the font, the framed buttons and the
   bands are what read as dated.
2. A full Adobe Spectrum (dark) restyle through the shared helpers (`ui/look.rs`, the frame in
   `ui/mod.rs`, `ui/wall.rs`): every page changes without touching its own code.
3. A different UI library: a rewrite of eleven pages, for a look.

## Decision
Option 2.

- **Colours** are tokens in `ui/look.rs`, neutral greys with no blue in them: work area `#1e1e1e`,
  panels `#2a2a2a`, raised and hover `#353535`, pressed and chosen `#434343`, hairlines `#3d3d3d`, field
  borders `#525252`; text `#e6e6e6`, secondary `#b0b0b0`, disabled `#6e6e6e`; accent `#2680eb` (hover
  `#378ef0`); positive `#33ab84`, notice `#e68619`, negative `#f0626a` (the brief's `#2d9d78` and `#e34850`
  made a step lighter so small status text keeps 4.5:1 on the panels).
- **One accent** for the main action (one filled pill: Try in game, Save when dirty, Use, Import) and for
  focus (a field's 2 px ring, the chosen node). Page colours stay for where you are (tab dot and underline,
  the 2 px bar of a section title), never for what is chosen.
- **Font:** Segoe UI Regular and Semibold read from the system's font folder at start (no font file in the
  repository, no new crate); egui's own font where they are missing. Body 13, small 11.5; titles and tabs
  semibold. The game's pixel font stays as a setting (View > Interface font: Clean | Game, `font=game` in
  `studio.txt`, Clean when absent) with its old sizes and pixel-sharp scale steps; the clean font's scale
  menu offers 80 to 200 %. A scale kept from the game font is snapped to the nearest of those at the first
  frame (133 % becomes 125 %).
- **Controls:** quiet buttons (a fill on hover only), corners 4 on controls and 8 on cards, menus and
  windows, soft shadows under menus and windows, text fields with a 1 px border, sliders with a 2 px track
  and a round handle, chips as one segmented group, tooltips with the key in a keycap.
- **Frame:** a 40 point top bar of plain-text tabs (the open one white, underlined in its page colour), a
  search-like "Find… Ctrl+P", View, Keys, the scale and Try in game; flat section headers with hairlines;
  the open file's row chosen grey with an accent bar; "draft" as a small amber pill; a slimmer status line.
- **The variations wall** has one action bar above its grid (Use and Cancel where the page's pick waits,
  More like this for the picked one, New batch, Back, How different from subtle to wild) instead of a
  "More like this" under every picture; the pictures are large, even, wordless, picked by a click and used
  by a double click, with a round "more like this" and play button on hover. `wall::Act` keeps its
  variants and gains `Use` and `Cancel`, so pages that drew their own Use and Cancel (Maps, Textures) let
  the wall draw them.
- egui's widget themes (`StyleProvider`, for the selected and field looks of buttons and text fields) are
  behind its `experimental` feature: the Studio names eframe's own egui with that feature on. It is the
  same crate already built, not a new one.

## Consequences
- A page that wants a main action uses `look::primary`; one per screen. Stray greys in frame code use the
  tokens; a page's own drawing may keep its colours.
- The experimental theme API may change with an egui update; it is used in one place (`look::Theme`).
- Segoe UI exists on every Windows; elsewhere the Studio falls back to egui's font.
