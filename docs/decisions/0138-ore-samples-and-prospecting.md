# 0138 – Ore samples on the ground and a prospecting strike

- **Status:** proposed
- **Date:** 2026-11-16

## Context
Ore was found only by subtle signs: stream gravel, gossan, the halo of stained rock (ADR 0087, 0095). There was
nothing to do to look for it. The developer's call (2026-11-16): loose ore samples on the ground above every
body, and a prospecting strike that tells what is near and how strong. Rubble (ADR 0093), the bodies, the halo
and the exact ore ledger (ADR 0063) stay as they were.

## Options considered
1. Samples as clutter rows a biome lists, with a rule "over an ore body" – would need every biome to list every
   sample and a per-row place rule that looks up bodies; the biome roll would thin them by biome weights.
2. Samples chosen per column from the bodies under it, before the biome's roll, through the existing pick-up
   path (`clutter::pick`, `pickup_at`, the taken marks) – one lookup in the bodies' tile index; no new storage.
   Chosen.
3. A prospecting answer as a chat line from the server – the bot and the client would have to parse it back.
4. A new `ToClient::Prospect(Vec<(element, band)>)` – the client words it and pitches the knock, the bot prints
   it as JSON. Chosen; the strike reuses `ToServer::Strike` (flint in hand knaps, a prospecting tool prospects).

## Decision
**Samples.** A `[[kind]]` of `data/minerals.toml` may name `sample = "<item>"`: copper cap and deep copper the
copper nugget, tin vein a tin pebble, banded iron an iron pebble, coal seam a coal lump, lead pocket a lead
pebble, gold vein a gold nugget, uranium a pitchblende pebble (stack 16, `data/items.toml`). Each has a pick-up
row in `data/clutter.toml` (`on = []`, no biome lists it) and a model in `data/models/` (`tools/paint_clutter.py`).
`clutter::sample` rolls, per column, each body in the column's tile whose spread covers it: its footprint seen
from above, each half axis at least `SAMPLE_SPREAD_M` (a vein is two metres thin), with chance `(1 - q) /
SAMPLE_EVERY_M2` (q: 0 at the middle, 1 at the rim, squared), so densest over the middle and none at the rim,
however deep the body. `pick` asks it before the biome's roll, so a sample is a pick-up like flint: deterministic
from seed and column, never under water, taken once, drawn by the mesher, checked by the server. A sample holds
no metal: the ore ledger, `made_hash` and the ore counts do not move (`shortcut:` in the data: nuggets become
smeltable with the copper age's recipes). `items::Items::parse` refuses a sample that names no item.
`technoape-server report=1` gives each kind with a sample `"samples": n`.

**Prospecting.** A `[[material]]` has `prospect = <metres>` (flint 16, 0 to 64, checked at load). In survival
the place button with a tool of kind `pick` or `any` whose head reaches, on hard rock (`items::knaps`: rock, ore,
the altered rocks), is a prospecting strike: at most one a second a player, a use of the tool. The server answers
that player only (`ore::prospect`): per element with ore left in the ground (the ledger's count, when open) on
the struck face, the distance d from the cell to its nearest body's ellipsoid surface (exact, `ore::to_surface`;
0 inside) gives a band against the range R: rich under R/8, strong under R/3, some under 2R/3, traces under R;
the three strongest, no direction. The client shows "copper: strong, tin: traces" or "the rock rings empty" in
the chat, chips the face at once and plays the knock on the answer, higher the stronger the best band. The bot's
`prospect dx dy dz` prints `{"bot":..,"prospect":[["copper","strong"]]}`.

| Number | Value | Where |
| --- | --- | --- |
| Samples a square metre at a body's middle | 1 / 40 | `ore::SAMPLE_EVERY_M2` |
| Least half width of a body's sample spread | 6 m | `ore::SAMPLE_SPREAD_M` |
| Which kinds have samples, and of what | see above | `data/minerals.toml` `sample` |
| A flint head's prospecting range | 16 m | `data/items.toml` `[[material]] prospect` |
| Longest range a material may have | 64 m | `items::PROSPECT_MAX` |
| Seconds between answered strikes | 1 | `items::PROSPECT_WAIT_S` |
| Band edges | R/8, R/3, 2R/3, R | `ore::band` |
| Elements an answer names | 3 | `ore::PROSPECT_NAMES` |
| The knock's pitch | 0.85 empty, 1 + 0.12 (band + 1) | `crates/client/src/frame.rs` |

Numbers: `report=1 seed=1337 planet=6144` (made_hash 2a79455bb6161781) lays 330 copper nuggets over the copper
caps, 146 over the deep copper, 75 tin, 4707 iron, 10203 coal, 51 lead, 26 gold and 4 pitchblende pebbles. The
banded iron and coal bodies are a hundred metres and more across, so their fields of pebbles are large; the
uranium is three small lumps. Cost: `sample` is about 17 us a level-0 chunk on the 1.8 % of chunks that lie
over a body (16 ns a column, seed 1337, 4 km planet; `may` is 1.6 us), against milliseconds for the chunk's
mesh; the server's tick does nothing more until a strike. No row in `docs/perf-debt.md`.

## Consequences
- Ore can be looked for: samples lead to a body's footprint, strikes in a few places home in on it.
- The ore tile index registers a body at least `SAMPLE_SPREAD_M` wide; generation is unchanged (the extra
  entries' spans are empty).
- A pick-up taken at a column before this (a flint) now hides the sample that lies there, if one does; rare.
- `ToClient` has a new variant: client and server must be of one build, as `Welcome`'s hashes already require.
- shortcut: a strike hears only the struck face's bodies (bodies keep 6 columns from a seam); a body across the
  seam is not heard.
- Later: samples into the copper age's recipes; better heads (copper, bronze) with longer `prospect`; a
  directional tool (a dowsing kit) is a separate call.
