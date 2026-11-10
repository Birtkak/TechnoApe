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
