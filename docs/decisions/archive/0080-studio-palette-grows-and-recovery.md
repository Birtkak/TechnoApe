# 0080 – The palette grows where you paint; recovery on the Models and Sounds pages

- **Status:** accepted
- **Date:** 2026-11-12

(0068 and 0069 are taken on branches not yet merged, so this ADR takes the next number free on every branch.)

## Context
`docs/design/studio.md`, second round step 8: "colours for textures feel limited". The call was that the one
palette stays (every pixel and voxel a palette colour) and grows in place: a colour is added, given shades and
found where the painting happens, not only on the Palette page. Separately, "The bar" asks that unsaved work
is kept every few seconds and offered back after a crash; ADR 0064 did that for textures only.

## Options considered
1. **A second copy of the palette logic in the swatches** – quick, but two places that check names, write the
   file and keep undo, and two histories that disagree. Refused by the task.
2. **The swatches send commands to the Palette page** (`add:<name>=#rrggbb`, `shades:<name>`, `remove:<name>`,
   `undo:<step>`), which the frame runs after the page is drawn. The checks, the file, the drafts list and the
   undo stay that page's. Chosen.
3. Moving the palette's history into `App` – the cleaner home in the long run, but it moves code the Palette
   page owns for no gain today.

For recovery: the frame's texture recovery stays as it is (ADR 0064); a page with work of its own gets three
`Page` methods (`kept`, `unsaved_of`, `restore`) and its own set `studio-recovery/<page>/`, kept, offered and
cleared by the frame the same way.

## Decision
- **Under every palette's swatches** (Textures' Colours, Models' Colours, the pattern panel's wells; one
  function, `textures::swatches`, so all three at once): a search box over colour and ramp names (the swatches
  stay grouped by ramp), the count, and a row "+ New colour", "Add shades", "Remove", plus "Undo: <what>" after
  a change made there. `ui/grow.rs`.
- **+ New colour:** hue, saturation and value sliders, a hex box and a name. The name is suggested from the
  hue and lightness ("dark blue", "pale green", free in the palette: `ramp::suggest`) and follows the colour
  until it is typed in. Add is greyed, with the reason in red, while the name or the colour is already taken.
  Add puts the colour at the end of the palette and makes it the first colour (or fills the armed well).
- **Add shades:** `ramp::shades` makes `ramp::make`'s ramp of five around the first colour and adds its four
  new shades, "<name> darker", "dark", "light", "lighter" (numbered when a name is taken). They join the
  colour's ramp, each before the first colour lighter than it, so the ramp's own order is kept; a loose colour
  gets a new ramp named after it, the colour at its middle. Shade and the patterns' auto colours walk ramps, so
  they use the shades at once. The preview shows the whole ramp, new shades marked, before Use.
- **Remove** is the Palette page's remove, now with the count: "N file(s) use it (...)". In use means a texture
  in memory or on disk, or a saved model file whose `colours` names it (`palette::models_using`). Before this,
  only textures were looked at, so removing a colour a model names broke that model.
- **Undo** sends `undo:<step>` with the step's own name ("Add plum", "Remove plum", "Add shades of plum"); the
  Palette page undoes only when its last step is that one, so the button never undoes another change (a later
  one, or the Palette page's own Add), and after a refused Remove it is refused too. Each page has its own
  search and Undo (Textures, Models, the wells).
- **The count says what is true:** the design doc's "N of 255" was not. The palette has no limit (textures hold
  RGB, models name colours); one model holds at most 62 colours, a letter each (`engine::models::LETTERS`). The
  swatches show "N colours" with that in the tip; the Models page shows "This model: k of 62 colours", amber
  from 56.
- **Recovery:** Models keeps `<name>.toml` with the text a save writes; Sounds keeps `<row>.toml` with the row's
  text. A file equal to the saved one is not offered. Restore puts each back as unsaved work, one undo step
  ("Recovered"); a new model or row stays new. A kept file the page's reader refuses (a colour removed since the
  crash, say) is set aside in `studio-recovery/<page>-refused/` and named in a red toast, never dropped. A clean
  close clears every set; a shot leaves them alone.

## Consequences
- The palette's history is still the Palette page's: the Textures page's undo list does not show a colour
  added there; the "Undo" button under the swatches and the Palette page's history do.
- An unsaved model's new colour is not seen by the in-use check (only saved model files are read): removing it
  makes that model's save refuse, with the reason; the recovery set still holds its text.
- Review: the diff is over 300 lines (principle 13), so a fresh reviewer read it. It found no crash or data
  corruption and six points, all fixed: the swatches' Undo matched steps by a generic name (could undo another
  change); the Palette page read every model file each frame for its Remove button (now at most once a second);
  a refused recovered file was lost (now set aside); `models_using` missed single-quoted names and matched keys
  like `colours_x`; Textures and Models shared one search and Undo; the count did not say how many a search
  shows ("k of n colours" now).
- The drive script must scroll the inspector (`wheel`) and let a toast fade (`wait 20`) before clicking in its
  bottom corner: the Colours section sits low, and a toast stands over that corner for 4 s.
