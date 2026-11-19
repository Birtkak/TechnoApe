# 0006 – Client and server split: one server crate, channels as the wire

- **Status:** accepted
- **Date:** 2026-11-10

## Context
ADR 0002 wants multiplayer from day one with a server-authoritative world, singleplayer as a local server,
bots as ordinary clients, and only changes going over the wire. The engine base (roadmap step 1) had the
client own and edit its `World` directly. This is the first step of the first playable: split them before
movement, digging and saving are built on the wrong side.

## Options considered
1. **One process, the server on its own thread, connections as channel pairs; TCP for remote players.**
   The same `Server` code runs inside the singleplayer client and as the dedicated binary; the client never
   knows which. Simple, no sockets in singleplayer, trivially testable.
2. **Always a separate server process, also in singleplayer.** Clean, but a second process to start, find and
   kill on every run and test, for no gain until "open my world to friends" exists.
3. **QUIC or a game networking crate (quinn, renet) from the start.** Unreliable channels for movement and
   built-in encryption. Tailscale already encrypts; the gain is latency under packet loss, which no one has
   measured yet.

## Decision
Option 1. `crates/server` holds `Server`: it owns the `World`, ticks at 20 Hz, applies the edits players send
and broadcasts the changed blocks and the player list each tick. New connections arrive on a channel as a
`Link` (a sender and receiver pair, `engine::net`); the client makes one in memory for the local server or
from a TCP stream for `connect=host[:port]`. The dedicated binary `technoape-server` accepts TCP and hands
each stream to the same channel. Messages are `serde` enums in `postcard` frames with a u32 length;
`ToServer`: Hello, Move, Edit; `ToClient`: Welcome (seed and planet sizes, the client rebuilds the terrain
from them), Edits, Players.

Trust boundary: the server drops an edit unless the block exists, the material is known, and it is within
`REACH` (16 m) of the player's reported position. Identity is a name; nothing else, the server sits on a
private Tailscale network (ADR 0005).

Recorded as shortcuts: positions are client-reported until gravity and collision move to the server; TCP
carries everything, so a lost packet delays the stream (switch to QUIC when playtests show it); a joining
player receives every edit in one message (chunked sync when worlds grow).

## Consequences
- Every client action goes through the protocol, so a bot is a protocol client without a window (next step).
- The client's `edit` benchmark now measures the real round trip: 2,000 edits/s through the local server at
  p99 1.1 ms on the 531 m planet.
- Two windowed clients on one PC throttle each other through the Windows compositor (the first drops to
  60 fps); that is a measurement artefact, not the netcode. Load tests use windowless bots.
- The `World` is built twice in singleplayer (server and client), which costs nothing today; the client's
  copy is a cache of the server's and may diverge only until the next delta.
