# ADR 0041: Lighting grade

Status: accepted (2026-10-24)

## Context
ADR 0037 decision 8 ends with "then three lighting grades to pick from": after the patterned
materials (ADR 0039), the developer picks the look of light and air. Until now `Atmosphere` had one
hand-tuned look: AgX tonemap, a neutral white sun, a blue-grey haze. Every value sat in `_ready` or
inline in the per-frame code, so there was no way to swap the look as a whole.

## Options considered
Three grades, each shot from the same spots (an old-growth forest at 2 pm, a mountain view from
45 m up, a sunset over plains, a cave with a torch) with `/grade` and `/shot` in one run:
- **natural:** the look until now. Neutral, soft, a blue-grey haze.
- **golden:** a warm afternoon like Lay of the Land. ACES tonemap, a warm orange sun, warm haze,
  an S-curve with warm highlights and cool shadows, a little bloom.
- **clear:** thin air, a deep blue sky, crisp contrast, a white sun.
A fourth, **storybook** (soft, bright, lifted shadows), looked too close to natural to be a real
choice and was dropped before the sheet. After the developer picked golden, two variants were shot
against it: bluer sky with a quarter of the fog, and that plus a warmer, stronger sun. The developer
kept the hazy original.

## Decision
1. **Golden is the game's look** (the developer's pick, 2026-10-24), as first built: the warm haze
   stays (fog 2.8 per km, about 500 m of visibility), and the far mountains read as silhouettes.
2. A grade is a dictionary in `Atmosphere.GRADES` over the node's values (tonemap, exposure,
   saturation, contrast, a per-channel curve as a 1D colour-correction LUT, sun and sunset colours,
   sun energy, ambient, shadow opacity, fog colour and density, aerial perspective, sun scatter, SSAO,
   bloom, sky zenith and horizon). Keys a grade leaves out keep natural's values. natural and clear
   stay as alternatives: `/grade <name>` switches live, `grade=<name>` on the command line.
3. Fog is set in metres now (`fog_per_km`, `mist_per_km`). The old per-voxel values came from the
   0.5 m world, so they made the fog four times thicker after ADR 0033. That thicker haze is what
   the developer picked, and the numbers keep it.
4. `/shot <name> [hud]` saves the view to `out/shots/<name>.png` (the run's out folder under
   autopilot), with the HUD and chat hidden, for look comparisons.

## Consequences
- Cost: ACES vs AgX and a 256-texel LUT are free; the budget run holds (p99 3.3 ms).
- The underground and underwater fog still use per-voxel numbers; they were tuned in the 12.5 cm
  world and look right, so they stay.
- Future look passes (palettes, materials, weather) are judged under golden.
- In caves the torch light comes out more orange under golden; acceptable for now.
