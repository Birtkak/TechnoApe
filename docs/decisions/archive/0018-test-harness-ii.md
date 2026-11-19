# 0018 – Test harness II: viewpoints by number, a crack detector, the feel report

- **Status:** accepted
- **Date:** 2026-11-10

## Context
`docs/design/fundamentals.md` section 4 has five steps on its testing ladder. Steps 4 and 5 are the first
visuals that can break: fixed viewpoints compared with stored images by pixel count, a crack detector, and
section 1's feel report (corrections, camera jerk, click-to-change). Claude should read an image only when
a number moves.

## Decision
- **Shots:** `run=stand ... shot=file.png` renders one frame a second after the world has loaded.
  - It clears to magenta, a colour no block or sky has, and counts the magenta pixels whose view ray passes
    inside a body's lowest possible terrain (radius minus `AMP`, minus 1 m). Those are cracks.
  - It stores the frame at a third of its size (640 x 360). With `baseline=`, it counts the pixels that
    differ by more than 8 of 255 in a channel.
  - JSON field `shot`: cracks, the first crack pixels, the difference in percent.
- **Viewpoints:** `tests/views/views.txt` lists name, allowed cracks and pose: a seam, a cube corner, an LOD
  view from 120 m, 400 m, and orbit. `tools/views.sh` runs them (about 10 s) and fails on more cracks than
  allowed or over 0.5% of pixels changed. `tools/views.sh accept` stores new baselines after a look.
  - There is no cave view: the terrain has no caves yet.
- **Feel report:** `run=play` adds a `feel` field with:
  - corrections, with time and size
  - drift
  - jerk: p50, p99, max and the five worst with their times, from the fixed-step physics path. The rendered
    camera interpolates linearly between steps, so its jerk would only measure the interpolation.
  - clicks and click-to-change: frames and ms from a predicted edit until every full-detail chunk that
    samples its cells draws the new version (`Streamer::shows`)
- **The gate** runs the viewpoints and a scripted 10 s play run: auto-walk over hills, digs, places and a
  hammer cylinder. Every click must show within 16.7 ms (one 60 Hz frame) and at most one correction is
  allowed. The gate now takes about 1 min 40 s.

## How the checks were checked
- Two shots of one pose differ by 0.000%: renders are bit-identical on this machine.
- With LOD skirts off (`SKIRT = 0`), the LOD view counts 210 crack pixels. With them on, it counts 4.
- Grass made slightly redder (+0.06 red) moves every view by 5-46% of its pixels. A threshold of 32 per
  channel missed this, so the threshold is 8.

## What it found
- **A pinhole crack** at the LOD view: 4 pixels near the bottom edge (pixel 1146,1074 of 1920 x 1080). It is
  not a T-junction of the default mesher: greedy meshing makes it far worse (406 pixels with `greedy=1`).
  Fixed 2026-11-10. The cause was a vertical detail-level border: full detail above layer 128, half detail
  below it. The skirt rule turned side and bottom padding near air into air, never the top padding. So the
  coarse chunk took its top padding for solid (sampled at its coarser column centres) and drew no top face,
  while the fine column above was air there. The top padding now follows the same rule, from the column's
  generated height, since the cells above it are outside the chunk. The view allows 0 cracks again, and eight
  more viewpoints at 40-250 m show none.
- **A start-up correction:** the client walks before the server has its first input, and it compares its
  present position with the server's older one. In auto-walk with jumping this crosses the 1 m snap
  threshold about 0.3 s after loading, in some runs. Fixed 2026-11-10 by ADR 0019 (numbered steps,
  replay from the server's state, eased corrections); the gate now asks for 0 corrections.
- **Landing jerk** (up to 2.4e5 m/s^3 at the first landing) is reported, not gated. It is what section 1's
  landing dip and step-up easing will smooth.
- Clicks show within 2 frames, 1.3 ms, off-screen at about 3000 fps.

## Consequences
- A change that moves pixels on purpose needs a look at `out/views/*.png` and `tools/views.sh accept`. The
  five baselines are 1.3 MB in the repository.
- A new GPU driver can move pixels. The first gate after one may need an accept.
- `acts=` scripts in `run=play` are how the gate plays. New controls get an act.
