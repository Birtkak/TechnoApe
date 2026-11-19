# Effects and Sound

Particle effects, model states, sound rows, our own mixer, sound bases and chains, cave reverb, the Studio's
Sounds and Effects pages and sound import. What holds today. Numbers in brackets are the ADRs
(`archive/NNNN-*.md`) a decision came from; INDEX.md lists every number.

## The developer's calls
- Sounds come "both from the start": made in code and from recordings dropped in [0032].
- Blocks take time to dig in survival, as in Minecraft (hardness and tool, cracks while dug, creative instant);
  the effects' break burst is one look, cubes of the block's texels, with no second candidate [0031; built
  by 0034, see survival docs].
- Only the developer's own sounds, CC0 and CC BY may come in as recordings [0066].
- The game is not planned for public release. Before any public release every CC BY row must be shown on a
  credits screen (author, title, source, licence) and a release check must refuse to ship without it [0066].
- Music plays at the title in place of wind (a loop may hold notes; a fourth bus and slider, Music) [0032].
- The game reads `data/sounds.toml` from disk at start when the file is there and parses, so a save made after
  the build is heard (the overseer's decision in 0048's review) [0048].

## Particle effects (`crates/client/src/fx.rs`, `data/effects.toml`)
- One system for every short-lived thing: debris, puffs, droplets, small motions. Rows from `data/effects.toml`
  (each field documented there, checked by `fx::parse` and a test). Built-in rows the code plays: `break`,
  `place`, `grow`, `spray`, `splash`, `chip` [0031, 0050].
- Pool: a fixed `CAP` of 1,024 / 4,096 / 16,384 by the Particles setting (few / normal / many), followed live
  in `run=play`. A full pool replaces the particle nearest its end among the next 32 from a moving cursor. A row
  of one plays one at every scale. At most 32 cell events a frame play; past them a break still drops what lies
  on its cell (`fall_through`). No allocation per frame [0031].
- Motion is in the body's frame, relative to its centre, under its own pull towards the centre, with drag. The
  floor is the top of the first solid block in the 8 under the particle's column (8 m with 1 m blocks); with none
  found the lookup goes on from where it stopped. On leaving its column a particle looks up the new one (one
  terrain height a crossing) or bounces off it if that cell is solid. A cell broken under resting debris drops it
  to the floor below. Debris collides only with the floor of its column and the side of the next, not overhangs or
  the player [0031].
- Draw: its own pass after the G-buffer's, one instance of 36 vertices per particle (debris a turning cube of the
  block's texels, a square a billboard, `block` the whole cell from the grid's corners), lit per fragment, never
  in the TLAS: particles cast no shadow. Particles are opaque in the G-buffer (smoke is grey squares, not haze;
  a blended pass would be a choice of its own). GPU stage `fx` and frame-thread stage `fx` [0031, 0050].
- Breaks come from deltas: `fx::changed` plays one `break` per solid-to-air change and one `place` per air-to-solid,
  whether one's own prediction, the server's answer or another player's edit. Debris size: `break` 0.1-0.18 m,
  `place` 0.04-0.07 m at 1 m blocks (16 texels to a block) [0031].
- The placed block grows: a `grow` particle draws the block from 0.05 of its size up; the real mesh waits
  `GROW_S` = 0.1 s and the grow ends when the mesh shows it, the cell changes again, or its 2 s particle ends. A
  click counts as shown at once, so `click_to_change_ms_max` measures a dig's remesh, not a place's [0031].
- Tweens and tracks (`fx::Tween`, `fx::Track`: linear, in, out, in-out, pulse, spring, shake): the block in hand
  dips on use (0.18 s) and bobs with the steps (one pulse per footstep count, `BOB` is `STEP_M` long, `walked`
  reset at each footstep) [0031].
- The water's droplets are `spray` particles that splash where they land; `water.rs` keeps only where they start
  [0031]. (also sky-weather-and-water.md)
- When effects play: `run=play`, `run=fx`, or any run with `fx=1`. Off, the pool is empty and nothing is drawn,
  so other runs and the reference views are unchanged [0031].
- Cracks and dig chips go through `Fx`; a run with effects off shows no dig progress [0031, see 0034]. (also
  mining-and-digging.md)
- `run=fx` is the benchmark: bursts of `break` 2-14 m ahead until `fx_n` (default 4,096) are alive; `fx_cap=` sets
  the pool; `fxedit=t:di:dj:dl:block,...` scripts edits at times for filmstrips (`air` to dig; entries in any
  order). It is in `tests/perf/baseline.json`, the full gate and the benchmark's small series [0031].
- Budget stated: frame thread `fx` proposed 0.05 ms (measured 0.06 ms p50, accepted as a shortcut; levers: step
  on a worker with a double buffer, or move the motion to the GPU); GPU `fx` 0.15 ms at 4,096 live, which 1 m
  blocks reach (levers none taken: one sun visibility per particle, fewer debris a block, a lower cap at
  "normal") [0031].

### Rows grown by 0050
- New optional row fields: `rate`, `start` ring (`radius`) / cone (`angle`, `dir`) / box (`box`), `gradient`
  (palette names via `fx::PALETTE`), `size_curve`, `bright_curve`, `noise = { strength, scale, speed }` (smooth
  value noise), `rise`, `spread` (a steady push each particle keeps), `glow`, look `model` and `texture`, `face`
  camera / flight / tumble; `count` is optional; `[[group]] name, rows`. Also `colour`, `then` [0050, 0051].
- Looks: a texture look is the top face of a named block (painted or its pattern) from the buffer the terrain
  reads; no new buffer. A model look is drawn by the models' own draw (one more instance list) [0050].
- GPU particle stays 32 bytes: `info` bits 16-17 the face, 18 the texture, 24-31 glow x 32; a flight square's `a`
  is its direction (3 x 10 bits) and `angle` its streak (a 30th of a second's flight); a tumbling square is the
  debris cube a tenth as thick; a camera-facing square turns by its `angle`. A model-look particle is turned
  with `(t, up, s)`, never mirrored [0050].
- The pool owns its `Set` (rows, groups). API: `Fx::start(name, at) -> Option<Handle>`, `moved`, `stop`,
  `restart`, `play_named`, `Fx::show`, `Fx::swap`. Lasting rows emit `rate x scale x dt` a step (a fraction
  carried). `start` keeps an emitter for a count-only row too, so `restart` bursts it again [0050].
- Glow: emission per particle plus one lamp per lasting effect that glows, through the lamp list (its rows' glows
  added, mean gradient colour x glow x 0.25, flickering by the noise when the row has noise), added to the
  frame's lamps after the world's (`cap_lamps` keeps the nearest). Only lasting effects give a lamp; a burst
  lights nothing round it [0050].
- Live reload: `Fx::watch` puts `data/effects.toml` on a `gpu::watch` thread; the frame thread swaps the set at
  its next `step`; living particles and lasting effects keep their rows by name; a refused file, or one without a
  row the code plays (break, place, grow, spray, splash, chip), is named on stderr and the old rows stay
  [0050, 0134].
- `fxplay=name[:n]` starts n lasting copies on the ground ahead (a fan). `effect <name>` / `effect -` in the
  view protocol shows a row or group on the stand cell [0050, 0051].
- Draft rows (`data/drafts.txt`): `flames`, `embers`, `smoke`, group `fire`; `sparks`, `steam`, `dust`, `drip`,
  `glint`, `leaves` (look model `leaf`, `data/models/leaf.toml`) [0050].
- Cost stated: fifty fires (2,671 particles, 50 lamps) spend 0.19 ms of the frame thread, mostly the noise.
  shortcut: at hundreds of fires sample the noise at half rate or step on a worker [0050].

## Model states (`crates/engine/src/models.rs`, `crates/client/src/models.rs`)
- `Life { states, events }`, `State { name, clip, effects: [{effect, point}], sound, light: {point, colour,
  strength} }`, `Event { t, sound, effect, point }`. A model file gains `[[state]]` tables and timed clip events.
  The life is read from the model file into a separate `Life` (`parse_life(file, text, palette, effect?,
  sound?)`); `Model::parse` takes the `state` and `event` keys out before its strict read; `write_life` puts them
  back. **The Studio must save a model through `write_life(&model, &life)`** or it loses its states and events
  [0050].
- `parse_life` refuses by file and reason: a state or event naming no clip, point, effect or sound, a light
  colour not in the palette, an event outside its clip or with nothing to play. A state's effects and sounds are
  checked against the files in use (`effects=`, `sound_dir=`), not `data/`'s [0050].
- `events_between(clip, events, t0, t1)`: each event once a pass of a loop (at t, t + length, ...), once in all
  otherwise. `Model::point_at(poses, name)` [0050].
- Client: `model=name@state` and `Models::set_state(cell, state)` put a model into a state: its clip plays from 0,
  its lasting effects start at their points and follow the parts (`Fx::moved` each frame), its looping sound is
  held (`Audio::hold`) at the nearest one's loudness by its reach, its light is a lamp at its point, its clip's
  events play once a pass. A state no longer asked for stops its effects (their particles live out). A clip
  with events and no state (`model=boar:walk`) fires its events as a state "" [0050].
- A reload while a state plays stops that state's effects and starts them again from the new file (the clip's
  time kept); with no models left, every state stops and its sound is let go [0050].
- The draft furnace: points `chimney` and `door`, state `working` (clip `work`, fire at `flame`, smoke at
  `chimney`, `furnace.burn`, a `lamp` light of 0.7), events at t = 2.0 (door shut): `furnace.clank` and `sparks`
  at the door [0050].
- Placed blocks are put into a state by code; no game rule does it yet (machines will) [0050].
- shortcut: a state's looping sound has no direction (a bus-wide loop at the nearest one's loudness); pan it when
  the mixer holds a loop at a place [0050].

## Sound: rows and our own mixer (`crates/client/src/audio.rs`)
- Our own mixer on `cpal` (the sound card only), not `kira` or `rodio`: the same `Mixer::mix` runs on the card's
  thread and in tests, so loudness, place, voice cap and end silence are tested by number [0032].
- Pieces: `data/sounds.toml` rows, `render` (layers of filtered noise and tones with a fade), recordings by name
  from `data/sounds/` (`hound`, WAV only), `Mixer::mix`, and `Audio`, the game's side, which works out what the
  head hears and sends the mixer a message. The voice cap is 32 [0032].
- Every take is made when the card opens, on the card's own thread; the frame thread sends messages only [0032].
- A block names its material's sound (`sound` in `data/blocks.toml`, stone when none); a test
  (`every_block_has_its_sounds`) holds every block to having `dig.`, `place.` and `step.` rows. A new block,
  machine or creature gets sound by naming rows; a recording replaces a row without code [0032, 0054].
  (also items-and-crafting.md)
- Sounds start from cell events: the player's own dig or place from the first cell it changes (`edit::changes`),
  another player's from the server's delta through `fx::server_edit`, which reads a cell's old block only with
  effects on or for the three cells of a batch that are heard. A hammer's sound is the first changed cell's block.
  Steps, landings and the menu keep their own call sites [0031, 0032].
- Off-screen runs open no card (`sound=1` does) and only count what would have played, so no benchmark or view
  moves [0032].
- Placement: a sound has a place against the head (pan, the far ear late, quieter with distance and gone at the
  row's reach, duller behind; the head's frame is the player's right, up and ahead on the planet) [0032, 0054].
- Music at the title in place of wind; takes never repeat and vary more; footsteps of heel and toe; a thump by
  the metres fallen. Rows are refused by `parse` for a loop whose steady layer with a delay writes past its
  buffer, or a loop shorter than a sample [0032].
- A card that is lost ends its thread and is tried again every two seconds [0032].
- `sound =` in `data/blocks.toml` changes the blocks' hash [0032]. (also multiplayer-saves-and-data.md)
- Latency stated: the 20 ms of `fundamentals.md` is not met on the dev PC's card (44 ms at worst, wireless
  headset, shared system mixer; the message waits at most one 10 ms buffer). Ways down: the system's low-delay
  shared mode, or exclusive mode as a setting. Mixer's longest call measured 63 us per 10 ms buffer [0032].
- shortcuts in the code: a sound's place is fixed against the head when it starts (moving sources need an update
  message); WAV only; a card that does not take floats stays silent [0032].

## Sound bases and chains (`crates/client/src/audio/chain.rs`)
- A row has `layer` or `from` (bases by row name, else `data/sounds/base/<name>.wav`), and either may carry
  `chain` and `spread`. Steps are tables named by their key (`{ lowpass = 1200, q = 0.7 }`), checked against
  `STEPS` (name, default, range), which the Studio's sliders also read. Spread:
  `spread = { pitch = 1.5, reverb = { mix = 0.1 } }` (a step's name gives its first number; a table under it any
  number by name); dotted keys read as the same table [0048].
- Steps: pitch (resample, then back to length by WSOLA), speed (WSOLA: 30 ms grains, half overlap, +-5 ms
  search), stretch (tape-like resample), RBJ biquads, reverb (four damped combs and two all-passes after
  Freeverb, sized by time to fall 60 dB, 0.3 to 4 s), echo (feedback delay until a thousandth), drive (tanh),
  tremolo, vibrato (a swinging delay sized so the pitch peaks at the depth), fade, trim (1 ms fades at cuts), reverse, gain, crush [0048].
- `parse` refuses, naming the row: both or neither of `layer`/`from`, an unknown step or number, a number out of
  range, a spread on a step not in the chain, trim or fade in a loop, a row made from itself directly or through
  others [0048].
- Take k uses take k (mod its count) of each base. The mix of bases is set to the row's gain; after the chain a
  take is only ever lowered to it. A loop's chain runs over three laps and keeps the middle one, its tail faded
  into its head. Each take draws its spread from a seed of the row's name and take, so a rebuild gives the same
  takes. Rows with no `from` and no `chain` go through `render` as before (a test holds their samples to a hash)
  [0048].
- A chain step may carry `on = false` (absent: on): the game leaves it out and the row sounds as without it (its
  spread draws nothing); the Studio reads and writes it [0048].
- `Bank::build(rows, dir, old)` makes rows after their bases and keeps the takes of any row whose key (the row,
  its bases' keys, its recordings' file times) is unchanged [0048].
- Live reload: the card's thread looks at `data/sounds.toml` and every file under `data/sounds/` twice a second;
  on a change it parses and builds there (`reload`), refusing by name a file that does not parse, a row that
  cannot be made, or a row that is gone; the old sounds stay. A good bank goes to the game's side, which takes it
  in `hold` (every frame) and in `play`, and sends it to the mixer in order with its plays. One-shots keep their
  old takes; a held row whose take changed gets a new voice easing in while the old eases out. The card's thread
  keeps the last two banks. At start the game reads `data/sounds.toml` from disk when present and parsing
  (`audio::start_rows`), the compiled-in rows when missing, and the compiled-in rows with the file named on
  stderr when it does not parse; the files' times are taken before the rows are read [0048, 0134].
- A row the bank lacks (a model's event or state naming one) is named once on stderr and skipped, debug and
  release alike [0048].
- Public for the Studio: `audio::render_row(bank, r, skip) -> Result<Vec<Vec<f32>>, String>` (every take at
  `RATE`; `skip` a step switched off, the others drawing the same numbers), `Audio::audition(samples, bus,
  distance, reach, dull)`, `Bank::build`, `audio::reload`, `audio::chain::{STEPS, Step, Spread, apply}` and each
  step's function [0048].
- Shipped: bases `base.thud` ... `base.scrape` (14 rows), `furnace.burn` (a loop) and `furnace.clank`; listed in
  `data/drafts.txt` as `data/sounds.toml#<row>` (a file path, `#`, a row's name) [0048].
- `sounds=1` reports per row `from`, `chain`, `ms` and `recordings` (a row's own files), and `load_ms`;
  `made_in_code` is false for a row that has a recording [0048, 0066].
- Cost: whole bank at the card's opening about 750 ms (`music.menu` alone 515 ms); `furnace.clank` 100 ms.
  shortcuts: a take is cut at a minute unfaded; with all 32 voices busy a reload swaps a held row's take in
  place (it may click). `pitch` and `speed` are the costly steps (if the Studio feels slow, search a decimated signal
  first) [0048].

## Cave reverb (`crates/client/src/audio.rs`)
- `room(solid, ear) -> (wet, size)`: 14 rays (the frame's six ways and eight corners) from the head, standing on
  the body's up (not the look), each out to the face of a cube 24 m from the head (a corner ray sqrt 3 times as
  far), 1 m steps, measured every 250 ms on the frame thread. The share that meet a solid cell (the block's
  `solid`; a row's `soft = true` (leaves, shrub, reed) is no wall) under half (a field: only the ground's five)
  gives no reverb; all full. `size` is the mean distance of the sideways and upward hits [0054].
- `Reverb`: the Freeverb shape of `chain::reverb` run sample by sample, each ear's 23 samples longer; rings 0.4 s
  (a burrow) to 3 s (a 24 m hall); at most half as loud as the sound; only the World and Steps buses feed it (the
  interface and music are in the head). Its share and feedback ease (no click); fully out it is not run and its
  buffers are cleared; a 1e-20 floor on the comb input stops denormals [0054].
- `Msg::Room { wet, size }`; `Audio::listen` (each frame in `frame.rs`, measures every 250 ms, sends on a change
  of 0.02 or 1 m); `"room":{"wet","size","us_max"}` in every run's `audio` JSON. No rays without a card (an
  off-screen run measures with `sound=1`) or more than 24 m over the build headroom (`Body::over_top`): there the
  place is the open [0054].
- The reverb is one for all sounds: a sound in a cave heard from outside rings by the listener's place, not its
  own [0054].
- Cost stated: one `room` at worst about 0.17 ms every 250 ms on the frame thread. shortcuts: rays treat water as
  air and cactus and coral as walls; move `room()` to a worker if `us_max` ever shows in the frame budget [0054].
- Open: sound behind a wall duller and quieter (one ray per voice), water, machines, weather, voice, subtitles,
  output device and speakers settings [0032, 0054].

## The Studio's Sounds and Effects pages (`crates/studio`)
- A save touches only its own row (`rows.rs`): a row runs from its `[[header]]` line to its last line of its own
  (blank lines and comments after it belong to what follows). The save puts the new text there in the file's own
  line endings, every other byte unchanged, checks the whole file with the game's reader (`audio::parse`,
  `fx::parse_set`), and writes through a temporary file and a rename. A new row goes at the end after one blank
  line. Keys the page does not edit (a sound's `layer`) and inner comments stay as written [0051].
- Sounds page (`sound.rs`, `ui/sounds.rs`): edits `from`, `chain`, `spread`, `variants`, and the row's `bus`
  (chips), `gain`, `reach` and `pitch` (sliders held to `sound::SCALARS`, a default not written). A card
  switched off is heard without (one off: `render_row`'s `skip`; several: left out) and is not saved (the 0048
  `on = false` key is a separate way to keep it in the file). Rendering runs on one worker thread, newest job of
  each kind wins, with a bank of only the rows the rendered rows are made from. Mutate moves each number by up to
  a share of its range (frequencies by octaves), deterministic for the wall's seed. Starts are ten fixed chains
  (`STARTS`). Loop rows are re-sent each lap, so an edit is heard from the next lap. In place's cave is a reverb
  step plus a little dullness [0051, 0048].
- Effects page (`effect.rs`, `ui/effects.rs`): writes its copy of the whole file to
  `out/studio-view/effects.toml` and the renderer plays from it. View protocol (`crates/client/src/view.rs`):
  `effects <file>` (swapped in by `Fx::swap`) and `effect <name>` / `effect -` (`Fx::show`). The Studio starts the
  renderer with `fx=1`. A page that does not set `Viewport::effect` shows none. A group's row switched off is not
  saved in it. The wall's previews are small closed-form sketches, not the game's renderer [0051].
- Effects inspector: `colour` (three sliders, for a square or texture without a gradient; "one colour" turns a
  gradient into its first stop's colour), a cone's `dir` (three sliders that never reach length 0), `then` (a
  pick of the file's saved rows), `look = "model"`/`"texture"` with a pick of `data/models/*.toml` or the game's
  blocks. `effect::choose_look` drops what the game takes only with another look (gradient and glow off squares
  and textures, a face off debris and blocks, white for a square with no colour), so the page never holds a row
  the game refuses. Under 220 points wide a label stands over its slider [0051].
- A shot never opens the sound card or writes into `data/`; the page never waits on a render [0051].

## Sound import in the Studio (`crates/studio`)
- Decoder: symphonia 0.5 (features `wav pcm ogg vorbis mp3 flac`; gapless MP3), a dependency of `crates/studio`
  only; the game does not link it and reads WAV alone [0066].
- Written format: 48 kHz (the mixer's `RATE`), one channel, 16-bit integer WAV, written with hound through a
  temporary file renamed over the target. Stereo is averaged; other rates resampled with a windowed sinc
  (Blackman, 16 zero crossings, low-passed under the lower Nyquist) [0066].
- The panel does, in order: the cut (two handles), Trim silence (under -40 dB of the peak, keeping 5 ms of quiet),
  5 ms fades at both ends, Level to the row's peak (the row's code-made peak over its gain; off, the file's own
  level held under full scale). A zero-length result is refused; a one-shot over 10 s is warned of, not refused;
  `import::share` works in f64 held to the length [0066].
- "Use as this sound" writes `data/sounds/<row>.wav`, or the next free take `<row>.1.wav` to `<row>.8.wav`;
  "Keep as a base" writes `data/sounds/base/<name>.wav`, a name no row or base has. Both are one undo step on the
  open row (numbered `Import #n data/sounds/...`, keyed by the open row). Undo and redo touch the WAV only while
  it is byte for byte what the step left, else refuse and say so. A Use keeps only its own credits row; undo sets
  or takes out that row in the file as it is then [0066].
- Credits: `data/sounds/credits.toml`, one `[[credit]]` per recording with `name` (its path under
  `data/sounds/`), `source` (URL), `licence` (`own`, `CC0`, `CC BY`, or what else was picked), `author`, `title`.
  The panel will not write a file without one of the three licences, nor without a source unless it is the
  developer's own, nor a CC BY one without author and title. `check=1` and the Studio's Problems list refuse
  every `*.wav` and `base/*.wav` under `data/sounds/` with no row, or whose licence is not one of the three, or a
  CC BY row lacking source, author or title [0066].
- A dropped file and a typed path take the same road (`import:<path>` from the command box and `cmd=`); `drive`
  has no drop step [0066].
- The Problems list replaces only the blocks error by the rows left out; any other error stays [0066].
- Open: a pending import is heard only in the panel, not in the family strip (a taste call left open). A long
  file is decoded on the Studio's frame thread (`shortcut:`); a worker when long files are common [0066].

## No longer holds
- 0031's `hud::sparks` and `App::acted`: gone, replaced by effects; the water's own droplets in its draw: now
  `spray` particles [0031].
- 0031's "break stays the burst alone at one click": replaced by 0034 (survival dig time, cracks through `Fx`).
- 0031's 0.5 m block sizes (debris 2 x 2 texels at 0.5 m) and its `baseline.json` `fx` row: replaced by the 1 m
  figures above (0033).
- 0032's "digs and places start at their own call sites until effects land": done, see "Sounds start from cell
  events".
- 0032's "Music at the title" as a later step and "not built: cave reverb": cave reverb built by 0054.
- 0048's "rows compiled in, card reads `sounds.toml`'s time only": replaced by `audio::start_rows` (0048 review).
- 0050's "sounds start from the rows compiled in": replaced by the same.
- 0051's "In place's cave is a reverb step plus a little dullness, until the game's cave reverb": the game's cave
  reverb exists (0054); the Studio audition still uses the step plus dullness.
- 0054's "Not built: water" and the 0051 "Review: not yet" lines: history only.
