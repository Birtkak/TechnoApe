# P1 – Headless verification loop

**Question:** Can Claude run Godot from the command line, read its output, and
get a pass/fail exit code without the editor or a human in the loop?

**Run:** `./run.ps1` (or the equivalent `Godot_..._console.exe --headless
--path . --script res://smoke.gd`).

**Measured answer (2026-10-21):** Yes.
- `--version` prints `4.7.2.stable.mono.official.ed1daf0bf`.
- `smoke.gd` runs under `--headless`, prints a line to stdout, and `quit(0)` /
  `quit(1)` propagates as the process exit code.
- FastNoiseLite with a fixed seed is deterministic across calls.

**Still to do in P1:**
- Add gdUnit4 and run a test suite headless.
- Launch a real scene headless and capture a screenshot for visual QA.
- Verify a C# project builds and runs via the CLI (`dotnet` + Godot .NET).
