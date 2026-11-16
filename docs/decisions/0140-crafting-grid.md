# 0140 – A crafting grid, a workbench and a recipe viewer

- **Status:** proposed; supersedes ADR 0139's recipe list (its recipes' first format, `ToServer::Craft` by name as a make, and the Make column). ADR 0139's copper tools and `leaves` stand.
- **Date:** 2026-11-16

## Context
After seeing ADR 0139's list the developer chose (2026-11-16): a **shaped** crafting grid (a pick is its head above its
stick); **2 x 2 in the inventory, 3 x 3 at a workbench**, a block placed and used; a **recipe viewer** like "Just
Enough Items" (click an item, see its recipe; **Fill** lays it out from what you carry, the lacking cells red). A "hold P
to analyze" explainer for complicated items is for later and not built.

## Options considered
1. **Where the grid's cells live.** (a) A new holder per player, with its own message to the client and its own save
   field. (b) **Chosen:** 9 more slots of the player (`items::CRAFT_AT` = 37..45, `items::SLOTS` = 46): the existing
   `Items` moves, the `Inventory` message, the save and the items ledger see them with no change of format. `give`,
   `count` and `take` keep to the first 37, so a place or a pick-up never draws from or drops into a cell.
2. **The take message.** A new variant, or **`ToServer::Craft { recipe, n }` reworked (chosen):** same fields and number;
   it now takes the grid's output, and the server makes only when the cells hold the recipe of that name. The name is
   a guard: a click on an output drawn from a grid that a pending move has since changed makes nothing rather than
   something else (the race ADR 0139's review found in the list). `ToServer::Use(Block)` and `ToServer::Close` are
   appended; no variant moved.
3. **Recipe format.** `in`/`process` kept beside a shape, or **the shape alone (chosen):** `grid` (rows, a letter a cell,
   a space empty) and `key` (a letter's item, or a list of alternatives); `out` kept. One item per cell per make.
4. **How the place button tells a use, a strike and a place apart.** A block with a `window` (only the workbench)
   aimed at within reach: **the use wins**, whatever is in hand (before the bucket, a strike, a place against it). A
   strike needs hard rock (`items::knaps`), which no window block is (tested), so the two never meet; to place a
   block by a bench, aim at another face or block. Sneak-to-place waits for a sneak key.
5. **Which recipe needs the bench.** The rule asked for: wider or taller than 2. The copper tools stay a head above a
   stick (1 x 2, as briefed), so **no recipe in the data needs the bench yet**: the bench gives room (the far column
   and row), and the rule is tested on a test recipe (`['S','S','S']`).

## Decision
- **Recipes** (`data/recipes.toml`, `engine::recipes`): `name`, `grid`, `key`, `out`. A pattern matches anywhere in the
  grid and mirrored left to right, every other cell empty, each cell one of its items. Refused at load with the row's
  name: an unknown item, a key letter unused or undefined (or not one letter), ragged rows, over 3 x 3, an empty edge
  row or column, an amount of 0, a name twice, its output among its inputs, two recipes one grid could match (same
  size, mirrored or not every cell both empty or sharing an item). The data: `workbench` (2 x 2 of any of the six logs)
  and the five copper tools (head above stick).
- **The workbench** (`data/blocks.toml` id 218): axe, loose (hands take it back), the planks pattern in wood colours
  (no new art), `window = "workbench"`. `blocks::WINDOWS` lists the windows a row may name; refused otherwise, or on a
  block not solid or falling. **The use step**: `blocks::Def::window` -> the client's `use_target`/`use_block` ->
  `ToServer::Use` -> `Server::use_block`, which opens it (sets `Client::bench`). The crucible later adds its name to
  `WINDOWS`, its own holder on the server and its arm in `use_block`; nothing for it is built.
- **Server-authoritative**: the 2 x 2's cells always take moves; the other five only while `Server::at_bench` (the
  bench used is still a workbench and within `BENCH_REACH` of the feet). `Craft` matches the 3 x 3 there, else the
  2 x 2 alone (anything outside it, or a recipe that needs the bench, matches nothing). Each make takes one item from
  every full cell and gives the output into the usable slots, all or nothing; shift asks for `u16::MAX` and the
  server stops when the cells no longer match or the output has no room. The ledger moves each item taken and made.
  **Never lost**: `Close`, a walk out of reach of the bench (checked every tick), a death (at once; the pack then lies
  where it fell on respawn, ADR 0040), a leave and a join all put the cells into the usable slots, the rest at the
  feet. A save keeps what lies in the cells; the join after puts it into the pack. A save from before (37 slots) loads
  with 9 empty cells.
- **Crafting by name is gone** (it would make a bench recipe with no bench): no Make column; the bot fills the cells
  with `move` and takes with `craft [n]` (naming the recipe its cells hold), `use dx dy dz`, `close`.
- **The screen**: survival's (and creative's at a workbench) shows the grid right of the pile with an arrow and the
  output (drawn from what the cells match, its name under it); a click takes one, shift as many as the cells hold.
  Under the hotbar the **viewer**: a search field (click it to type) over every item whose name holds it, recipes
  first, 11 x 2 in view, the wheel scrolls; a click shows the recipe beside it: the pattern in a 3 x 3 from its top
  left (a cell red when the slots and cells hold none of its items; alternatives shown in turn), what it makes, "needs a
  workbench" or "by hand, 2 x 2", and **Fill**. Fill (`inventory::fill_plan`) first moves what lies in a cell the pattern
  does not want back into the slots, then one wanted item into each pattern cell, all by ordinary `Items` moves (the
  server judges each); the cells nothing was found for stay empty and red until they get an item. With the 2 x 2
  open and a bench recipe shown, Fill is dim and the line says it needs a workbench. An item with no recipe says
  where it comes from when the data knows (knapped from, found digging, left by digging), else "no recipe".
  Creative's own screen is unchanged. Scripts: acts `view:<item>`, `fill`.

## Numbers for the developer (tuning, by play)
| What | Value | Where |
|---|---|---|
| Bench reach (use, and while its 3 x 3 counts) | 6 m, feet to the bench's middle | `crates/engine/src/items.rs` `BENCH_REACH` |
| Workbench | 4 logs, any kind, 2 x 2 -> 1 | `data/recipes.toml` |
| Workbench hardness / tool | 2.5 s by hand, axe, loose | `data/blocks.toml` id 218 |
| Copper tool | 1 head above 1 stick, 2 x 2 (no bench) | `data/recipes.toml` |
| A recipe needs the bench | wider or taller than 2 | `engine::recipes::Recipe::bench` |
| Items a make takes | 1 from every cell of the pattern | `engine::recipes::make` |
| Viewer | 11 x 2 items in view, recipes first | `crates/client/src/inventory.rs` `VCOLS`, `VROWS` |

## Tests (each seen failing first)
- Engine: `recipes::a_pattern_matches_anywhere_mirrored_with_alternatives_and_nothing_else`,
  `the_grid_matches_its_recipe_the_2x2_only_without_a_bench` (a tall test recipe refused in the 2 x 2, accepted in the
  3 x 3), `bad_recipes_are_refused_with_their_name` (17 refusals), `a_make_takes_one_from_each_cell_and_gives_its_output_or_changes_nothing`
  (red: the matcher and the overlap check stubbed), `the_recipes_read_...` and
  `blocks::the_workbench_opens_its_window_and_other_windows_are_refused` (red: no workbench row).
- Server: `the_grid_makes_its_pattern_from_its_cells_or_nothing_changes`, `the_bench_cells_count_only_at_a_workbench`
  (refused with no bench, a use of another block or from 20 m; a pick made in the far column at the bench; walking away
  returns the cells and refuses again), `the_cells_go_back_on_close_death_and_leaving_and_a_save_keeps_them` (close,
  close with a full pack -> at the feet, death, save and load then join, leave, a 37-slot save; ledger exact each time).
  Red with the take, the bench check and the put-away stubbed.
- Bot: `a_survival_bot_makes_a_workbench_then_a_copper_pick_at_it_and_digs_copper` (4 oak logs -> workbench, placed,
  used, head and stick in the far column -> pick, digs copper, 159 uses). Red with `use` sending nothing.
- Client: `fill_lays_a_recipe_from_the_slots_and_marks_what_is_lacking` (all there; a stick lacking -> cell 3 red; two
  kinds of log; a bench recipe: None at the 2 x 2), `an_item_with_no_recipe_says_where_it_comes_from_if_the_data_knows`,
  and the layout test (output, search, items, pattern and Fill in the panel, on the screen, over no slot; 4 or 9 cells).

## Evidence
Off-screen `run=play planet=768` filmstrips, read: `out/craft/a-crop.png` (inventory: the 2 x 2 with the head in its
top left and the cell under it red, the viewer's copper pick selected with its pattern, the stick's cell red, "by hand,
2 x 2", Fill lit); `out/craft/b-crop.png` (`/place workbench`, look down, the place button: the WORKBENCH screen, the
3 x 3 with four logs filled by Fill, the arrow and a workbench in the output).

## Cost
The server checks each player's bench once a tick (a block read and a distance) and matches the grid on a `Craft`.
The client matches the grid and draws the viewer while the screen is open. Not measured; no perf-debt row.

## Consequences
- The "hold P to analyze" explainer has its place: the recipe panel under Fill (the panel's lines stop at its top half).
- A machine's window (the crucible) hangs on `Def::window` and `use_block`; its slots become a holder of its own.
- No data recipe needs the bench yet; the first 3-wide recipe makes it necessary. Until then the bench is room.
- shortcut: the output is not predicted (one server tick); no drag-to-spread over cells; Fill lays one make's worth.
- An older build reads a 46-slot save as "slots that do not fit" and repacks them (nothing lost).
