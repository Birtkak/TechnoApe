# 0032 – Sound: rows of data and our own mixer

- **Status:** accepted
- **Date:** 2026-11-11

## Context
The game was silent. `docs/design/sound.md` has the rules: sound is presentation, a sound is a row of
data, the frame thread never waits for it, and it must be checkable without ears (Claude cannot hear).
The developer decided the sounds come "both from the start": made in code, and from recordings dropped in.

## Options considered
1. **`cpal` (the sound card only) and our own mixer** – about 150 lines of mixing we own; the same
   function runs on the card's thread and in the tests, so loudness, place, the voice cap and silence at
   the end are tested by number. Reverb and a measured head are ours to write later.
2. **`kira`** – tweens, places, reverb and filters ready-made; but its mixer runs only on its own thread,
   so checks without ears would need a second path, and code-made sounds would still be ours.
3. **`rodio`** – simple playback and decoders; its places are two ears by distance only, and the same
   testing problem.

Not spiked against each other by measurement: the choice turns on testability, which only option 1 has.

## Decision
- **Option 1.** `crates/client/src/audio.rs`: `data/sounds.toml` rows, `render` (layers of filtered noise
  and tones with a fade), recordings by name from `data/sounds/` (`hound`, WAV only), `Mixer::mix`, and
  `Audio`, the game's side, which works out what the head hears and sends the mixer a message.
- Every take is made when the card opens, on the card's own thread. The frame thread sends messages and
  nothing else.
- A block names its material's sound (`sound` in `data/blocks.toml`, stone when it names none); a test
  holds every block to having dig, place and step rows.
- Off-screen runs open no card (`sound=1` does) and only count what would have played, so no benchmark or
  view moves.

## Measured (dev PC, release build, 2026-11-11, while two other sessions were compiling)
| What | Number |
|---|---|
| The mixer's longest call, play with digging and walking | 63 µs per 10 ms buffer (under 1 % of one core) |
| Voices at once in that run | 2 (cap 32) |
| Delay the card reports, at worst | 44 ms (wireless headset, 44.1 kHz, the system's shared mixer) |
| Frame thread | a message per sound; no measurable time |
| A scripted play run (dig, dig, place, walk 3 s, hotbar) | `dig.grass` 1, `dig.dirt` 1, `place.grass` 1, `step.grass` 4, `ui.tick` 1 |

## Consequences
- **The 20 ms of `fundamentals.md` is not met** on this card: 44 ms at worst. The message waits at most one
  10 ms buffer; the rest is the system's shared mixer and the headset. Ways down, when it is felt: the
  system's low-delay shared mode (cpal does not ask for it), or exclusive mode as a setting.
- A new block, machine or creature gets sound by naming rows; a recording replaces a row without code.
- Not built yet (sound.md, "Later"): cave reverb and walls, water, machines, weather, music, voice,
  subtitles, the output device and speakers settings.
- shortcuts in the code, each marked: a sound's place is fixed against the head when it starts (fine under
  a second; moving sources need an update message); WAV only; a card that does not take floats stays silent.
- Until the effects system (ADR 0031) lands, sounds start at their own few call sites (the click, the
  server's edits, the physics step, the menu); they move onto its events when the two branches meet.
- **The review found, and the next commit fixed:** footsteps while pressed against a wall (the distance
  was the wish's, not the way gone); a block read per incoming edit cell on the frame thread, needed for
  three at most; far edits using up a batch's three sounds; wind in vacuum under "softened" and "full" and
  none at the title; a lost card left messages queueing for ever (its thread now ends and the card is
  tried again every two seconds); a second open closed itself; a recording with no rate jammed a voice;
  the checks let through a noise filter that is not stable. Still without a test: the call sites (steps,
  landings, other players' edits), checked by a scripted run's counts only.
- One build rule: `sound =` in `data/blocks.toml` changes the blocks' hash, so a client of this build
  refuses a server of an older one until the server is rebuilt.
