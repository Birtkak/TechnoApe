# 0013 – The block registry: one TOML data file, built into the binary

- **Status:** accepted
- **Date:** 2026-11-10

## Context
Digging and placing (ADR 0008) needs blocks to be content, not code: working principle 6 asks for data files
with a schema and tests. Until now a block was a bare number. `MATERIALS = 4` was the server's check,
`material()` returned literals, physics took "not 0" to mean solid, and the shader had its own palette. Those
four had to agree by hand.

## Options considered
- **Format:** TOML via the `toml` crate (serde, the stack's serialisation already), RON, or JSON. TOML reads
  and edits best by hand and gives line-numbered errors. RON adds nothing a flat list needs. JSON has no
  comments.
- **Loading:** read at start-up from `data/`, or built into the binary with `include_str!`. Built in means
  one file to ship and a client and server from one build cannot disagree. Modding and hot reload can come
  later, behind the same `Registry::parse`.

## Decision
`data/blocks.toml` lists `[[block]]` entries:
- `id`: u8, which is what the wire, the edits map and the mesher carry.
- `name`
- `solid` (default true)
- `colour`
- `pattern`: plain, grass or stone.
- `side`: the block a grass-pattern block shows on its sides.

`engine::blocks::BLOCKS` parses the file once. A broken file panics at start-up. Unknown fields, duplicate ids
or names, a missing `side`, colours outside 0..1, and an id 0 that is not a non-solid `air` are all refused,
each with a test.

The registry feeds every place a block number meant something:
- Generation looks up grass, dirt and stone by name.
- `phys::blocked` asks `solid`.
- The server accepts any registered id.
- The soak picks from `placeable()`.
- The client uploads every block's look to a 256-entry storage buffer (binding 3), and the shader reads its
  colour and pattern there.
- `Welcome` carries the file's FNV hash, and the client and bot refuse a server whose blocks differ.

## Consequences
- A new block is one TOML entry, as long as it uses an existing pattern. A new pattern is shader code.
- Ids are stable numbers in saves and recordings, so an id is never reused for another block.
- The seed-1 soak keeps its final hash (abbd5a34a557dec0): the ids did not change.
- Stone cracks now show. The old shader keyed them on material 0 (air), so they never drew.
- The mesher still draws every non-air block as an opaque cube. Glass, water or plants need a `transparent`
  field and a mesher change when they arrive.
