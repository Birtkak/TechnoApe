# 0048 – Sound bases and chains

- **Status:** accepted
- **Date:** 2026-11-11

## Context
`docs/design/studio-tools.md`, "Sounds (wave 2)": a row of `data/sounds.toml` may be made from bases and a
chain of changes, each take with its own spread of the chain's numbers, so one base becomes many natural
takes. This is the game's side (schema, steps, rendering, reload, what the Studio's page calls); the page
itself is the Studio's. Rules that hold: everything is rendered once when the card opens (ADR 0032), the
rows made before stay sample for sample the same, no new dependencies, nothing on the frame thread or the
card's callback.

## Options considered
1. **Steps as a table each, named by their key** (`{ lowpass = 1200, q = 0.7 }`), numbers checked against
   one table of steps (name, default, range) – short to write, TOML writes it cleanly, one place to add a
   step; the Studio's sliders read the same table. Chosen.
2. A tagged form (`{ step = "lowpass", hz = 1200 }`) or serde enums per step – longer rows, and serde's
   errors do not name the row.
3. An audio DSP crate (fundsp, dasp) – a dependency for some 300 lines we can test by number.

Spread: `spread = { pitch = 1.5, reverb = { mix = 0.1 } }` (a step's name: its first number; a table under
it: any number by name). TOML's dotted keys (`reverb.mix = 0.1`) read as the same table.

## Decision
- `crates/client/src/audio/chain.rs`: `STEPS` (every step's numbers, defaults, ranges), `Step::parse`,
  `parse_spread`, `apply`, and each step as a pure function: pitch (resample, then back to the length by
  WSOLA), speed (WSOLA: 30 ms grains, half overlap, ±5 ms search), stretch (resample, tape-like), RBJ
  biquads, reverb (four damped combs and two all-passes after Freeverb, sized by its time to fall 60 dB,
  0.3 to 4 s), echo (feedback delay until a thousandth), drive (tanh), tremolo, vibrato (a swinging delay
  sized so the pitch peaks at the depth), fade, trim (1 ms fades at cuts), reverse, gain, crush.
- `audio.rs`: a row has `layer` or `from` (bases by row name, else `data/sounds/base/<name>.wav`), and
  either may carry `chain` and `spread`. `parse` refuses, naming the row: both or neither, an unknown step
  or number, a number out of range, a spread on a step not in the chain, trim or fade in a loop, a row made
  from itself directly or through others. Take k uses take k (mod its count) of each base. The mix of bases
  is set to the row's gain; after the chain a take is only ever lowered to it.
- A loop's chain runs over three laps and keeps the middle one, its tail faded into its head as `render`
  does, so reverb and echo ring round the seam.
- Each take draws its spread from a seed of the row's name and take (a different constant from `render`'s),
  so takes differ and a rebuild gives the same takes. Rows with no `from` and no `chain` go through
  `render` exactly as before; a test holds their samples to a hash taken before this change.
- `Bank::build(rows, dir, old)` makes rows after their bases and keeps the takes of any row whose key (the
  row, its bases' keys, its recordings' file times) is unchanged.
- **Live reload:** the card's thread looks at `data/sounds.toml` and every file under `data/sounds/` twice a
  second; on a change it parses and builds there (`reload`), refusing by name a file that does not parse, a
  row that cannot be made, or a row that is gone (the game may play it), and the old sounds stay. A good
  bank goes to the game's side, which takes it in `hold` (every frame) and sends it to the mixer in order
  with its plays, so row numbers never disagree. The mixer leaves one-shots playing their old takes; a held
  row whose take changed gets a new voice easing in while the old eases out. The card's thread keeps the
  last two banks so their samples are not freed in the callback.
- **For the Studio** (public in the client library):
  `audio::render_row(bank: &Bank, r: &Row, skip: Option<usize>) -> Result<Vec<Vec<f32>>, String>` (every take
  at `RATE`; `skip` a chain step switched off, the other steps drawing the same numbers),
  `Audio::audition(&mut self, samples: &[f32], bus: Bus, distance: f32, reach: f32, dull: f32)`,
  `Bank::build`, `audio::reload`, `audio::chain::{STEPS, Step, Spread, apply}` and each step's function.
- Shipped bases `base.thud` … `base.scrape` (14 rows of layers) and two rows no event plays yet,
  `furnace.burn` (a loop from hum, rumble and hiss) and `furnace.clank`; all on `data/drafts.txt` as
  `data/sounds.toml#<row>` (a file path, `#`, a row's name: one row of a shared file).
- `sounds=1` gains per row `from`, `chain` and `ms` (the row made again, timed), and `load_ms`.

## Measured (dev PC, release build, 2026-11-11, `sounds=1`)
| What | Number |
|---|---|
| Whole bank at the card's opening, before (the old build's `sounds=1` run, start-up included) | 741 to 791 ms |
| Whole bank, after (`load_ms`) | 750 ms |
| Of that, the rows made before (their own `ms` summed; `music.menu` alone 515 ms) | 563 ms |
| The 14 bases and the 2 examples | about 190 ms |
| Heaviest example, `furnace.clank` (4 takes, pitch + 5 more steps) | 100 ms (25 ms a take) |
| `furnace.burn` (a 4 s loop, three laps through lowpass, tremolo, reverb) | 12 ms |
| Mixer's longest call | not measured live (no card opened); its inner loop is unchanged but for a bool read |

## Consequences
- A sound family can grow from a few bases by editing numbers; the Studio's page calls `render_row` on
  every slider move (a light row takes a few ms, the clank 100 ms in release).
- The card opens a little later by what the new rows cost; every chain row adds its own `ms`.
- Shortcuts in the code, each marked: a take is cut at a minute unfaded; with all 32 voices busy a reload
  swaps a held row's take in place (it may click); a cave in the Studio's audition is only duller until the
  cave reverb (sound.md, second cut).
- `pitch` and `speed` (WSOLA) are the costly steps (most of the clank's 100 ms). If the Studio feels
  slow, search a decimated signal first.
- Not seen failing first: the audio-level tests (rows from bases, refusals, reload) were written beside the
  code; the steps' tests were seen red against a pass-through, the reload's no-click test red against an
  in-place swap.
- Review: not yet (the author does not review; a fresh reviewer reads the diff before the commit).

## Review (2026-11-12, a fresh reviewer's findings, fixed)
- The rows were compiled in and the card only took `data/sounds.toml`'s time at start, so a file saved after
  the build (the Studio's every edit) was not heard until it changed again. Decided (the overseer): the game
  reads `data/sounds.toml` from disk at start when it is there and parses (`audio::start_rows`), the
  compiled-in rows when it is missing, and the compiled-in rows with the file named on stderr when it does not
  parse. The files' times are taken before the rows are read, so a save meanwhile is a reload at the first look.
  Parsing at start is on the frame thread once (as effects and models are read); making the takes stays on
  the card's thread. Test `the_rows_start_from_the_file_on_disk`, seen red against the compiled-in rows.
- A reload reached the mixer only through `Audio::hold`, so a run that held no loop never took one: `play`
  takes it too, before the row is looked up (`a_reload_reaches_the_mixer_with_nothing_held`, seen red).
- A row the bank lacks (a model's event or state naming one) hit `debug_assert` in `Audio::play`: now named
  once on stderr and skipped, debug and release alike (`a_missing_row_is_named_once_and_skipped`, seen red).

## Addendum (2026-11-12): a switched-off step is kept
A chain step may carry `on = false` (absent: on). The game checks it, then leaves it out, so the row sounds exactly as
without it (its spread draws nothing); the Studio writes and reads it instead of dropping the card on save. Tests
`a_step_written_off_is_the_row_without_it` (client) and the Studio's chain-edits test, both seen red.
