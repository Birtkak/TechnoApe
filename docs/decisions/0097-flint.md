# 0097 – Flint: on the ground by water and limestone, and inside gravel

- **Status:** accepted (built)
- **Date:** 2026-11-13

## Context
The first tools need flint (the developer: "Pick ups on the ground or inside a gravel block (u get gravel and
flint) when broken"). Flint already was an item (`data/items.toml`) and a clutter row (ADR 0082) that lay on
any of its ground blocks wherever a biome listed it. What was missing: flint where a knapper would look for it,
some near every spawn, and flint out of dug gravel.

## Options considered
- **Flint as its own block row** (a "flint gravel" block mixed into the ground). A second gravel to paint and
  mesh, and a dig of it still has to give two things; rejected.
- **A random roll on the dig** (the server's RNG). Not reproducible from the seed; a replay would differ, and
  a block put back and dug again would roll again (a flint farm). Rejected.
- Chosen: **a data rule on the block row, rolled from the seed and the cell, only for cells still as
  generated.** Reusable for any loose block that holds something.

## Decision
- **The item:** flint, `stack = 16` (a small stack for a tool stone, as ADR 0093's small stacks), in the
  creative catalogue and `/give flint` as every item. Its icon is a cube of `slate` (items have no pictures yet).
- **On the ground** (`clutter.rs`, `data/clutter.toml`): a clutter row may carry a place rule.
  - `water = n` (1 to 8): the column, or one within n columns on its face, has water standing over its ground
    (`Body::water_at` above the first air; `water_at` alone names the sea for every dry column). Flint's is 8:
    rivers' banks and their gravel bars, lake shores, beaches.
  - `over = [blocks]`: else the rock under the column (`ROCKS` at 6 m down) is one of them. Flint's: limestone
    and marble (chalk is the flint rock; we have no chalk, limestone stands for it).
  - A row with neither lies wherever its biome rolls it, as before.
  - Flint's `on` widened to the grasses, dirt, mud, sand, stone and gravel, so the rule, not the top block,
    decides. Weight 1 per 100 columns in the land biomes (alpine too), 1.5 on beaches.
  - Measured (`flint_lies_within_a_short_walk_of_every_spawn`, 1536-column planet, seeds 5, 11 and 23): 13, 108
    and 38 flints within 60 m of the spawn (seed 11's spawn is over limestone; seed 5's nearest water is 39 m
    off); the test requires 5. Of the spawn face's dry land, 48 %, 27 % and 30 % may hold flint; the test requires
    under half. Both seen red first (seed 11 had no flint; then every dry column passed, the review's finding).
  - **Never regrows:** clutter is a pure function of the seed and the column; a taken pick-up is stored
    (ADR 0082). Taken by looking and clicking, the existing pick-up path.
- **In gravel** (`blocks.rs`, `edit::held`, the server's `edit`): a block row may say
  `holds = ["flint", 0.25]` (an item and a chance, 0 < chance <= 1, only on a `loose` block; refused
  otherwise at load). A survival dig of such a cell drops the block's pile and, if the roll says so, the item.
  - **Deterministic:** the roll is `mix` of the body's baked seed, the body, face and cell, so the server, a
    replay and a test agree.
  - **No farm:** the cell must have no edit (`World::edit(b)` is None) and its generated block must be the dug
    block. `World::set_block` records an edit for every placed block, even one equal to the generated block,
    so gravel put back holds nothing; neither does generated gravel that fell (it lands as an edit).
  - Test `generated_gravel_holds_flint_and_gravel_put_back_none` (server, baked 1536 planet): of up to 400
    generated gravel cells 15-35 % hold flint, a flint cell's dig gives one flint, gravel set back and dug
    again gives none.
- **Ledgers:** the flint goes through the same `drops` as the dug pile (`drop_dug`), so `item_made` counts it;
  the server test checks the item ledger stays exact.
- **Bots:** a bot plays creative until its script says `/playmode survival`; then its `dig` is held for the
  block's dig time (x 1.1 + 0.1 s), as a player's, since the server checks the hold (ADR 0034). Test
  `a_survival_bot_digs_flint_out_of_gravel`: the bot digs a flint cell, walks onto the pile, and `slots` shows
  gravel 1 and flint 1. On a dedicated server (seed 1337, 4 km planet) a second bot walking onto a dug bed-gravel
  cell reported `[[0,"gravel",1],[1,"flint",1]]`.
- **Cost:** not measured. The place rule runs only on a column whose clutter roll already chose flint (about 1
  in 100 columns), at most 289 water lookups there, stopping at the first wet one; the gravel roll is one hash per dug cell. Neither adds work to a
  tick or to generation worth a perf-debt row.

## Calls for the developer
- **The chance in gravel:** 1 in 4 (`holds` on the gravel row).
- **How common on the ground:** 1 per 100 allowed columns (beach 1.5) gives 13 to 108 within 60 m of a spawn;
  near water 8 columns. Lower it if a limestone spawn feels littered.
- **How visible:** the ground flint is the clutter row's placeholder model, a small grey stone that is easy to
  miss among the pebbles. Not a choice: it waits for a painting or model that makes it read as flint (dark,
  glassy, a white cortex).
- **Stack size:** 16.
- **Which rocks count as "chalk-like":** limestone and marble.

## Stills
`out/flint/flint-river-bank.png` (a flint on the bank by the spawn's river; main's build at the same view has
none) and `out/flint/flint-from-gravel.png` (the dug bed-gravel cell; the pile is too small to read from the
stand point, the bot's `slots` line above is the evidence of the drop). Both builds draw a magenta sky in
`run=stand` shots: a main bug, not this branch's.

## Review
A fresh reviewer read `git diff main...claude/flint` (engine, server, bot):
- **High, fixed:** the water test passed on every column of a planet with a sea (`water_at` names the sea for
  dry land), so the place rule did nothing. Now water must stand over the ground; a test checks that under half
  the dry land passes (seen failing at 6128 of 6128). The weights and reach were then set again (above).
- **Low, fixed:** a bot's `/playmode` with a word the server refuses flipped it to creative; it now follows only
  the server's words.
- **Low, answered:** unbaked (noise-land) worlds roll gravel with seed 0, one pattern for all; they are tests'
  and old saves', and the roll is still deterministic.
- Checked fine: the hash's precedence, no farm by putting back or falling, the ledger, the replay, no panics,
  every caller of `pick`.

## Consequences
- Any loose block can hold something by one data line (clay holding a shard, sand holding a shell).
- A row's place rule is reusable for other clutter (shells by water, fossils over limestone).
- Flint's clutter look is still the placeholder; a painting replaces it when there is one.
