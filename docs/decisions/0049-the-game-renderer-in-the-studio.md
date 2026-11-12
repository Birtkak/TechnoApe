# 0049 – The game's renderer in the Studio

- **Status:** accepted
- **Date:** 2026-11-11

## Context
The Studio (ADR 0044, 0046) shows blocks and models as flat 2D pictures. A block's texture or a model can look right
flat and still look wrong under the sun, beside a lamp or among its neighbours. The Studio should show them with
the game's own renderer: a lit 3D view with an orbit camera and a time-of-day slider, used by the Textures page
(a lit cube among neighbours) and by the new Models page (a model at a pose).

Constraints:
- The client must not change its behaviour: `tools/views.sh` and the client's tests stay green.
- The Studio's interface must never wait on the renderer starting.
- `shot=` must keep working, so Claude can check the page without eyes.

## Options considered
1. **In process, on the Studio's device.** egui-wgpu hands out its device, and a paint callback draws the
   game's passes into the panel. This has the least delay and the fewest copies. But the client's `Gpu` makes
   its own device from its own window's surface (`gpu.rs`). It is driven by winit's event loop (`lib.rs`) and
   runs its streamer, mesher and simulation threads under its own per-frame budget. Taking all of that into
   eframe's loop would rework the client deeply, on the code every game frame runs, which is the opposite of
   "widen only what is needed". A renderer panic would also take the Studio and its unsaved work down with it.
2. **In process, on its own device.** This brings the same rework of the window and loop, and only saves
   option 3's process start and pipe copy.
3. **A child process.** The release game runs as `run=view`, an off-screen stand run that never ends on its own.
   It reads one command per line on stdin, and after each `frame id` it writes the drawn frame's RGBA to stdout.
   The Studio copies the frame into an egui texture. The client gains a mode, not a new shape.

## Decision
Option 3.

**The client.** A new module, `crates/client/src/view.rs`.
- Commands: `cam ex ey ez tx ty tz` (in the stand cell's frame, in a model's metres), `time s`,
  `model name [clip] t` / `model -`, `models dir`, `textures dir`, `place di:dj:dl:block,...` (`_` is air),
  `reset`, `size w h`, `frame id`.
- The reply to `frame` is a line `frame id w h fov` followed by w*h*4 bytes.
- Files are read on the stdin thread. The frame loop sleeps to at most 60 frames a second.
- Hooks in `frame.rs`, `app.rs`, `gpu.rs` and `lib.rs` apply only when `run == "view"`.
- Nothing in the client was made public. `models::Models` gained `feed()` and `show()`, and `cell_axes` was
  split out of `cell_frame` with the same result. `engine::models::sample` became `pub`.

**The Studio.** `ui::viewport::Viewport`. It is reusable: a page holds one (the app holds the shared one) and
calls:
- `ui(ui, height, fallback)` draws the view and handles the orbit input: drag turns, wheel zooms, middle drag
  pans. It draws the 2D fallback until the renderer is up, or for good if it fails.
- `controls(ui)` shows the scene (grass, or a dark cave beside a lamp), the time slider and the renderer's state.
- `show_model`, `models(gen)` and `textures(dir, gen)` choose what is shown and where its files are.
- `place` sets the blocks placed around the view.
- `ray(pos)` and `to_screen(p)` turn the view's pixels into rays and points for picking and overlays.
- `settle(ctx, max)` (for `shot=`) and `report()` (JSON).

**Finding and starting the renderer.**
- The executable is the first that exists of: `renderer=`, `target/release/technoape.exe`, the Studio's sibling
  `technoape.exe`, and `build/TechnoApe.exe`.
- It starts on a thread with `CREATE_NO_WINDOW`, `save=0 record=0 sound=0`, and its own `settings.txt` under
  `out/studio-view/`.

**Measured** on the dev PC (RTX 3070) in `shot=` runs:

| | |
|---|---|
| start to first frame | 2.9–3.3 s (the 2D preview shows meanwhile) |
| delay from asking for a frame to having it, p50 | 15.6–21.7 ms (mostly the 60 fps cap's sleep) |
| frame size | follows the panel, clamped 640–1920 x 360–1200 (774x588 for the Models view at 1280x800) |

The delay is one or two egui frames: orbiting feels attached, not instant.

## Consequences
- Every page can show the real look: the Textures inspector's lit cube and the Models page. A renderer crash
  leaves the Studio running on its 2D previews.
- The renderer runs, at up to 60 fps, as long as the Studio is open with a viewport alive.
  `// shortcut:` render only when asked, if the Studio's GPU use shows up while the game runs beside it.
- The view needs a built game (`cargo build --release` or `build/TechnoApe.exe`). Without one it stays 2D.
- A frame goes through a pipe and a texture upload (about 1.8 MB at 774x588). The overlay (grid, pivots,
  selection) is drawn by egui and can lag the 3D image by a frame while the camera moves.
- Not built: a Luna regolith scene; watching model files for changes made outside the Studio; naming points
  (they get automatic names and can be dragged). A palette change shows in the view only once the model is next
  written. If the Textures page has not yet written its folder, the world shows the code-made patterns.
- If option 1 is needed later (for less delay, or to drop the copy), the protocol shows exactly what the client
  must offer: a camera, a clock, model and texture folders, placed cells, and a frame.

## Review
A fresh reviewer read `view.rs` (2026-11-12) and fed the view malformed lines. Found: `place` with a far offset
(`place 2000000000:0:0:stone`) panicked in the bake's lookups and ended the process. Fixed: an offset is
refused (named on stderr) beyond 64 cells from the stand cell (`place_spec`, test
`a_place_is_a_cell_near_the_stand_cell_and_a_block`). And `size 100000 100000` gave a 65535 pixel window whose
surface wgpu refused with a panic; a size is now held to the device's largest texture (`view_size`, test
`a_size_is_one_the_surface_takes`). The other malformed lines tried (`cam`, `model` without a name, `time nan`,
`cam nan ...`, `frame -5`, unknown effects and states) were refused or harmless.

## The live view driven (2026-11-12)
`models_live_view_a_click_adds_on_the_face_seen_and_a_drag_turns_it` (ignored: it needs
`target/release/technoape.exe`; `cargo test -p studio --test drive -- --ignored models_live_view`) finds a
single copper voxel in the game's frame, clicks it with Add and checks the voxel lands on it; a right drag turns
the orbit and the frame. The pick was right. A shot now settles the view after its input, and its `view`
report carries the orbit. The Palette page gained undo (ADR 0046's page writes at once, so a step holds the
file's old text and undo writes it back and repaints the textures).
