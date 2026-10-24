# ADR 0044: Sound

Status: accepted (2026-10-24)

## Context
The game was silent: no players, no buses, no assets; ADR 0032 left audio settings "for when the game has
sound". The developer's calls (2026-10-24):
- **Source:** CC0 libraries, cut and levelled with our own tools (not synthesis, not paid packs).
- **Music:** sparse ambient tracks with long silences, Minecraft / Vintage Story style.
- **First pass:** player and world, sim and physics, crafting, animals and UI. Biome ambience (wind,
  birds, crickets, cave drips) comes later.
- **Style:** grounded and warm, like Vintage Story, to match the golden grade (ADR 0041). Positional 3D
  sound, caves reverberate.

## Options considered
- Sources: CC0 recordings (Kenney, BigSoundBank, OpenGameArt) vs synthesis (sfxr-style, small and fully
  data-driven but retro) vs royalty-free bundles (Sonniss GDC: realistic but not CC0 and tens of GB).
  Freesound needs an account for downloads, so it is out for now.
- Engine: Godot's own AudioStreamPlayer3D and buses vs FMOD / Wwise GDExtensions. Godot's mixer covers
  3D panning, attenuation, reverb and low-pass; a middleware adds a second build chain and a licence
  for nothing we need yet.
- Sim sound: one emitter per burning or flowing cell (thousands, impossible) vs grouping cells into a
  few loops per kind. Grouping wins: Noita does the same.

## Decision
1. **Assets.** Every sound is CC0: Kenney (impact, RPG, interface and UI audio), BigSoundBank (field
   recordings: steps, animals, fire, water), OpenGameArt (music by HoliznaCC0 and Yoiyami). The raw
   downloads stay outside the repo (`~/Tools/downloads/sounds/<pack>/`); `game/tools/make_sounds.py`
   reads `game/tools/sound_recipes.json` and writes `game/assets/sounds/**.ogg` with ffmpeg
   (`~/Tools/ffmpeg/`): it slices single hits out of long recordings by onset, trims silence, fades,
   levels every one-shot to the same loudness, crossfades loops into seamless ones and encodes
   Ogg Vorbis (mono for effects, stereo for music). `assets/sounds/CREDITS.md` names each file's source.
2. **Data.** `data/sounds.json` holds the sounds (variants, volume, pitch range, bus, range, cooldown),
   each material's sound group (steps, digging, breaking, placing), which particle bursts also make a
   sound, the sim loops, each animal's calls, and the music. A test checks that every file exists and
   every material, mob and burst resolves.
3. **Runtime.** A `Sound` autoload (`scripts/audio/game_audio.gd`, class `GameAudio`) outlives the title
   screen and the game. Buses: Master > Music, World (effects and loops, with a low-pass under water and
   reverb in enclosed spaces), UI. It keeps pools of 3D and 2D voices, steals the oldest when they are
   all busy, and skips sounds beyond their range. World units are voxels: `unit_size` and
   `max_distance` are converted from metres.
4. **Hooks, not edits.** Game code calls `Sound.play(name, pos)` only where nothing else tells the story.
   Everything else is listened to: the player's signals (breaking, placing, landing, eating, dying), a
   `burst_played` signal on ParticleFx (mob hurt and death, shearing, knapping and clay chips, splashes),
   FallingPieces' piece events (creaking, crashes), and every button the UI makes (clicks; no hover sound, the developer turned it down,
   hooked through `SceneTree.node_added`). It also watches the player for steps, swimming and mining hits,
   and the mobs for their calls.
5. **Sim loops.** At 10 Hz `SimWorld.sample_cells` picks random cells around the camera with the
   sound's own material mask (native; only hits come back). Per kind (fire, flowing water, falling sand,
   lava, steam) the hits give a loudness and a centre, and one looping 3D voice per kind follows them.
6. **Music.** The first track 1-3 minutes after a world loads, then 5-12 minutes of silence between
   tracks, day and night pools; the title screen plays its own. Never during the loading screen.
7. **Settings.** Master, music, effects and interface volume on a new Audio tab, saved in
   `user://settings.cfg`. `sound=0` / `music=0` on the command line; autopilot runs keep sound on, so they
   exercise it, but log rather than listen.
8. **Checks without ears.** `Sound.stats()` (in the autopilot JSON and F3): voices busy, plays by name,
   culled and stolen counts, loop levels, music state, cost per frame. `/sound <name>`, `/sound stats`,
   `/music next|stop`.

9. **Reviewed by ear.** The developer judged every sound on a phone page with Yes / No per sound
   (2026-10-24, three rounds: 104 sounds, then the 32 replacements, then 9 third tries) until all passed.
   Their calls: the menus make one soft click and nothing else (tabs, sliders, slots and hovers are
   silent after two rejected sets); throwing an item is silent; an animal's death is its own hurt call
   pitched down plus a body-fall thud.

## Consequences
- Adding a sound: a recipe line, `python tools/make_sounds.py`, a `sounds.json` entry; no code.
- Some stand-ins passed the ear test: the pig is a mouth imitation, the bear is a zombie growl and a
  cat growl pitched down, knapping is rubble clacks pitched up. Better recordings can replace them
  through the recipes at any time.
- The 2026-10 budget includes sound: the manager must stay under 0.3 ms a frame on the main thread.
  Measured (headless forest run with a felling, a crater, a water ball and four animals, 420 sounds in 30 s):
  0.07 ms a frame on average, 1.6 ms at worst; p99 10.4 ms against 10.2-10.4 ms with `sound=0` in the
  same run (that scenario misses the budget headless either way).
- Effect files load synchronously: interface sounds at start-up, the rest in `attach()` under the
  loading screen (0.4 s). Loading them on ResourceLoader threads beside the game's start-up broke the
  headless renderer (uninitialised RIDs, null meshes), so it is off.
- Biome ambience, weather, footsteps of other players and per-surface reverb come later.
