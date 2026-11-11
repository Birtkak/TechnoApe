# 0019 – Input steps and reconciliation

- **Status:** accepted
- **Date:** 2026-11-10

## Context
ADR 0007 left a shortcut. The client sent its latest `Input` at 20 Hz. The server stepped that input on its
own clock. The client snapped to the server's position when the two disagreed by over a metre. The feel
report (ADR 0018) caught what that costs: in some runs, a 1.1 m snap about 0.3 s after loading. The client
walked before the server had its first input. Then it compared where it is now with where the server was a
few ticks ago. With real latency on the dedicated server this would rubber-band all the time.
`fundamentals.md` section 1 asks for 0 corrections in a clean local run, and eased corrections.

## Options considered
1. **Latest input, server clock, a snap threshold** (as before). It is simple. But the two sides step
   different counts of different inputs, so they never agree exactly, and the threshold hides the gap until
   it is large.
2. **Numbered steps, each taken once (the Quake/Overwatch model).** The client sends every physics step's
   input with its number. The server takes each exactly once, in order. It answers with its whole physics
   state after the last step it took, and the client replays its later steps from there. Same code, same
   inputs, same order: the paths are identical to the bit. Any difference is a real disagreement, such as a
   world edit the client predicted wrongly or inputs the server dropped.
3. **The server's clock with a time stamp per input.** This is closer to option 1, and clock-offset
   estimation makes it more complex than option 2.

## Decision
Option 2.
- **Wire:** `ToServer::Steps { first, inputs }` carries the inputs not sent yet, one per 1/60 s step, about
  three per 20 Hz message. Every tick, every client gets `ToClient::You { step, me }`: the number of the
  last step taken (0 for none yet) and the whole `phys::Player` (position, velocity, look, ground contact).
  `Input` (one input held every tick) stays for bots and the soak, which do not predict.
- **The server** queues new steps in order and drops repeats and old ones. It takes them at most as fast as
  real time: three a tick, plus up to 12 saved up when a batch comes late. A client more than one second
  (60 steps) ahead loses the rest. That keeps the queue short and makes speeding impossible. A `Move`
  clears the queue.
- **The client** does not move before the first `You`, which places it (a saved player starts where it
  left). On each `You` it drops the steps the server has taken and replays the rest from the server's
  state. If that lands over 1 mm from the prediction, it counts a correction (`feel.corrections`). The drawn
  position keeps the old place as an offset that eases out with a 100 ms time constant. A jump of 2 m or
  more (a teleport) is not eased.
- **The look is the player's own (2026-11-10).** A step's input carries `turn`, the mouse's turn about the
  player's up since the last step, and the pitch; no direction. `phys::step` carries `Player::fwd` to the
  step's up (gravity keeps it level), turns it, and `reframe` rotates it with a frame change. A direction
  in the input meant another one once a step changed frames: a client's frame took several steps with one
  input, so after the planet's sphere of influence the later steps looked the old frame's way and a flight
  at 1000 m/s fell back in and out of it every few ticks (bug 2026-11-10_19-46-22). A non-zero `fwd` still
  sets the look outright, for bots and the soak.

## Consequences
- A clean local run has 0 corrections and `drift_max_m` exactly 0.0, and the gate now asks for 0.
- A mutation (the server drops every 97th step) gives one correction of 8-15 cm every 1.6 s, eased. A
  correction is shown, not hidden.
- Upload is three inputs per message instead of one, still under 100 bytes. Every client gets one `You`
  per tick.
- Latency now delays only when the server sees a player move, not how the player's own movement feels.
  Other players are still drawn at their last received position. Interpolating them comes with the
  dedicated server.
- The server trusts the input values (any look, any move in -1..1), not positions, so a modified client
  can only walk as a player could.
- **Fixed 2026-11-11 (db30c50):** the client timed its step messages from when the world had loaded, but the
  player steps from its spawn before that. The first message then held about 98 steps, the server's queue
  (60) dropped the rest, and the gate's played run at detail 8 showed one 3-6 m correction at t = 0.9 s. The
  sending now runs by the wall clock. The bug predated ADR 0025's optimisation pass; detail 8 only made the
  load long enough to show it.
- **Open: a placement flake.** About 1 run in 4, also before ADR 0025, the played run below shows
  corrections of 0.1-0.7 m at t = 5.6 or 8.1 s, right after a placement, and the click count sometimes drops
  from 6 to 3-5. A likely cause: the client applies its own placement only when the server's echo comes
  back (against ADR 0008 and 0015), so for a few ticks its replayed steps meet a different world than the
  server's. Repro (Git Bash, `MSYS_NO_PATHCONV=1`, run it four times):
  `target/release/technoape.exe run=play save=0 seconds=10 autowalk=1 acts="0.5:m:0:250,1:say:/playmode creative,2:L,2.1:l,3:R,3.1:r,4:L,4.1:l,5:h,5.2:R,5.3:m:-100:0,5.6:r,7:L,7.1:l,8:R,8.1:r" settings=tests/gate-settings.txt`.
  The fix starts with a test that fails first, for example a bot that places a block under its own feet and
  steps in the same tick, expecting 0 corrections.
