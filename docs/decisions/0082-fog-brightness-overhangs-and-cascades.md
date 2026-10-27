# ADR 0082: Fog and brightness settings, overhangs stay lit, cascades read as water

Date: 2026-10-27. Status: accepted.

## Context

Bug reports from one playtest (2026-10-27_15-15-44, 15-40-26, 15-41-13, 15-41-51, 15-42-49, 15-45-02,
15-49-06; 2026-10-26_15-53-53): a step under a ledge at the bottom of a gorge went from daylight to cave
darkness; fog felt heavy in that gorge and there was no way to turn it off; torches were too soft and gave no
light from the left hand; there was no brightness option; far cascades were a white wall; dug lava was a
mosaic of mismatched plates.

## Decision

1. **Overhangs.** The underground look (ADR 0041 notes) no longer hangs on one vertical ray. `Atmosphere`
   counts the columns at and 2 m around the camera (`SKY_AROUND`, `sky_around_m`) whose vertical ray reaches
   the sky (`sky_open`; a point inside rock is closed) and scales the underground target by
   `1 - 3 x open share`: a third of them open is daylight, a cave with one shaft above stays a cave.
   45-degree rays were tried first; in a 2 m wide gorge they hit the walls and changed nothing.
2. **Mist.** Godot's height fog grows with the depth below `fog_height`, near or far, so a ravine below sea
   level stood in a wall of mist. `fog_height` is now `min(sea level + 6 voxels, camera - 4 m)`: the mist lies
   below the camera. Air fog per km is unchanged (ADR 0071).
3. **Settings (Video tab, `user://settings.cfg`).** *Fog* (on by default) turns the air's haze and mist off;
   caves and water keep their fog. *Brightness* 0..1 (0.5 = the graded look): `Atmosphere.brightness_mult`
   = 2^((b - 0.5) x 2) multiplies the ambient, its square root the exposure. Autopilot runs keep the defaults.
4. **Torches.** `torch_light_energy` 1.3 -> 1.8. The held light follows the offhand (mirrored hand position)
   when the main hand holds no light.
5. **Cascades.** Liquid tops shade with the ripple normal, which points up, so a steep sheet seen head on
   was a grazing mirror of the bright sky. A steep top (the triangle's slope, as ADR 0069 A3) now loses its
   specular and gloss, gets no shore rim, and a third of the old foam.
6. **Lava.** Tops sample the crust field on one horizontal plane (`(x, 0, z)`), sides in world space; tops
   at different heights no longer slice the 3D noise at different levels.

## Consequences

Evidence: before/after sheet at the report spots (river, lava, gorge fog, overhang), kept out of git. The
specular cut is per triangle, so a non-planar quad on a cascade can still differ between its two halves; a
smooth slope needs vertex normals from the godot_voxel fluid patch (not changed here). The brown spikes over
the river cascade at seed 1128002456 are not the fluid and are left open.
