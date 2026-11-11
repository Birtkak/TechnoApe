# Sound

- **Status:** design, 2026-11-11 (the developer: "let's get started on sound"; sounds come "both from the
  start": made in code and from recordings). The first cut below is built (`crates/client/src/audio.rs`,
  `data/sounds.toml`); ADR 0032 records the library and the measured cost.
- What it must meet: `vision.md` ("rich 3D sound"), `fundamentals.md` (a sound starts within 20 ms of its
  action), ADR 0002 (cave reverb, material footsteps and digging, wind and water from the first playable),
  `settings.md` (the Sound page), `effects.md` (a sound starts from the same events as an effect).

## The rules
1. **Sound is presentation.** Client only, never in the server's tick, nothing in the simulation depends
   on it (principle 7).
2. **The world says what happened; the client decides how it sounds.** The same events that start an
   effect start a sound: a cell changed (so another player's digging is heard), the player's own predicted
   action the frame of the click, a step, a landing, a menu's click.
3. **A sound is a row of data** (`data/sounds.toml`, schema and test, principle 6). A row is made in code
   (layers of noise and tones, each with a filter, a pitch sweep and a fade) **or** it is a recording:
   a file `data/sounds/<row's name>.wav` (or `<name>.1.wav`, `<name>.2.wav`... for variants) replaces the
   code-made sound of that name. The developer drops a file in where a sound feels cheap; nothing else
   changes.
4. **A block names its material's sound** (`sound = "stone"` in `data/blocks.toml`; stone when it names
   none). Digging stone plays `dig.stone`, walking on it `step.stone`. A new block gets sound for free.
5. **Never the frame thread, never a stall** (principle 8). Every row is rendered to samples when the game
   loads; the sound card's own thread mixes them. The game only sends small messages (play this here, the
   listener is there). A hard cap of 32 voices, the quietest dropped first.
6. **Checkable without ears** (principle 10). The mixer is one plain function: the sound card calls it,
   and so do the tests, which measure what comes out (length, loudness, where its pitch sits, no clipping,
   silence at the end). `sounds=1` prints those numbers for every row as JSON, and a play run's JSON counts
   every sound it played by name. In the game `/sound <name>` plays a row, for the developer's ears.
7. **Off where numbers must repeat:** off-screen runs open no sound card unless `sound=1`.

## How it sounds in the world
- **Where:** each voice has a place. Loudness falls with distance (a row's own reach); left and right by
  where it is around the head, with the small delay between the ears and a duller tone behind
  (headphones). *Later:* a measured head (HRTF), surround.
- **Air carries it:** the world's sounds are scaled by the air at the listener (the value the sky already
  uses), so in space only what you touch and your suit are heard, dull, as `settings.md` decides ("Sound in
  space": true, softened, full).
- **Under water** everything is dull.
- **Caves and rooms** (*second cut*): a few rays from the listener measure how closed in the place is and
  how big; that sets one shared reverb. Behind a wall a sound is duller and quieter (one ray per voice).

## First cut (built, ADR 0032)
- The system: rows, code-made sounds, recordings, the mixer, places, the cap, the checks of rule 6.
- Sounds: digging and placing by material (also other players'), footsteps and landing by the block
  underfoot, the menu's hover and click, wind that grows with height and speed, the hotbar's tick.
- Settings, page Sound: master, world, footsteps, interface (0 to 100 %), mute in the background, sound in
  space.
- The play run's JSON gains `audio`: the device, its delay in ms, the most voices at once, the mixer's
  longest call.

## Later, on the same system
Cave reverb and walls (second cut); water (shore, pouring, swimming); machines' hums as loops that follow
their state; weather; creatures; music (needs a compressed format: its own choice then); voice chat with
the dedicated server; subtitles for sounds with their direction.

## Open
- The exact step length and loudness of each sound: tuned by the developer's ears in the first playtest.
- The delay: 44 ms at worst on the developer's headset through the system's shared mixer, over the 20 ms
  `fundamentals.md` asks for (ADR 0032 has the ways down).
