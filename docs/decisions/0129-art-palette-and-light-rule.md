# ADR 0129: One palette and light rule for all drawn art

Status: accepted (2026-11-02)

## Context
The developer asked for models, textures and UI that are "even better and all fit our design theme but
more detailed, more colors and more defined". The theme is set (ADR 0037: Lay of the Land palettes for
nature, ADR 0039: patterns for crafted materials, pixel-art item icons), but every generator picked its
own shading: flat noise tiles, grey-only shading, black or no outlines, flat grey menu boxes. Similar
materials (ores, rocks, woods) sat close in value and hue. The in-game lighting grade stays the
developer's call (ADR 0037 decision 8); this ADR is about the art we draw, not the renderer.

## Decision
1. **Hue-shifted ramps.** Every material, item or part is a ramp of shades, not one colour plus
   noise. Shadows turn cooler (toward blue-violet) and a little more saturated; highlights turn warmer
   (toward yellow) and a little less saturated. Neighbouring shades differ clearly in value.
2. **One light, from the top-left.** Pixel art (item sprites, patterned tiles, UI frames) is lit from the
   top-left: top and left edges take the highlight, bottom and right the shade. Models get the same rule
   per part (a lighter top edge or cap, a darker base or underside), the renderer adds the rest.
3. **Outlines from the object's own ramp.** 16x16 sprites get a one-pixel outline in the darkest shade
   of their own colour, never pure black, so icons separate from any slot and from each other.
4. **Saturation is for what matters.** Bulk surfaces (stone, wood, dirt, metal casings) stay muted; ore
   minerals, fire, glow, gauges, liquids and interactive parts carry the saturated colour, so the eye
   finds them. Each family keeps its own hue key: stone neutral-cool, dirt and wood warm, coal blue-black
   with brass, iron rust-red, copper orange with verdigris, tin black with white quartz.
5. **Applied in shared helpers.** The atlas grades every generated tile through one function
   (`grade` in `tools/make_atlas.py`); sprites through the shared shape and palette helpers in
   `tools/make_item_sprites.py`; models through the family palettes (`WorkPalette`, `MobModel`,
   `ApeModel`); the UI through nine-slice skins in `assets/ui/` drawn by `tools/make_ui_skins.py`
   (the "workshop" skin: dark slate panels in a bronze frame with rivets, the existing gold accent for
   hover and selection). New art drawn through those helpers follows the rule without extra work.
6. Tile indices, material ids, UI layout and behaviour do not change; hand-painted overrides in
   `game/art/` still win over everything generated.

## Consequences
- New tiles, sprites and models added in the generators pick up the grade, outline and ramps for free;
  a tile that must keep exact colours (a glow key, a UI swatch) opts out by name.
- The look is one coherent style the developer can still re-grade in one place per generator.
- The world lighting grade (fog, sky, exposure) remains open for the developer.
