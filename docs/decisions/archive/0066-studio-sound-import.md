# 0066 – Sounds from outside files in the Studio

- **Status:** accepted
- **Date:** 2026-11-12

## Context
`docs/design/studio.md`, usability round step 5: the developer wants to start sounds from recordings found on
the net, not only from layers and chains (ADR 0032, 0048). The game reads WAV alone (`client::audio::wav`,
hound), and should stay that way: one format, no decoder in the game. The Studio must read the formats such
files come in (WAV, OGG Vorbis, MP3, FLAC), cut and level them, and keep where each came from, since the
developer allows only their own sounds, CC0 and CC BY (the answers at the end of the design doc).

## Options considered
1. **symphonia 0.5** (features `wav pcm ogg vorbis mp3 flac`) – one pure-Rust crate for all four formats,
   one API, gapless MP3 (the encoder's delay and padding cut, so an MP3 is as long as its source). About ten
   small crates. Chosen.
2. lewton + minimp3 + claxon + hound – four crates and four APIs; minimp3 binds C (a C compiler in the build).
3. ffmpeg as a tool – not on every machine, a process per file, and its licence is GPL.

## Decision
- **The decoder:** symphonia, a dependency of `crates/studio` only. The game does not link it.
- **The written format:** 48 kHz (the mixer's `RATE`), one channel, 16-bit integer WAV, written with hound
  (already the game's crate) through a temporary file renamed over the target. Stereo is averaged to one
  channel (the game's sounds are placed in the world, one channel each); other rates are resampled with a
  windowed sinc (Blackman, 16 zero crossings, low-passed under the lower Nyquist), tested on a tone.
- **What the panel does to it, in order:** the cut (two handles), Trim silence (under -40 dB of the peak,
  keeping 5 ms of quiet so the fades fall there), 5 ms fades at both ends, Level to the row's peak (the row's
  code-made peak over its gain; the game multiplies a recording by the row's gain, so a code-made row writes
  full scale). Off, Level keeps the file's own level held under full scale.
- **Where it goes:** "Use as this sound" writes `data/sounds/<row>.wav`, or the next free take
  `<row>.1.wav` to `<row>.8.wav` (what `Bank::build` reads); "Keep as a base" writes
  `data/sounds/base/<name>.wav`, a name no row or base has. Both are one undo step on the open row: undo puts
  the files back as they were (the WAV removed or its old bytes, the credits file as before), redo again.
- **Credits:** `data/sounds/credits.toml`, one `[[credit]]` per recording: `name` (its path under
  `data/sounds/`), `source` (a URL), `licence` (`own`, `CC0`, `CC BY`, or what else was picked), `author`,
  `title`. The panel will not write a file without a licence of the three, nor without a source unless it is
  the developer's own, nor a CC BY one without its author and title. `check=1` (and the Studio's Problems list) refuses every `*.wav` and `base/*.wav`
  under `data/sounds/` that has no row, or whose row's licence is not one of the three, or a CC BY row without its source, author or title.
- **The release duty (the developer, 2026-11-12):** the game is not planned for public release. Before any
  public release every CC BY row must be shown on a credits screen (author, title, source, licence) and a
  release check must refuse to ship without it. The source and licence fields stay in the file for that.
- A one-shot (a row that does not go round) longer than 10 s is warned of in the panel, not refused.
- `sounds=1` names a row's recordings (`recordings`), and `made_in_code` is false for a row that has one: it
  used to guess from the rate and take count, which a 48 kHz recording on a one-take row fooled.

## Consequences
- The game still reads WAV only; a recording from anywhere is one import away. A long file is decoded on the
  Studio's frame thread (marked `shortcut:`); a worker when long files are common.
- A dropped file and a typed path take the same road (`import:<path>` from the command box and `cmd=`).
  `drive` has no drop step; the path field is what scripts use.
- Review (fresh subagent, before the commit): see the last section.

## Review
A fresh subagent read the diff against the design section before the commit. What it found and what was done:
1. **Undo wrote the whole credits file back** as it was before the Use, so undoing an older import dropped
   later imports' rows. Fixed: a Use keeps only its own credits row (before and after); undo sets or takes
   out that row in the file as it is then.
2. **Undo removed the WAV whatever was on disk** (a file polished in another program since would go). Fixed:
   undo and redo touch the WAV only while it is byte for byte what the step left, else refuse and say so.
3. **Steps matched by name only**, and a renamed new row lost its imports. Fixed: each import's step is
   numbered (`Import #n data/sounds/...`) and keyed by the open row; a new row with an import keeps its name.
4. **A long file could panic** (an f32 share of a long length rounded past its end). Fixed: `import::share`
   in f64, held to the length, with a test.
5. **CC BY without author, title or source passed `check=1`.** Fixed: the panel and `check=1` both ask for
   them (the release duty needs them).
6. The drive script was run, not kept in the repo (answered: the evidence is in the session's report; the
   path field and `import:<path>` are what a later test drives).
7. **The Problems list hid other errors** while blocks.toml was broken. Fixed: only the blocks error is
   replaced by the rows left out; any other stays. `open=<block>` for a row the Studio left out still refuses
   to start (answered: it names the block, which is right).
8. A pending import is heard only in the panel, not in the family strip (answered: the panel replaces the
   page's middle while pending, so there is no other preview to show it in; a taste call left open).
9. A zero-length result could be written: refused now. `made_in_code` stays true for a row made `from` a
   file base (answered: it is made in code, from a recording; `recordings` names the row's own files).
