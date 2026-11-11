# 0036 – The keys page's groups and search, waypoint pages, the playtest menu, /place

- **Status:** accepted
- **Date:** 2026-11-11

## Context
A batch of the developer's F9 reports (bugs/2026-11-11_13-03-12 to 13-16-39) asked for: a keys page with
groups, a search and clashes in red; a page per waypoint (name, colour, icon, where it shows); a key that
hides the waypoints; a playtest menu on F10, first with an eclipse; the wheel through the hotbar; a /place
command; and /goto without the old words planet and moon.

## Options considered
1. Each as its own system (a text field widget, a waypoint editor window, a debug console) – more to keep.
2. Each on what is there: menu rows, the row being typed into (`App::edit`), the settings file, the chat's
   commands – chosen.

## Decision
- **Keys page:** `Settings::key_rows(query)` builds it: a search row, then each group's heading and its
  actions, then the defaults. A query keeps an action with a word that starts with it or a key named by it.
  Rows are a fixed-width table. `hud::menu` takes a tone per row: red for a shared key, plain text for a
  heading.
- **Waypoints:** a waypoint now has a colour, an icon (diamond, square, plus, ring) and a view: on its own
  body (the old behaviour and the default), always, or within a radius (0.5 to 100 km). The file's line is
  `body|x|y|z|colour|icon|view|name`; the old `body|x|y|z|name` lines still read. Waypoints on another body
  are drawn where that body is now (`World::place`), so "always" works from Luna and from space.
- **Waypoint marks on and off:** the setting `waypoints_shown` (Accessibility), toggled by the action
  `waypoint_overlay`, default K.
- **F10, the playtest menu** (action `playtest`): the eclipse of the sun and the eclipse of Luna. The moment
  is searched forward in the sky's own functions (`nav::next_eclipse`: the closest line-up of Sol, Terra
  and Luna within one sky cycle, in minute steps then narrowed), so it follows whatever the orbits are. The
  clock goes there with `/time set` (20 s early for the sun, so the shadow comes over), the player to where
  it is seen (the shadow's middle on the ground; under Luna), with a line saying how high and which way to
  look. Both searches take 0.5 ms in a release build, so they run on the frame thread when clicked.
- **The wheel:** a cylinder's or cone's height while the hammer holds one, else the flight speed while
  flying, else the hotbar, round its ends. Before, the wheel set the hammer's height whenever not flying.
- **/place <block> [face i j layer]:** the block under the feet, or the block F3's "Feet in" names on the
  body the player is on; it replaces what is there, never a cell a player stands in. Free, like /give.
- **/goto:** Terra and Luna by name, and `home`/`spawn` for Terra's spawn; `planet` and `moon` are gone.

## Consequences
- The playtest menu is the place for later quick set-ups; each is a row and a function.
- A waypoint set to "always" is drawn through the bodies in front of it; that is what "always" means.
- /place costs nothing in survival; the operator role (ADR 0023's shortcut) will have to cover it.
- Review: not reviewed by a fresh reader before this commit (the batch's overseer reviews the branch).
