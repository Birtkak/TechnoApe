# 0038 – Three seams: the window between worlds, the ground in fast flight, the far planet's band

- **Status:** accepted
- **Date:** 2026-11-11

Three F9 reports of 2026-11-11, each a seam that breaks immersion. One decision each.

## A. Going between a world and the title shows the desktop (report 2026-11-11_12-54-55)

### Context
"going from world to menu and back it switches windows for a split second". Nothing re-applies the window,
the fullscreen state or the cursor grab when the pause menu opens or closes: `set_menu` only frees or
confines the pointer, and `apply_display` runs only when a display setting changed (`settings::RISKY`).
The switch is the process itself: the pause menu's Title row and the title's world rows call `start_again`,
which ends this process and starts a fresh one per world (ADR 0023's choice: nothing of the old world carries
over). The old window closed before the new process even started, so for the new process's start-up the
desktop showed.

### Options considered
1. **One process, one window, the world swapped inside it** – no seam at all, but the App, the local server,
   the streamer, the GPU buffers and the audio would all need a reset path: a large redesign, and every
   leftover is a bug.
2. **Hand over the window** – the old process keeps its window (its last frame stays on screen) until the
   new process has presented its first frame; the new process says so by writing a file the old one named
   (`shown=<file>`); the old one waits for it at most 5 s, then closes.
3. **Start the new window hidden and show it after its first frame** – would also hide the new window's
   blank first moment, but presenting into a hidden window is untested on our drivers.

### Decision
Option 2: the smallest change that removes the gap, and the save is still written before the new process
starts (the old local server stops first). `sys::wait_shown` holds the wait and has a test
(`waits_for_the_next_window`); the first frame's present writes the file (`frame.rs`).

### Consequences
For up to the new process's start-up (usually under a second) two processes hold the GPU. If the new
process fails, the old window stays 5 s longer, frozen. Option 1 stays the real fix if the seam still shows;
option 3 can follow if the new window's first moment flashes.
