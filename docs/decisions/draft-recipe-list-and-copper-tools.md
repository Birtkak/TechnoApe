# NNNN – Hand crafting by a recipe list, and the copper tools

- **Status:** proposed (built; waits for the developer's play-test and merge)
- **Date:** 2026-11-16

## Context
The copper age's first half (ADR 0137, `crafting.md`): a head and a stick make a copper tool by hand. The crucible
that casts heads is a later run; until then ingots and heads come from `/give`. The developer's calls (2026-11-16):
hand crafting is a **recipe list in the inventory** (what can be made, a click makes one; no grid); a worn-out
**metal** tool gives its stick back, a flint tool vanishes as before; bronze, not brass; no repair.

## Options considered
1. **How the wire names a recipe.** (a) Its index in the file: breaks when a row is added in the middle. (b) **Chosen:**
   its name (`ToServer::Craft { recipe: String, n }`), so rows may be added anywhere; a client of another build
   naming an unknown recipe makes nothing. Saves hold no recipe (only what was made), so they need nothing.
2. **n that cannot all be made.** All or nothing, or **as many as fit (chosen):** the server makes one at a time on a
   copy of the slots and stops at the first make that lacks an input or has no room for all of its output; that make
   changes nothing. It falls out of the loop, and shift-click "make all" needs it.
3. **Where a worn tool's stick comes from.** A rule "metal heads leave a stick" in code, or **a `leaves` field on the
   tool's row (chosen)**: data, and a stone-headed hafted tool later decides for itself.
4. **Where the list sits.** Under the hotbar (the window grows taller, past a 720p screen), or **a column at the
   right of survival's window (chosen):** the window's scale-to-fit (ADR 0113) already handles the wider panel.

## Decision
- **`data/recipes.toml`** (compiled in, `engine::recipes`): `[[recipe]]` with `name`, `process`, `in = [[item, n], ..]`,
  `out = [item, n]`. Only `process = "hand"` exists; the crucible adds `melt`/`cast` to `recipes::PROCESSES` with no
  format change. Refused at load with the row's name: an unknown item, an unknown process, an amount of 0 or nothing
  in, a name used twice (any case) or an item twice in `in`, a recipe that makes its own input. Read against the items
  in force; items are only added or changed in place (ADR 0134), so its ids stay right after a live reload.
- **Crafting** (`recipes::craft`, `Server::craft`): alive, survival or creative, a `hand` recipe; inputs out of any
  slot, the output into the usable slots (`items::give`), the ape's rules kept (`ape_fits`). The ledger moves the
  inputs out and the output in, exactly. Answered by the slots (`Inventory`); the client does not predict it.
- **The list** (survival's inventory, right column, heading "Make"): every `hand` recipe, those the slots can make
  now first and bright ("ready: n"), the others dim with what they lack ("needs 1 stick"); a click makes one,
  shift-click as many as the slots hold the inputs for (the server makes those that fit). The slots' look and
  widgets; no new art. Creative's screen keeps its grid (the server crafts there too, from a bot or a later list).
- **The bot:** `craft <recipe> [n]` (spaces or `_` in the name); `slots` shows the result as before.
- **Copper:** `[[material]] copper`; items `copper ingot` (64), five heads (16: pick, axe, shovel head, knife blade,
  hammer head) and five tools (1, `leaves = "stick"`); five recipes, head + stick. All look like the copper block
  until items have pictures; in hand the cube (no model path is free for them). `items::wear` leaves the row's
  `leaves` in the slot at the last use; the dig and the prospecting strike count it in the ledger. The copper block
  loses `later`: the copper pick takes it, and the data test (`reached != later`) insists both ways.
- **The hammer, as it really is:** no block names `tool = "hammer"`, so the copper hammer fits nothing: it digs at
  bare-hand speed, takes nothing bare hands do not, never wears and does not prospect (only pick and every-kind
  tools do). It waits for a use (smithing, crushing). The hand axe (kind "any") fits every block, a hammer's too.
  The *build* hammer (ADR 0017) is a different thing: a shape tool, unchanged.

## Numbers for the developer (tuning, by play)
| What | Value | Where |
|---|---|---|
| Copper speed | 4.0 (granite 9 s by hand, 4.5 s flint, 2.25 s copper) | `data/items.toml` `[[material]] copper` |
| Copper grade | 2 (takes the copper block; not iron-nickel, 3) | same |
| Copper uses | 160 digs | same |
| Copper prospecting range | 24 m (the pick; flint 16) | same, `prospect` |
| What a worn copper tool leaves | 1 stick | each tool row's `leaves` |
| A tool's recipe | 1 head + 1 stick | `data/recipes.toml` |
| Stacks | ingot 64, heads 16, tools 1 | `data/items.toml` |
| Recipe rows the list shows | 6 (no scroll yet) | `crates/client/src/inventory.rs` `MAKE_H`, `layout` |
| Head costs for the casting run (recorded only) | pick 2 ingots, axe 2, shovel 1, knife 1, hammer 3 | `data/recipes.toml` header |

## Tests (each seen failing first, except where said)
- `recipes::bad_recipes_are_refused_with_their_name` (red: nothing refused), `the_recipes_read_and_the_copper_tools_are_a_head_and_a_stick`
  (passed at once: it reads the data), `a_craft_takes_its_inputs_and_gives_its_output_or_changes_nothing` (red: 0 made).
- `items::copper_tools_take_grade_two_of_their_kind_and_leave_a_stick` (red: `leaves` None), the `leaves` rows of
  `bad_rows_are_refused_with_their_name` (red), and `every_block_that_is_not_loose_has_a_tool_that_takes_it_or_waits`
  (red with copper's `later` still on, then the data).
- Server: `a_head_and_a_stick_make_a_copper_pick_by_hand_or_nothing_changes` (three asked, two made; no head, no
  room, no such recipe: nothing; creative alike; ledger exact) and `copper_tools_dig_their_kind_take_copper_and_leave_a_stick`
  (pick, axe, shovel, knife each in `hardness / 4` and a use; copper taken whole; the last use leaves a stick, ledger
  exact; a save keeps the slots), both red before the server read `Craft` and counted the stick.
- Bot: `a_survival_bot_crafts_a_copper_pick_and_digs_copper` (given a head and two sticks, `craft copper pick 3` makes
  one, digs copper, 159 uses and a stick left). Written after the command, so not seen red.
- Client: `the_screens_put_every_slot_apart_and_inside_the_panel` checks the recipe rows (in the panel and on the
  screen at the smallest UI, over no slot).

## Evidence
Off-screen `run=play planet=768` with `acts=` (give 2 pick heads, a stick, axe heads, a hand axe; open the inventory;
point at the first row; `lclick`): `out/craft/inv2-crop.png` before the click (pick and axe "ready: 1" bright and
first; shovel, knife, hammer dim with "needs 1 copper shovel head"), `out/craft/inv3-crop.png` after it (a copper pick
in hotbar slot 2 with its full wear bar, the stick gone, one head left; every row now dim, "needs 1 stick"). Read.

## Cost
Crafting runs only on a `Craft` message (a slots copy a make). The list computes `makeable` for each hand recipe (5)
per frame while the inventory is open: a few slot counts. No perf-debt row.

## Consequences
- The crucible adds processes and recipes as rows; a machine with a window lists its own process's recipes the
  same way.
- Long "needs" lines are cut with ".." in the 340 px column; a tooltip with the whole line when recipes get longer.
- shortcut: the list shows 6 rows and has no scroll; enough for the five copper tools.
- Creative's screen shows no list (its grid gives everything); the server crafts in creative all the same.
