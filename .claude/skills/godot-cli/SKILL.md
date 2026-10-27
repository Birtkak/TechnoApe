---
name: godot-cli
description: How to run Godot 4.7.2 headless for tests, benchmarks, C# builds and screenshots in this project. Use whenever running or verifying anything in a Godot project here.
---

# Godot CLI for TechnoApe

Binary (console build prints to stdout; use it for everything scripted):

```
G="$HOME/Tools/Godot/4.7.2-mono/Godot_v4.7.2-stable_mono_win64/Godot_v4.7.2-stable_mono_win64_console.exe"
```

Always run from the project directory (`--path .`). First run of a project,
or after adding addons/resources: `"$G" --headless --path . --import`
(may segfault once when the voxel extension is present; re-run).

| Task | Command |
|---|---|
| Main-loop script, headless | `"$G" --headless --path . --script res://file.gd` (script extends SceneTree; `quit(code)` sets exit code) |
| gdUnit4 tests | `"$G" --headless --path . -s res://addons/gdUnit4/bin/GdUnitCmdTool.gd --add res://test --ignoreHeadlessMode` (exit code = result; HTML/XML in `reports/`) |
| C# | `dotnet build <project>.csproj -c Debug` then run a scene with the C# node: `"$G" --headless --path . res://scene.tscn` |
| Rendered run + screenshot | drop `--headless`; add `--resolution 960x540 --position 1200,100`; script saves PNG via `root.get_viewport().get_texture().get_image().save_png()` after ≥3 frames |
| Pass args to script | after `--`, read with `OS.get_cmdline_user_args()` |
| Timings | `Time.get_ticks_usec()`; disable vsync for frame stats: `DisplayServer.window_set_vsync_mode(DisplayServer.VSYNC_DISABLED)` |

Rules:
- Wrap runs in `timeout <s>` so a hang never blocks the session.
- Wait for `WorkerThreadPool` tasks before `quit()` or you get null-lock errors.
- gdUnit4 addon: delete `addons/gdUnit4/test/` or C# builds fail.
- Typed GDScript: `var x := call()` fails to infer on Variant returns; annotate.
- Godot front faces are clockwise; CCW corner tables need index order 0,2,1 / 0,3,2.
- Kill strays: `taskkill /F /IM Godot_v4.7.2-stable_mono_win64_console.exe`.
- **Every run gets a hard cap**: prefix with `timeout <s>` (Git Bash) sized to the expected length plus a little (sheet tools 60 s, a 20 s autopilot 90 s, the full test suite 600 s), then `taskkill` the strays in the same command. A tool script that waits for frames also quits itself on a deadline (see `tools/station_sheet.gd` DEADLINE_MS). Never wait on an open-ended Godot process.
