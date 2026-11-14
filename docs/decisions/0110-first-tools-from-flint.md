# 0110 – The first tools: flint knapped into a hand axe

- **Status:** accepted (built; waits for the developer's play-test and merge)
- **Date:** 2026-11-14

## Context
`docs/design/items.md` "Tools" designs tools as data: an item with a kind and a head material whose row gives
speed, grade and uses; blocks name their tool and grade beside `hardness` and `loose`. `crafting.md`'s starter
puts a knapped hand axe in minutes 10-20. Flint lies on the ground and in gravel (ADR 0097). Until now digging had
one speed, bare hands (ADR 0034), rock left its rubble (ADR 0093, 0094) and every other block that is not loose
broke into nothing. The developer chose **knapping by doing**: strike flint on rock, no menu. Constraints: server
authority, today's bare-hand behaviour kept exactly (the tool only adds), creative unchanged, data with tests.

## Options considered
1. **Where the server learns what is in hand.** (a) The edit and the dig hold carry a slot: two messages change
   shape and every sender with them. (b) The server picks the best tool in the hotbar: not what the player
   holds. (c) **Chosen:** a new `ToServer::Hand(slot)`, sent on join and on each change of slot; the server keeps
   it per player (slot 0 until told). Two variants added at the end of `ToServer`, so recordings still read.
2. **Where the uses left live.** The stack's 8-byte payload was there for this (items.md "What an item is").
   Chosen: the payload counts the digs **had** (0 is new), so every `Stack::new` (`/give`, creative's grid, a
   knap) is a fresh tool with no other change, and the save and the wire, which already carry the payload, keep
   the wear with **no format change** (no save version bump; the items hash in `Welcome` changes with the file, so
   an older client is refused as for any data change).
3. **What a tool does to rock.** Rock with rubble keeps leaving its rubble (mining.md 4: broken rock is rubble);
   the pick only makes it faster. The grade decides the drop of a block that is not loose and has no rubble
   (dolomite, copper, which hands break into nothing). Against: rock dropping as a block to a pick, which would
   undo ADR 0093.
4. **The first tool.** (a) Hafted flint axe, pick and shovel at once: each needs a handle and a recipe, the
   process runner of crafting.md, not built. (b) **Chosen (the overseer's call; the developer may overrule):**
   one unhafted **hand axe** that counts as every kind at grade 1, slow and soon worn, so the first minutes work
   with one object. Hafted tools wait for the process runner.

## Decision
- **Data.** `data/items.toml` gains `[[material]]` rows (`speed` over 1 to 20, `grade` 1..4, `uses` 1..10000) and
  item fields `tool` (a kind of `blocks::TOOLS`: axe, pick, shovel, hammer, knife, or `"any"`), `head` (a
  material; a tool stacks by one) and `knap = [item, strikes]`. One material, flint: speed 2, grade 1, 40 uses.
  The hand axe: `tool = "any"`, `head = "flint"`. Flint: `knap = ["hand axe", 4]`. `data/blocks.toml` gains
  `tool` (every solid block), `grade` (0..4, default 0) and `later` (no tool of its grade exists yet). Checked at
  load (`Items::parse`, `Registry::parse`): unknown kind, material or knap target, a material named twice, a
  grade over 4, a grade or `later` on a loose block, a tool on a block that is not solid, a tool that stacks, a
  knap into itself are refused with the row's name.
- **Digging** (survival only; `edit::dig_time_with`, the server's `edit` and `dig`, the client's `dig_step` and
  cracks, the bot's `dig`): a cell whose block's `tool` is the held tool's kind (or any, for the hand axe) takes
  `hardness / speed`, any other its bare-hand time. A block that is not loose drops itself when the tool takes it
  (`Tool::takes`: the kind and at least the grade); rock still leaves its rubble; bare hands are as before. Each
  dug cell the tool fits costs one use (`items::wear`; a hammer's shape as many as it digs); the last use leaves
  nothing, the ledger's
  count of the tool goes down by one (the "worn away" of items.md), and the client plays `tool.break` and the
  `knap` chips.
- **Knapping** (`Server::strike`, `ToServer::Strike(cell)`): in survival, alive, within reach, a knapping item in
  hand and hard rock there (`items::knaps`: the block's tool is pick and hands do not take it, so not soil, wood,
  gravel, rubble, ice or coral). The server counts strikes per player and item; a strike sooner than
  `KNAP_WAIT_S` = 0.4 s (8 ticks) after the last that counted does not count (the client waits a tick more, so
  a strike it shows is not one jitter makes the server drop); another item in hand starts again.
  At 4, one flint goes and a hand axe comes: into the flint's slot when it empties, else where it fits, else at
  the feet. The ledger moves one flint to one hand axe. The client strikes on the place button (press) when a
  knapping item is in hand over hard rock, at most once per wait, and plays `knap` (sound) and `knap` (chips of
  the rock) at once; the swap comes with the next `Inventory`.
- **The look.** A tool's slot shows a bar of its uses left (green to red) in the hotbar and the inventory, in
  place of the count. Items still have no slot pictures: the hand axe is the cube of its `look` (slate) with the
  accent's corner mark, as flint is. In hand, `data/models/hand axe.toml` (voxels, 5 x 7 x 2, three stone tones)
  is drawn by the existing in-hand model path, which now takes any item by name (flint shows its clutter model in
  hand too). The name over the hotbar now names items, not only blocks.
- **Commands.** `/give hand_axe` (`ITEMS.named` reads `_` as a space). The bot: `hold k` (the slot in hand) and
  `strike dx dy dz`; `slots` adds a tool's uses left as a fourth number.

## Numbers for the developer (tuning, by play)
| What | Value | Where |
|---|---|---|
| Hand axe speed | 2.0 (granite 9 s by hand, 4.5 s) | `[[material]] flint` |
| Hand axe uses | 40 digs | same |
| Hand axe grade | 1 | same |
| Strikes to knap | 4 | flint's `knap` |
| Wait between strikes | 0.4 s | `items::KNAP_WAIT_S` |

| Blocks | tool | grade |
|---|---|---|
| grasses, dirt, sand, snow, gravel, clay, mud, regolith (all three), copper/tin/gold gravel, bog iron, bauxite, gossan, every rubble | shovel | 0 |
| logs, bamboo, cactus, every leaves | axe | 0 |
| shrub, reed | knife | 0 |
| stone, every rock (granite to kreep), dolomite, the altered rocks, the hard ores | pick | 1 |
| ice, coral, glass, the lamps (loose: hands take them, so no grade) | pick | 0 |
| copper (the block) | pick | 2, later |
| iron-nickel | pick | 3, later |

A grade or `later` on a loose block, a grade without a tool, or a tool on a block that is not solid is refused at
load; ice, coral, glass and lamps are loose, so they are not knapping rock.

## Tests (each seen failing first, except where said)
- `blocks::every_solid_block_names_its_tool` (red: "grass names no tool", then the data).
- `items::every_block_that_is_not_loose_has_a_tool_that_takes_it_or_waits` and
  `items::a_tool_wears_one_use_a_dig_and_is_gone_at_its_last` (red: no tool in data/items.toml), and the bad rows
  in `bad_rows_are_refused_with_their_name`.
- Server: `flint_struck_four_times_on_rock_becomes_a_hand_axe` (soil knaps nothing, 8 strikes in 0.8 s count as
  two, then two more make the axe; ledger exact; creative knaps nothing) and
  `the_hand_axe_digs_faster_takes_more_and_wears_out` (`hardness / speed`; bare hands refused in the axe's time;
  granite leaves rubble; dolomite lies dug; copper breaks; uses count down; the last one leaves nothing and the
  ledger is exact; creative instant and unworn), both red before the server read `Hand` and `Strike`;
  `a_shape_dug_with_a_tool_costs_a_use_a_cell` (red with one use an edit: "left: Some(1), right: Some(2)").
  `a_save_keeps_a_tools_wear` passed at once: the save already kept the payload; it guards it.
- Bot: `a_survival_bot_knaps_a_hand_axe_and_digs_with_it` (written with the bot's commands, so not seen red).

## Cost
The server's tick with 50 bots (`run=sim quick=1`, `play` build, `bash tools/slot.sh measure`, 150 ticks each):
main (ffc9f557) p50 2.76 ms, p99 4.24 ms, max 4.69 ms; this branch p50 2.20 ms, p99 3.87 ms, max 6.76 ms. Within
the run-to-run noise: the sim's bots never hold a tool or strike, and a dig adds one row lookup per cell. A
strike is one block lookup and a slot change. No perf-debt row.

## Evidence
- Filmstrip `out/bugs-1114/tools-knap.png` (`run=play`, `acts=`): granite placed under the feet, four strikes
  (`knap` played 4 times), the hand axe in hand (a grey teardrop at the lower right), its green wear bar on hotbar
  slot 2 and its name over the bar, the granite cracking in 4.5 s into its rubble.
- A bot against a dedicated server (`technoape-server listen=127.0.0.1:7791 save=0`, seed 1337, 4 km planet):
  strikes 0.2 s apart count once, the fifth strike makes the axe (`[1,"hand axe",1,40]`), a dig with it leaves
  granite rubble and 39 uses.
- Soak `technoape-server soak=1 bots=20 minutes=1 seed=1`: ok.

## Consequences
- Hafted tools are one `[[material]]`-backed item row each once the process runner can make a handle and a head;
  a new metal is a material row (items.md "a tool head is a form").
- A tool taken up at the end of a bare-hand hold digs at once (the hold counts seconds, not who held them).
  shortcut in `Server::edit`; a hold per tool when it matters.
- Only the striker sees and hears a strike (no message for other players), as only the digger saw its own hits
  before ADR 0034's `Digging`.
- Slot pictures for items, hafting, and the worn-away row of the material ledger for metals are still to come.

## Calls for the developer
- The hand axe as every kind at once, instead of separate first tools (overrule: give it `tool = "axe"` and add
  rows).
- The numbers above.
- Knapping only on rock that hands do not take (not ice, coral or rubble).

## Review
A fresh reviewer read `git diff ffc9f557` (engine, server, client, bot). Nothing high; bare hands, creative, the
ledger, the hand index, client and server dig times, replay and saves checked out. Fixed:
- *Medium: a hammer shape cost one use however many cells it took* (27 dolomite for one use): a use a fitted
  cell now, stopping at the last; test above.
- *Medium: a strike the client showed could be dropped by the server's wait under jitter:* the client waits
  0.45 s.
- *Low: a strike in the frame of a change of slot went out with the old hand:* `strike` sends `Hand` first.
- *Low: the worn-out chips named `slate` by name:* they take the worn item's `look`.
- *Low: data gaps* (a material named twice; `later` or a grade on a loose block; a grade without a tool; a tool
  on air): refused at load, with tests. Loose blocks lost the grade 1 the first fill gave them.
Answered, not changed: the knap count does not decay and is not tied to one rock (three strikes, a walk, one
more makes the axe; harmless); the worn-out sound reads an `Inventory` that loses a last-use tool, so a server
change no prediction knew of would sound too (no such change exists; a "worn" message when one does); `_` reads
as a space in `/give` names (no name has one).
