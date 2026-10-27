# ADR 0092: Cave darkness fades in, lava looks like Minecraft's, the far horizon applies live

Status: accepted (2026-10-27)

## Context

Bug reports of 2026-10-27:

- 17-48-23: "darkness should be much more gradual than this instant switch" (in a cave). The
  underground look (`Atmosphere._underground`: sun out, cave fog, dim ambient) eased towards its target
  at 2.5 per second, so a step into a cave went from daylight to cave dark in 0.4 s. The target itself
  jumps: at a mouth the generator surface over the camera jumps by metres, and the sky share
  (ADR 0082) moves in ninths times three.
- 17-57-29: "make lava just look like minecraft lava". The fluid shader drew dark crust plates with
  thin glowing cracks under a glossy swell: mostly black, read as a wet liquid.
- 18-13-09: a graphics change needed quit and rejoin. The saved settings show the far horizon
  (ADR 0074) being changed; it was the one video setting built only at world load (the tooltip said
  so). Shadows, AO, bloom, fog, brightness and view distance already applied live.

## Decision

1. **The underground look fades over 2.5 s** (`Atmosphere.underground_fade` = 0.4 per second), both
   ways, like eyes adjusting. Teleports and loads still set it at once (`_apply_states(true)`).
2. **Lava is Minecraft's**: `lava_surface` in `terrain_fluid.gdshaderinc` draws a bright orange body
   with darker red-orange blotches and hot yellow streaks from two drifting noise fields, sampled in
   pixels of half a voxel (16 a metre, Minecraft's texel), with a little per-pixel grain. It glows all
   over (emission = colour x `lava_glow` x 0.4), is matte (no specular, roughness 1) and barely
   ripples; a falling face scrolls down. Colours are uniforms (`lava_dark`, `lava_body`, `lava_hot`);
   `lava_crack` and the crust colours are gone. No atlas tile or material id changed.
3. **The far horizon applies live**: `Main._sync_far_layer` runs on every settings change and makes,
   rebuilds (another reach) or frees the far layer. A freed layer (`FarLayer.teardown`) turns `far_on`
   off on the near materials and the sky and gives the sun back its layer; its edited columns are
   saved first and the new layer reads them. A view distance change moves the far materials'
   `near_cut` (`FarLayer.apply_near_cut`). A layer made in play processes while paused, so its tiles
   build and its pipelines compile behind the pause menu, not in play.

## Consequences

- Walking into a cave dims over a couple of seconds; dark corners right inside a mouth stay a little
  lighter for that time.
- Lava pools light nothing around them by themselves (bloom and emission only), as before: a real
  light needs a native query for lava surfaces near the player (sample_cells is too sparse for a pool).
- Toggling the far horizon costs one rebuild of its tiles (seconds of streaming behind the menu).
